use crate::monitor::{View, BLUE, GREEN, PURPLE};
use crate::{
    audio::{self, Capture, Player},
    dsp::{self, Frame},
    training::{self, Config, Exercise, Mode, Range, RangeEvent, RangePhase, Result, Segmenter},
};
use eframe::egui::{self, Color32, Pos2, RichText, Stroke, Vec2};
use std::{
    collections::VecDeque,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
#[derive(Debug)]
enum Workflow {
    AwaitQuiet { since: Option<f64>, after: f64 },
    Listening,
    Reference,
    Cooldown { until: f64 },
    Complete,
    Stopped,
}
struct Example {
    note: i32,
    second: Option<i32>,
    started: Option<f64>,
    stage: u8,
    reference_time: f64,
}
struct Guide {
    label: String,
    note: i32,
    texture: Option<egui::TextureHandle>,
    texture_key: Option<(usize, f32, f32, f32)>,
    frames: Vec<Frame>,
    voice: Vec<Frame>,
    voice_start: Option<f64>,
    pending: Option<std::sync::mpsc::Receiver<Vec<Frame>>>,
}
impl Guide {
    fn trim_voice_end(&mut self) {
        let keep = self
            .voice
            .iter()
            .rposition(crate::melody::reliable)
            .map_or(0, |i| i + 1);
        self.voice.truncate(keep);
        self.texture_key = None;
    }
    fn review_frames(&self, singing: bool) -> &[Frame] {
        if singing {
            &self.voice
        } else {
            &self.frames
        }
    }
    fn review_label(&self, singing: bool) -> String {
        if !singing {
            return self.label.clone();
        }
        let pitches: Vec<_> = self
            .voice
            .iter()
            .filter(|f| crate::melody::reliable(f))
            .map(|f| dsp::midi(f.hz.unwrap()))
            .collect();
        if pitches.is_empty() {
            "No voice".into()
        } else {
            dsp::name(training::median(&pitches).round() as i32)
        }
    }
}
#[derive(Clone, Copy)]
enum InstructionIcon {
    Up,
    Down,
    Hold,
    Listen,
    Breathe,
    Done,
}

/// Reusable presentation for a short coaching instruction, independent of
/// range detection so drills can supply their own cues later.
struct InstructionCard<'a> {
    icon: InstructionIcon,
    title: &'a str,
    detail: &'a str,
    color: Color32,
}
impl InstructionCard<'_> {
    fn show(&self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 60.0), egui::Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 8.0, Color32::from_rgb(24, 38, 49));
        p.rect_stroke(
            rect,
            8.0,
            Stroke::new(1.0_f32, self.color),
            egui::StrokeKind::Inside,
        );
        let point = |x: f32, y: f32| rect.min + Vec2::new(6.0 + x * 0.7, 15.0 + y * 0.7);
        let stroke = Stroke::new(2.5_f32, self.color);
        match self.icon {
            InstructionIcon::Up | InstructionIcon::Down => {
                let (tip, tail, wing) = if matches!(self.icon, InstructionIcon::Up) {
                    (6.0, 36.0, 18.0)
                } else {
                    (36.0, 6.0, 24.0)
                };
                p.line_segment([point(21.0, tail), point(21.0, tip)], stroke);
                p.line_segment([point(9.0, wing), point(21.0, tip)], stroke);
                p.line_segment([point(33.0, wing), point(21.0, tip)], stroke);
            }
            InstructionIcon::Hold => {
                for y in [16.0, 27.0] {
                    p.line_segment([point(7.0, y), point(35.0, y)], stroke);
                }
            }
            InstructionIcon::Listen => {
                p.circle_filled(point(15.0, 31.0), 6.0, self.color);
                p.line_segment([point(20.0, 31.0), point(20.0, 8.0)], stroke);
                p.line_segment([point(20.0, 8.0), point(32.0, 12.0)], stroke);
            }
            InstructionIcon::Breathe => {
                let points = (0..=24)
                    .map(|i| {
                        let x = i as f32 / 24.0;
                        point(
                            5.0 + x * 32.0,
                            21.0 - (x * std::f32::consts::TAU).sin() * 8.0,
                        )
                    })
                    .collect();
                p.add(egui::Shape::line(points, stroke));
            }
            InstructionIcon::Done => {
                p.line_segment([point(7.0, 22.0), point(17.0, 32.0)], stroke);
                p.line_segment([point(17.0, 32.0), point(36.0, 10.0)], stroke);
            }
        }
        let text = p.with_clip_rect(rect.shrink(5.0));
        text.text(
            Pos2::new(rect.left() + 44.0, rect.top() + 6.0),
            egui::Align2::LEFT_TOP,
            self.title,
            egui::FontId::proportional(18.0),
            self.color,
        );
        let detail = text.layout(
            self.detail.to_owned(),
            egui::FontId::proportional(11.0),
            Color32::from_gray(195),
            rect.width() - 50.0,
        );
        text.galley(
            Pos2::new(rect.left() + 44.0, rect.top() + 29.0),
            detail,
            Color32::from_gray(195),
        );
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, self.title));
        response.on_hover_text(self.detail)
    }
}

pub struct Coach {
    melody: crate::melody::Coach,
    epoch: Instant,
    capture: Option<Capture>,
    player: Player,
    devices: Vec<String>,
    selected: Option<String>,
    settings: bool,
    error: Option<String>,
    playback_only: bool,
    config: Config,
    segmenter: Segmenter,
    workflow: Workflow,
    latest: Option<Frame>,
    history: VecDeque<Frame>,
    reference: VecDeque<Frame>,
    attempts: Vec<training::Measured>,
    feedback: Option<Result>,
    view: View,
    seconds_back: f64,
    guides: Vec<Guide>,
    guide: Option<usize>,
    review_singing: bool,
    sequence_start: usize,
    example: Option<Example>,
    range: Option<Range>,
    range_reference_ready_at: Option<f64>,
    range_piano_finished: bool,
    range_input_after: f64,
    range_resume_at: Option<f64>,
    range_descending: bool,
    accepted: Option<(i32, i32, i32)>,
    accepted_sustain: Option<(i32, f64)>,
    capture_seen: f64,
    spectrum_secs: f32,
    spectrum_low: f32,
    spectrum_high: f32,
    spectrum_gain: f32,
}
impl Coach {
    pub fn new(cc: &eframe::CreationContext<'_>, playback_only: bool) -> Self {
        Self::configure(&cc.egui_ctx);
        Self::init(playback_only)
    }
    fn configure(ctx: &egui::Context) {
        ctx.set_visuals(egui::Visuals::dark());
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = Vec2::new(8.0, 4.0);
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        ctx.set_style(style);
    }
    fn init(playback_only: bool) -> Self {
        let mut s = Self {
            melody: crate::melody::Coach::default(),
            epoch: Instant::now(),
            capture: None,
            player: Player::new(),
            devices: Vec::new(),
            selected: None,
            settings: false,
            error: None,
            playback_only,
            config: Config::default(),
            segmenter: Segmenter::default(),
            workflow: Workflow::Stopped,
            latest: None,
            history: VecDeque::new(),
            reference: VecDeque::new(),
            attempts: Vec::new(),
            feedback: None,
            view: View::Compact,
            seconds_back: 0.0,
            guides: Vec::new(),
            guide: None,
            review_singing: false,
            sequence_start: 0,
            example: None,
            range: None,
            range_reference_ready_at: None,
            range_piano_finished: false,
            range_input_after: 0.0,
            range_resume_at: None,
            range_descending: false,
            accepted: None,
            accepted_sustain: None,
            capture_seen: 0.0,
            spectrum_secs: 1.0,
            spectrum_low: 65.0,
            spectrum_high: 2000.0,
            spectrum_gain: 12.0,
        };
        if !playback_only {
            s.devices = audio::inputs();
            s.start_capture();
        }
        s
    }
    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
    fn start_capture(&mut self) {
        if self.config.exercise == Exercise::Melody && self.range.is_none() {
            self.stop_reference();
            self.melody.reset();
        }
        self.segmenter.clear();
        self.latest = None;
        self.capture_seen = self.now();
        if let Some(r) = &mut self.range {
            r.reset_hold();
        }
        match Capture::start(self.selected.as_deref(), self.epoch) {
            Ok(c) => {
                self.capture = Some(c);
                self.error = None;
                self.workflow = Workflow::AwaitQuiet {
                    since: None,
                    after: self.now(),
                };
            }
            Err(e) => {
                self.capture = None;
                self.error = Some(e);
                self.workflow = Workflow::Stopped;
            }
        }
    }
    fn stop_capture(&mut self) {
        if self.config.exercise == Exercise::Melody && self.range.is_none() {
            self.stop_reference();
            self.melody.fail(
                "Microfone parado ou interrompido: dados insuficientes. Reinicia a captação.",
            );
        }
        self.capture = None;
        self.latest = None;
        self.segmenter.clear();
        self.workflow = Workflow::Stopped;
        if self.range.is_some() {
            self.stop_reference();
            self.range = None;
        }
    }
    fn reset(&mut self) {
        let was_playing = self.player.playing;
        self.melody.reset();
        self.sequence_start = self.guides.len();
        self.guide = None;
        self.stop_reference();
        self.segmenter.clear();
        self.attempts.clear();
        self.feedback = None;
        self.workflow = if self.capture.is_some() {
            if was_playing {
                Workflow::Cooldown {
                    until: self.now() + 1.0,
                }
            } else {
                Workflow::AwaitQuiet {
                    since: None,
                    after: self.now(),
                }
            }
        } else {
            Workflow::Stopped
        };
    }
    fn stop_reference(&mut self) {
        self.player.stop();
        self.example = None;
        self.segmenter.clear();
        if self.capture.is_some() {
            self.workflow = Workflow::Cooldown {
                until: self.now() + 1.0,
            };
        } else {
            self.workflow = Workflow::Stopped;
        }
    }
    fn begin_example(&mut self) {
        self.stop_reference();
        if self.config.exercise == Exercise::Melody {
            self.melody.prepare_reference();
            match self.player.melody(&self.melody.sequence()) {
                Ok(()) => {
                    self.melody.state = crate::melody::State::Playback;
                    self.workflow = Workflow::Reference;
                }
                Err(e) => self.melody.fail(&e),
            }
            return;
        }
        let second = if self.config.exercise == Exercise::Transition {
            Some(self.config.target + self.config.interval)
        } else {
            None
        };
        let note = self.config.target;
        self.reference.push_back(Frame {
            time: self.now(),
            hz: None,
            confidence: 0.0,
            rms: 0.0,
            clipped: false,
            spectrum: None,
        });
        self.add_guide(note, second);
        match self.player.example(note, second) {
            Ok(()) => {
                self.workflow = Workflow::Reference;
                self.example = Some(Example {
                    note,
                    second,
                    started: None,
                    stage: 0,
                    reference_time: 0.0,
                });
                self.error = None;
            }
            Err(e) => {
                self.error = Some(e);
                self.player.stop();
            }
        }
    }
    fn add_guide(&mut self, note: i32, second: Option<i32>) {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let pcm = audio::render_example(
                dsp::hz(note as f32),
                second.map(|n| dsp::hz(n as f32)),
                12000,
            );
            let mut detector = dsp::Detector::new();
            let frames = (0..=pcm.len() - 600)
                .step_by(120)
                .enumerate()
                .map(|(i, start)| {
                    detector.analyze(
                        &pcm[start..start + 600],
                        12000.0,
                        start as f64 / 12000.0,
                        i % 10 == 0,
                    )
                })
                .collect();
            let _ = sender.send(frames);
        });
        self.guides.push(Guide {
            note,
            texture: None,
            texture_key: None,
            label: second
                .map(|s| format!("{} → {}", dsp::name(note), dsp::name(s)))
                .unwrap_or(dsp::name(note)),
            frames: Vec::new(),
            voice: Vec::new(),
            voice_start: None,
            pending: Some(receiver),
        });
        if self.guides.len() > 24 {
            self.guides.remove(0);
            self.sequence_start = self.sequence_start.saturating_sub(1);
        }
        self.guide = None;
    }
    fn start_range(&mut self) {
        self.melody.reset();
        let tail = if self.player.playing { 0.5 } else { 0.0 };
        self.sequence_start = self.guides.len();
        self.guide = None;
        self.stop_reference();
        self.segmenter.clear();
        self.range_reference_ready_at = None;
        self.range_piano_finished = false;
        self.range_input_after = self.now() + tail;
        self.range_resume_at = None;
        self.range_descending = false;
        self.range = Some(Range::new());
        self.review_singing = true;
    }
    fn range_reference(&mut self) {
        if let Some(g) = self.guides.last_mut() {
            g.trim_voice_end();
        }
        self.range_piano_finished = false;
        self.segmenter.clear();
        self.range_resume_at = None;
        self.range_reference_ready_at = None;
        if let Some(r) = &mut self.range {
            r.reset_hold();
            if let Some(note) = r.target {
                self.add_guide(note, None);
                if let Err(e) = self.player.play(note) {
                    self.error = Some(e);
                    self.player.stop();
                } else {
                    self.error = None;
                }
            } else {
                let tail = if self.player.playing { 0.5 } else { 0.0 };
                self.player.stop();
                self.range_input_after = self.now() + tail;
            }
        }
    }
    fn observe_range_frame(&mut self, f: &Frame) {
        if self.range_resume_at.is_some() || f.time < self.range_input_after {
            return;
        }
        // A deliberately silenced, already matched hold keeps microphone
        // tracking. It cannot qualify any new match without playback readiness.
        let ready = ((self.player.playing && self.player.ready)
            || (self.range_piano_finished
                && self.range.as_ref().is_some_and(Range::holding_match)))
            && self.range_reference_ready_at.is_some_and(|t| f.time >= t);
        if let Some(range) = &mut self.range {
            let was_holding = range.holding_match();
            let event = range.observe_guided(f, ready, self.range_reference_ready_at);
            if let Some(g) = self.guides.last_mut() {
                if range.holding_match() && g.voice.len() < 1600 {
                    // Retain only the confirmed hold, never its approach slide.
                    let start = *g.voice_start.get_or_insert(f.time);
                    let mut copy = f.clone();
                    copy.time -= start;
                    g.voice.push(copy);
                    g.texture_key = None;
                } else if was_holding && !range.holding_match() {
                    g.trim_voice_end();
                }
            }
            self.range_event(event);
        }
    }
    fn poll_range_reference(&mut self, now: f64) {
        if self.range_resume_at.is_some() {
            return;
        }
        if let (Some(range), Some(start)) = (&mut self.range, self.range_reference_ready_at) {
            let ready = (self.player.playing && self.player.ready) || self.range_piano_finished;
            if ready && now - start >= 6.0 && !self.range_piano_finished {
                self.player.stop();
                self.range_piano_finished = true;
            }
            let event = range.reference_elapsed(now - start, now, ready);
            self.range_event(event);
        }
    }
    fn range_event(&mut self, event: RangeEvent) {
        match event {
            RangeEvent::None => {}
            RangeEvent::ReferenceChanged => {
                let descending = self
                    .range
                    .as_ref()
                    .is_some_and(|r| matches!(r.phase, RangePhase::Lower | RangePhase::LowerEdge));
                if descending && !self.range_descending {
                    self.range_descending = true;
                    self.player.stop();
                    self.range_reference_ready_at = None;
                    self.range_resume_at = Some(self.now() + 2.0);
                } else {
                    self.range_reference();
                }
            }
            RangeEvent::Complete => self.finish_range(),
        }
    }
    fn finish_range(&mut self) {
        if let Some(g) = self.guides.last_mut() {
            g.trim_voice_end();
        }
        self.range_resume_at = None;
        if let Some(r) = self.range.take() {
            if let Some(result) = r.recommendation() {
                self.accepted = Some(result);
                self.accepted_sustain = r.highest_sustain;
                self.config.target = result.2;
            } else {
                self.error =
                    Some("Range check incomplete: match at least two different notes.".into());
            }
        }
        self.reset();
    }
    fn poll(&mut self) {
        for g in &mut self.guides {
            if let Some(receiver) = &g.pending {
                if let Ok(frames) = receiver.try_recv() {
                    g.frames = frames;
                    g.pending = None;
                }
            }
        }
        self.player.poll();
        if let Some(error) = self.player.error.take() {
            if self.config.exercise == Exercise::Melody && self.range.is_none() {
                self.melody.fail(&error);
            }
            self.error = Some(error);
            self.example = None;
            self.segmenter.clear();
            if let Some(r) = &mut self.range {
                r.reset_hold();
            }
            self.workflow = Workflow::Cooldown {
                until: self.now() + 0.5,
            };
        }
        let now = self.now();
        if self.range.is_none() {
            self.range_resume_at = None;
        }
        if self.range_resume_at.is_some_and(|until| now >= until) {
            self.range_reference();
        }
        if (!self.player.playing || !self.player.ready) && !self.range_piano_finished {
            self.range_reference_ready_at = None;
        }
        if self.range.is_some()
            && self.player.playing
            && self.player.ready
            && self.range_reference_ready_at.is_none()
        {
            self.range_reference_ready_at = Some(now);
        }
        if self
            .capture
            .as_ref()
            .is_some_and(|c| c.failed.load(Ordering::Relaxed))
        {
            self.stop_capture();
            self.error=Some("Microphone interrupted or analysis fell behind. Restart capture for a fresh attempt.".into());
        }
        // Drain a bounded queue before rendering; no DSP is performed on this thread.
        let mut frames = Vec::new();
        if let Some(c) = &self.capture {
            while let Ok(f) = c.frames.try_recv() {
                frames.push(f);
            }
        }
        if !frames.is_empty() {
            self.capture_seen = now;
        }
        if self.capture.is_some() && now - self.capture_seen > 3.0 {
            self.stop_capture();
            self.error = Some(
                "No audio frames received. Check the input device, then restart capture.".into(),
            );
        }
        for f in frames {
            self.latest = Some(f.clone());
            self.history.push_back(f.clone());
            while self.history.len() > 60000 {
                self.history.pop_front();
            }
            if self.range.is_some() {
                self.observe_range_frame(&f);
                continue;
            }
            if self.config.exercise == Exercise::Melody {
                self.observe_melody_frame(&f);
            }
            match &mut self.workflow {
                Workflow::Cooldown { until } if f.time >= *until => {
                    self.workflow = Workflow::AwaitQuiet {
                        since: None,
                        after: *until,
                    };
                }
                Workflow::AwaitQuiet { since, after } => {
                    if f.time >= *after
                        && f.rms < 0.008
                        && (self.config.exercise != Exercise::Melody || !f.clipped)
                    {
                        let start = *since.get_or_insert(f.time);
                        if f.time - start >= 0.25 {
                            self.workflow = if self.config.exercise != Exercise::Melody
                                && self.attempts.len() >= 5
                            {
                                Workflow::Complete
                            } else {
                                Workflow::Listening
                            };
                        }
                    } else {
                        *since = None;
                    }
                }
                Workflow::Listening if self.config.exercise != Exercise::Melody => {
                    if let Some(segment) = self.segmenter.push(f) {
                        let result = training::evaluate(&segment, self.config);
                        if let Result::Measured(m) = &result {
                            self.attempts.push(m.clone());
                        }
                        self.feedback = Some(result);
                        if self.attempts.len() >= 5 {
                            self.workflow = Workflow::Complete;
                        }
                    }
                }
                _ => {}
            }
        }
        self.poll_melody(now);
        if let Some(range) = &mut self.range {
            let event = range.advance(now);
            let resting = range.resting(now);
            if resting {
                self.player.stop();
            }
            self.range_event(event);
        }
        // Process queued matches first so a successful final attempt wins
        // over the unanswered-prompt timeout. Each new target resets readiness.
        self.poll_range_reference(now);
        if self.example.is_some() && self.player.ready {
            let example = self.example.as_mut().unwrap();
            let start = *example.started.get_or_insert(now);
            let elapsed = now - start;
            let note = if example.stage == 1 {
                example.second.unwrap_or(example.note)
            } else {
                example.note
            };
            if now - example.reference_time >= 0.01 {
                self.reference.push_back(Frame {
                    time: now,
                    hz: Some(dsp::hz(note as f32)),
                    confidence: 1.0,
                    rms: 0.106,
                    clipped: false,
                    spectrum: None,
                });
                example.reference_time = now;
            }
            if let Some(second) = example
                .second
                .filter(|_| example.stage == 0 && elapsed >= crate::piano::EXAMPLE_NOTE_SECONDS)
            {
                example.stage = 1;
                example.started = None;
                if let Err(e) = if audio::is_wsl() {
                    Ok(())
                } else {
                    self.player.play(second)
                } {
                    self.error = Some(e);
                    self.stop_reference();
                }
            } else if elapsed >= crate::piano::EXAMPLE_NOTE_SECONDS {
                self.stop_reference();
            }
        }
        if let Some(r) = &self.range {
            if let Some(note) = r
                .target
                .filter(|_| self.player.ready && self.player.playing)
            {
                self.reference.push_back(Frame {
                    time: now,
                    hz: Some(dsp::hz(note as f32)),
                    confidence: 1.0,
                    rms: 0.106,
                    clipped: false,
                    spectrum: None,
                });
            }
        }
        while self.reference.len() > 60000 {
            self.reference.pop_front();
        }
        if self.latest.as_ref().is_some_and(|f| now - f.time > 0.25) {
            self.latest = None;
        }
    }
    fn status(&self) -> (&str, Color32) {
        if self.playback_only {
            return ("Playback diagnostic · microphone disabled", PURPLE);
        }
        if let Some(r) = &self.range {
            if r.phase == RangePhase::Baseline {
                return ("Range check · sing your starting note", GREEN);
            }
            return if self.player.ready {
                ("Range check · match the guide", GREEN)
            } else {
                ("Range check · reference not ready", Color32::YELLOW)
            };
        }
        match self.workflow {
            Workflow::Listening => ("Listening · sing, then pause", GREEN),
            Workflow::Reference => ("Listen to the reference", PURPLE),
            Workflow::Cooldown { .. } => ("Reference finishing…", PURPLE),
            Workflow::AwaitQuiet { .. } => ("Pause briefly to begin", Color32::YELLOW),
            Workflow::Complete => ("Drill complete · review your consistency", GREEN),
            Workflow::Stopped => ("Microphone stopped", Color32::GRAY),
        }
    }
    fn settings_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.settings;
        egui::Window::new("Audio settings").open(&mut open).anchor(egui::Align2::RIGHT_TOP,[-12.0,60.0]).collapsible(false).resizable(false).show(ctx,|ui|{
            ui.label("Microphone");let old=self.selected.clone();
            egui::ComboBox::from_id_salt("device").selected_text(self.selected.as_deref().unwrap_or("System / desktop default")).width(300.0).show_ui(ui,|ui|{ui.selectable_value(&mut self.selected,None,"System / desktop default");for device in &self.devices{ui.selectable_value(&mut self.selected,Some(device.clone()),device);}});
            if old!=self.selected && self.capture.is_some(){self.capture=None;self.start_capture();if self.range.is_some(){self.range_reference();}}
            ui.horizontal(|ui|{if ui.button("Refresh devices").clicked(){self.devices=audio::inputs();}
 if ui.button("Use default").clicked(){self.selected=None;if self.capture.is_some(){self.capture=None;self.start_capture();}}});
            ui.separator();ui.label("Reference output");ui.colored_label(PURPLE,audio::route());ui.small("Piano reference. Use headphones during range checking.");
            ui.separator();ui.label("Session data");ui.small("No voice recordings, accounts or network services. Charts retain up to 10 minutes; up to 24 reference guides. Everything clears when you close the app.");
        });
        self.settings = open;
    }
    fn active_target(&self) -> Option<i32> {
        if self.config.exercise == Exercise::Melody && self.range.is_none() {
            let elapsed = match self.melody.state {
                crate::melody::State::Recording { .. } => self.melody_elapsed(self.now()),
                crate::melody::State::Playback => self
                    .melody
                    .playback_start
                    .map_or(-1.0, |start| self.now() - start),
                _ => -1.0,
            };
            return self
                .melody
                .notes()
                .iter()
                .find(|n| elapsed >= n.start && elapsed < n.start + n.duration)
                .map(|n| n.midi);
        }
        if let Some(range) = &self.range {
            return range.target;
        }
        if self.config.exercise == Exercise::Transition
            && (self.example.as_ref().is_some_and(|e| e.stage == 1)
                || self.segmenter.started_at().is_some_and(|t| {
                    self.latest
                        .as_ref()
                        .is_some_and(|f| f.time - t >= crate::piano::EXAMPLE_NOTE_SECONDS)
                }))
        {
            Some(self.config.target + self.config.interval)
        } else {
            Some(self.config.target)
        }
    }
    fn coaching_callout(&self, ui: &mut egui::Ui) {
        let (icon, title, message, color) = if self.config.exercise == Exercise::Melody
            && self.range.is_none()
        {
            use crate::melody::State;
            let (title, detail) = match self.melody.state {
                State::Playback => ("Listen", "Listen to the piano sequence.".into()),
                State::Recording { .. } => (
                    "Sing",
                    self.active_target()
                        .map_or_else(|| "Follow the melody".into(), dsp::name),
                ),
                State::Results => ("Review", "See the note results below.".into()),
                State::Error => ("Check input", "Restart capture and try again.".into()),
                State::Ready => (
                    "Sing",
                    "Start when ready · your voice starts the timeline.".into(),
                ),
                _ => ("Ready", "Sing to start, or press play for piano.".into()),
            };
            (InstructionIcon::Hold, title, detail, BLUE)
        } else if let Some(range) = &self.range {
            let (icon, title) = match range.phase {
                RangePhase::Baseline => (InstructionIcon::Hold, "Sing an easy note"),
                RangePhase::ConfirmStart => (InstructionIcon::Hold, "Match this note"),
                RangePhase::Higher | RangePhase::UpperEdge => (InstructionIcon::Up, "Go higher"),
                RangePhase::Lower | RangePhase::LowerEdge => (InstructionIcon::Down, "Go lower"),
                RangePhase::Complete => (InstructionIcon::Done, "Complete"),
            };
            let pace = range.pacing_cue(self.now(), self.range_reference_ready_at);
            let (icon, title) = if self.range_resume_at.is_some() {
                (InstructionIcon::Down, "Breathe · then lower")
            } else {
                match pace {
                    Some("Listen to the piano") => (InstructionIcon::Listen, "Listen"),
                    Some("Matched — finish gently") => (InstructionIcon::Hold, "Hold"),
                    Some("Matched — take a breath") => (InstructionIcon::Breathe, "Breathe"),
                    _ => (icon, title),
                }
            };
            let direction = match range.phase {
                RangePhase::Higher | RangePhase::UpperEdge => "Going up",
                RangePhase::Lower | RangePhase::LowerEdge => "Going down",
                _ => "Follow the piano",
            };
            let message = if let Some(note) = range.target {
                if matches!(range.phase, RangePhase::Lower | RangePhase::LowerEdge) {
                    let high = range
                        .matched
                        .iter()
                        .max()
                        .copied()
                        .map(dsp::name)
                        .unwrap_or_else(|| "—".into());
                    format!("{direction} · {}\nHighest matched: {high}", dsp::name(note))
                } else {
                    format!("{direction} · {} · hold in green for 1s", dsp::name(note))
                }
            } else {
                "Use an easy, relaxed ‘ah’.".into()
            };
            let message = if title == "Hold" {
                format!(
                    "{} · {:.1}s\nRelease when ready",
                    range.target.map(dsp::name).unwrap_or_else(|| "—".into()),
                    range.held_seconds()
                )
            } else {
                message
            };
            (
                icon,
                title,
                message,
                if matches!(range.phase, RangePhase::Lower | RangePhase::LowerEdge) {
                    PURPLE
                } else {
                    BLUE
                },
            )
        } else if let Some((low, high, start)) = self.accepted {
            (
                InstructionIcon::Done,
                "Complete",
                format!(
                    "Try {} · target set\nMatched {}–{}",
                    dsp::name(start),
                    dsp::name(low),
                    dsp::name(high)
                ),
                GREEN,
            )
        } else {
            let (icon, title) = match self.workflow {
                Workflow::Reference => (InstructionIcon::Listen, "Listen"),
                Workflow::Listening => (InstructionIcon::Hold, "Sing"),
                Workflow::Cooldown { .. } | Workflow::AwaitQuiet { .. } => {
                    (InstructionIcon::Breathe, "Breathe")
                }
                Workflow::Complete => (InstructionIcon::Done, "Review"),
                Workflow::Stopped => (InstructionIcon::Hold, "Ready"),
            };
            (
                icon,
                title,
                "Your next instruction appears here.".into(),
                BLUE,
            )
        };
        let response = InstructionCard {
            icon,
            title,
            detail: &message,
            color,
        }
        .show(ui);
        if let Some(range) = &self.range {
            response.on_hover_text(range.stage_message());
        }
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("SingCoach");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").on_hover_text("Audio settings").clicked() {
                    self.settings = !self.settings;
                }
            });
        });
        ui.add_space(6.0);
        ui.separator();
        {
            let card_width = 280.0;
            let gap = 12.0;
            let controls_width = ui.available_width() - card_width - gap;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                ui.allocate_ui_with_layout(
                    Vec2::new(controls_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(controls_width);
                        self.header_controls(ui);
                    },
                );
                ui.allocate_ui_with_layout(
                    Vec2::new(card_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(card_width);
                        self.coaching_callout(ui);
                    },
                );
            });
        }
    }
    fn header_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let checking = self.range.is_some();
            let label = if checking {
                "■ Stop range check"
            } else {
                "▶ Find my range"
            };
            if ui
                .add_enabled(
                    checking || self.capture.is_some(),
                    egui::Button::new(label)
                        .min_size(Vec2::new(136.0, 30.0))
                        .corner_radius(8),
                )
                .on_hover_text(
                    "Explore notes you can match in your relaxed usual voice. Use headphones.",
                )
                .clicked()
            {
                if checking {
                    self.range = None;
                    self.stop_reference();
                } else {
                    self.start_range();
                }
            }
            ui.vertical(|ui| {
                ui.set_width(232.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                if let Some((low, high, start)) = self.accepted {
                    ui.colored_label(
                        BLUE,
                        format!("Highest: {} → Lowest: {}", dsp::name(high), dsp::name(low)),
                    );
                    ui.small(format!("Estimate: {}", crate::range::range_estimate(low, high)))
                        .on_hover_text("Rough range-only comparison, not a confirmed voice type. Based on your matched endpoints; comfortable range and tone are not assessed. Overlapping labels are alternatives, not multiple confirmed voice types.");
                    ui.small(format!("Suggested practice note: {}", dsp::name(start)))
                        .on_hover_text("A matched note nearest the middle of this check, automatically set as your exercise target. Comfort is yours to judge; choose another target if it feels easier.");
                    if let Some((note,seconds)) = self.accepted_sustain {
                        ui.small(format!("Highest sustain: {} · {seconds:.1}s",dsp::name(note)));
                    }
                } else {
                    ui.label("Your range: not checked");
                    ui.small("Highest: — · Lowest: —");
                }
            });
            ui.separator();
            if !self.playback_only
                && ui
                    .add(
                        egui::Button::new(if self.capture.is_some() { "■" } else { "▶" })
                            .min_size(Vec2::new(36.0, 30.0)),
                    )
                    .on_hover_text(if self.capture.is_some() {
                        "Stop microphone (Space)"
                    } else {
                        "Start microphone (Space)"
                    })
                    .clicked()
            {
                if self.capture.is_some() {
                    self.stop_capture();
                } else {
                    self.start_capture();
                }
            }
            let listening = self.capture.is_some();
            let color = if listening { GREEN } else { Color32::LIGHT_RED };
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 5.0, color);
            ui.colored_label(
                color,
                if self.playback_only {
                    "Playback only"
                } else if listening {
                    "Listening"
                } else {
                    "Stopped"
                },
            );

        });
        ui.horizontal_wrapped(|ui| {
            let live = self.latest.as_ref();
            let hz = live
                .filter(|f| f.confidence >= 0.85 && !f.clipped)
                .and_then(|f| f.hz);
            ui.label(
                RichText::new(
                    hz.map(|h| dsp::name(dsp::midi(h).round() as i32))
                        .unwrap_or("—".into()),
                )
                .size(20.0)
                .color(BLUE),
            );
            let error = hz
                .zip(self.active_target())
                .map(|(h, note)| dsp::cents(h, note));
            if let Some(cents) = error {
                ui.label(format!(
                    "{} · {cents:+.0} cents",
                    if self.range.is_some() && cents.abs() <= crate::range::MATCH_CENTS {
                        "Close enough"
                    } else if cents.abs() <= 15.0 {
                        "On target"
                    } else if cents > 0.0 {
                        "Above target"
                    } else {
                        "Below target"
                    }
                ));
            } else {
                ui.label("Sing to see your note");
            }
            ui.small(format!(
                "Voice {} · error {} · periodicity {}",
                hz.map(|h| format!("{h:.1} Hz")).unwrap_or("—".into()),
                error
                    .map(|c| format!("{c:+.0} cents"))
                    .unwrap_or("—".into()),
                live.map(|f| format!("{:.0}%", f.confidence * 100.0))
                    .unwrap_or("—".into())
            ));
        });
        if let Some(e) = self.error.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(Color32::from_rgb(255, 168, 142), e);
                if ui.small_button("Dismiss").clicked() {
                    self.error = None;
                }
            });
        }
        if self.range.is_some() {
            self.range_ui(ui);
        }
    }

    fn range_ui(&mut self, ui: &mut egui::Ui) {
        let r = self.range.as_ref().unwrap();
        ui.horizontal_wrapped(|ui| {
            if let Some(note) = r.target {
                ui.colored_label(BLUE, dsp::name(note));
            }
            let cue = if r.phase == RangePhase::Baseline {
                "Sing an easy note"
            } else if self.range_resume_at.is_some() {
                "Take a breath"
            } else if let Some(cue) = r.pacing_cue(self.now(), self.range_reference_ready_at) {
                cue
            } else if self.range_piano_finished {
                "Piano finished · Replay note to retry"
            } else if !self.player.ready {
                "Reference not ready"
            } else {
                "Hold in green · 1s"
            };
            ui.add(
                egui::ProgressBar::new(r.hold.min(1.0))
                    .desired_width(160.0)
                    .text(cue),
            );
            ui.small(match r.phase {
                RangePhase::Baseline => "Finding your starting note",
                RangePhase::ConfirmStart => "Confirm your starting note",
                RangePhase::Higher => "Exploring higher",
                RangePhase::UpperEdge => "Smaller steps higher",
                RangePhase::Lower => "Exploring lower",
                RangePhase::LowerEdge => "Smaller steps lower",
                RangePhase::Complete => "Complete",
            });
            if r.target.is_some() {
                ui.small("One-second match · anywhere in green");
            }
        });
        ui.small(r.hint);
        if self.range_piano_finished && r.holding_match() {
            ui.small("Piano stopped after 3 runs · keep holding until you release.");
        }
        if matches!(r.phase, RangePhase::Higher | RangePhase::UpperEdge) {
            ui.small(
                "3 unanswered repetitions → lower; singing above baseline gets one extra set.",
            );
        }
        if matches!(r.phase, RangePhase::Lower | RangePhase::LowerEdge) {
            ui.small(
                "3 unanswered runs: lower toward your start; below it, finish with matched notes.",
            );
        }
        let phase = r.phase;
        ui.horizontal_wrapped(|ui| {
            ui.small("Use headphones; stay in your relaxed usual voice.");
            let guided = matches!(
                phase,
                RangePhase::Higher
                    | RangePhase::UpperEdge
                    | RangePhase::Lower
                    | RangePhase::LowerEdge
            );
            if ui
                .add_enabled(
                    guided,
                    egui::Button::new(
                        if matches!(phase, RangePhase::Higher | RangePhase::UpperEdge) {
                            "Struggling — go lower"
                        } else {
                            "Struggling — finish"
                        },
                    )
                    .small(),
                )
                .clicked()
            {
                let event = self.range.as_mut().unwrap().limit();
                self.range_event(event);
            }
            if ui
                .add_enabled(
                    self.range.as_ref().is_some_and(|r| r.target.is_some()),
                    egui::Button::new("Replay note").small(),
                )
                .clicked()
            {
                self.range_reference();
            }
            if ui
                .add_enabled(
                    self.range.as_ref().is_some_and(|r| r.matched.len() >= 2),
                    egui::Button::new("Finish").small(),
                )
                .clicked()
            {
                self.finish_range();
            }
        });
    }
    fn monitor(&mut self, ui: &mut egui::Ui, height: f32) {
        ui.strong(if self.range.is_some() {
            "Panel · Voice setup"
        } else if self.config.exercise == Exercise::Melody {
            "Panel · Melody + your voice"
        } else if self.review_singing && !self.guides.is_empty() {
            "Panel · Your singing + piano target"
        } else if self.guides.is_empty() {
            "Panel · Your voice"
        } else {
            "Panel · Your voice + example"
        });
        crate::monitor::tabs(ui, &mut self.view);
        if self.config.exercise == Exercise::Melody && self.range.is_none() {
            crate::monitor::melody_path(
                ui,
                &self.melody,
                self.now(),
                height,
                self.view,
                self.latest.as_ref(),
            );
            return;
        }
        let latest = self
            .history
            .back()
            .map_or(0.0, |f| f.time)
            .max(self.reference.back().map_or(0.0, |f| f.time));
        let first = self
            .history
            .front()
            .map(|f| f.time)
            .into_iter()
            .chain(self.reference.front().map(|f| f.time))
            .fold(latest, f64::min);
        let window = if self.view == View::Spectrum {
            self.spectrum_secs as f64
        } else {
            3.0
        };
        let max = (latest - first - window).max(0.0);
        self.seconds_back = self.seconds_back.min(max);
        if max > 0.0 {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut self.seconds_back, 0.0..=max)
                        .fixed_decimals(2)
                        .text("Seconds back"),
                );
                if ui.small_button("Latest").clicked() {
                    self.seconds_back = 0.0;
                }
            });
        }
        if self
            .range
            .as_ref()
            .is_none_or(|r| r.phase != RangePhase::Baseline)
        {
            self.reference_history_ui(ui);
        }
        let end = latest - self.seconds_back;
        let selected: Vec<&Guide> = if self
            .range
            .as_ref()
            .is_some_and(|r| r.phase == RangePhase::Baseline)
        {
            Vec::new()
        } else if self.view == View::Full {
            self.guides.iter().collect()
        } else if let Some(index) = self.guide {
            self.guides.get(index).into_iter().collect()
        } else if self.review_singing
            || (self.view != View::Spectrum
                && self.range.is_none()
                && self.config.exercise == Exercise::Transition)
        {
            self.guides
                .iter()
                .rev()
                .find(|g| !self.review_singing || !g.voice.is_empty())
                .into_iter()
                .collect()
        } else {
            let current = &self.guides[self.sequence_start.min(self.guides.len())..];
            if current.is_empty() {
                self.guides.last().into_iter().collect()
            } else {
                current.iter().collect()
            }
        };
        let selected: Vec<_> = selected
            .into_iter()
            .filter(|g| !self.review_singing || !g.voice.is_empty())
            .collect();
        if self.view == View::Spectrum {
            egui::CollapsingHeader::new("Zoom and clarity").show(ui, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.spectrum_secs, 0.25..=3.0)
                        .logarithmic(true)
                        .text("Time detail (seconds)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.spectrum_low, 65.0..=3500.0)
                        .logarithmic(true)
                        .text("Lowest frequency (Hz)"),
                );
                self.spectrum_high = self.spectrum_high.max(self.spectrum_low + 50.0);
                ui.add(
                    egui::Slider::new(&mut self.spectrum_high, self.spectrum_low + 50.0..=4000.0)
                        .logarithmic(true)
                        .text("Highest frequency (Hz)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.spectrum_gain, 0.0..=30.0)
                        .text("Amplify detail (dB)"),
                );
                if ui.small_button("Reset view controls").clicked() {
                    self.spectrum_secs = 1.0;
                    self.spectrum_low = 65.0;
                    self.spectrum_high = 2000.0;
                    self.spectrum_gain = 12.0;
                }
            });
            let voice: Vec<_> = self
                .history
                .iter()
                .filter(|f| f.time >= end - window && f.time <= end)
                .collect();
            let mut sequence = Vec::new();
            for (index, g) in selected.iter().enumerate() {
                for f in g
                    .review_frames(self.review_singing)
                    .iter()
                    .filter(|f| f.time < window && f.spectrum.is_some())
                {
                    let mut copy = f.clone();
                    copy.time += index as f64 * (window + 0.06);
                    sequence.push(copy);
                }
            }
            let reference: Vec<_> = sequence.iter().collect();
            let seconds = (selected.len() as f64 * window
                + selected.len().saturating_sub(1) as f64 * 0.06)
                .max(window);
            let label = if self.review_singing {
                format!("Captured singing · {} clips", selected.len())
            } else if selected.len() > 1 {
                format!("Examples · {} notes · pinned", selected.len())
            } else {
                format!(
                    "Example · {} · pinned",
                    selected.first().map(|g| g.label.as_str()).unwrap_or("—")
                )
            };
            let bounds = (self.spectrum_low, self.spectrum_high);
            let gain = self.spectrum_gain;
            if ui.available_width() >= 680.0 {
                ui.columns(2, |cols| {
                    cols[0].label(
                        RichText::new(&label)
                            .color(if self.review_singing { BLUE } else { PURPLE })
                            .strong(),
                    );
                    crate::monitor::spectrum(
                        &mut cols[0],
                        &reference,
                        0.0..seconds,
                        height,
                        bounds,
                        gain,
                        !self.review_singing,
                    );
                    cols[1].label(RichText::new("Your voice").color(BLUE).strong());
                    crate::monitor::spectrum(
                        &mut cols[1],
                        &voice,
                        end - window..end,
                        height,
                        bounds,
                        gain,
                        false,
                    );
                });
            } else {
                ui.label(
                    RichText::new(label)
                        .color(if self.review_singing { BLUE } else { PURPLE })
                        .strong(),
                );
                crate::monitor::spectrum(
                    ui,
                    &reference,
                    0.0..seconds,
                    height * 0.5,
                    bounds,
                    gain,
                    !self.review_singing,
                );
                ui.label(RichText::new("Your voice").color(BLUE).strong());
                crate::monitor::spectrum(
                    ui,
                    &voice,
                    end - window..end,
                    height * 0.5,
                    bounds,
                    gain,
                    false,
                );
            }
            ui.small(format!("Voice: {window:.2}s · references: {seconds:.2}s · {:.0}–{:.0} Hz · gain +{gain:.0} dB",bounds.0,bounds.1));
            ui.small("Stop to review · hover for measurements");
        } else {
            let live_singing = self.review_singing
                && self.range.is_some()
                && self.view == View::Compact
                && self.guide.is_none();
            let voice: VecDeque<_> = self
                .history
                .iter()
                .filter(|f| f.time >= end - 3.0 && f.time <= end)
                .cloned()
                .collect();
            let clips: Vec<_> = selected
                .iter()
                .map(|g| g.review_frames(self.review_singing))
                .collect();
            let pinned = if self.view == View::Full || self.review_singing {
                crate::monitor::overview_pitch(&clips, end)
            } else {
                crate::monitor::pinned_pitch(&clips, end)
            };
            let mut indicators = Vec::new();
            if self.review_singing && !live_singing {
                let slot = 3.0 / selected.len().max(1) as f64;
                for (i, g) in selected.iter().enumerate() {
                    if let (Some(first), Some(last)) = (g.voice.first(), g.voice.last()) {
                        let start = end - 3.0 + i as f64 * slot;
                        indicators.push(crate::monitor::PitchGuide {
                            note: g.note,
                            start,
                            end: start
                                + (last.time - first.time) / (last.time - first.time + 0.05)
                                    * slot
                                    * 0.92,
                        });
                    }
                }
            }
            let empty = VecDeque::new();
            crate::monitor::pitch_monitor_guided(
                ui,
                if self.review_singing && !live_singing {
                    &pinned
                } else {
                    &voice
                },
                if self.review_singing { &empty } else { &pinned },
                (
                    self.active_target()
                        .map(|note| crate::monitor::PitchTarget {
                            note,
                            tolerance_cents: if self.range.is_some() {
                                crate::range::MATCH_CENTS
                            } else {
                                15.0
                            },
                        }),
                    &indicators,
                ),
                self.view == View::Full,
                height,
                end,
            );
            ui.small(if live_singing {
                "Blue = your live voice · green = target · click a thumbnail to review"
            } else if self.review_singing {
                "Blue = captured singing · faint purple = guide note only"
            } else {
                "Blue = microphone · purple = piano reference · green = target"
            });
            if height >= 180.0 {
                ui.small(if self.view == View::Full {
                    "Full scale: C2–C6 · saved clips fitted together for review"
                } else {
                    "Compact hides notes between distant ranges · hover the gap label for distance"
                });
            }
        }
    }
    fn reference_history_ui(&mut self, ui: &mut egui::Ui) {
        if self.guides.is_empty() {
            return;
        }
        ui.horizontal(|ui| {
            ui.small(if self.review_singing {
                "Saved singing · faint purple = guide note"
            } else {
                "Saved guides · voice + piano reference"
            });
            if self.guide.is_some() && ui.small_button("All notes").clicked() {
                self.guide = None;
            }
        });
        let spectrum = self.view == View::Spectrum;
        let gain = self.spectrum_gain;
        let low = self.spectrum_low;
        let high = self.spectrum_high;
        let selected = self.guide.unwrap_or_else(|| {
            self.guides
                .iter()
                .rposition(|g| !self.review_singing || !g.voice.is_empty())
                .unwrap_or(0)
        });
        let singing = self.review_singing;
        egui::ScrollArea::horizontal()
            .id_salt("saved_guides")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (order, (index, g)) in self.guides.iter_mut().enumerate()
                        .filter(|(_, g)| !singing || !g.voice.is_empty()).enumerate() {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(96.0, 58.0), egui::Sense::click());
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                true,
                                format!("Review {}", g.review_label(singing)),
                            )
                        });
                        if ui.is_rect_visible(rect) {
                            let p = ui.painter_at(rect);
                            let frames = if singing { &g.voice } else { &g.frames };
                            let color = if singing { BLUE } else { PURPLE };
                            let ink = if self.view == View::Full || index == selected {
                                color
                            } else {
                                Color32::GRAY
                            };
                            p.rect_filled(rect, 5.0, Color32::from_rgb(24, 30, 42));
                            p.text(
                                Pos2::new(rect.center().x, rect.top() + 9.0),
                                egui::Align2::CENTER_CENTER,
                                format!("{} · {}", order + 1, g.review_label(singing)),
                                egui::FontId::proportional(11.0),
                                ink,
                            );
                            let plot = egui::Rect::from_min_max(
                                rect.min + Vec2::new(5.0, 20.0),
                                rect.max - Vec2::new(5.0, 5.0),
                            );
                            if spectrum {
                                let key = (frames.len(), low, high, gain);
                                if g.texture_key != Some(key) {
                                    let mut pixels = egui::ColorImage::new(
                                        [96, 32],
                                        crate::monitor::spectral_color(-120.0, gain),
                                    );
                                    let duration = frames.last().map_or(1.0, |f| f.time + 0.05);
                                    for x in 0..96 {
                                        let t = x as f64 / 96.0 * duration;
                                        if let Some(bins) = frames
                                            .iter()
                                            .filter(|f| f.spectrum.is_some())
                                            .min_by(|a, b| {
                                                (a.time - t).abs().total_cmp(&(b.time - t).abs())
                                            })
                                            .filter(|f| (f.time - t).abs() < 0.1)
                                            .and_then(|f| f.spectrum.as_ref())
                                        {
                                            for y in 0..32 {
                                                let freq =
                                                    high * (low / high).powf(y as f32 / 31.0);
                                                let bin = ((freq / 65.0).ln()
                                                    / (4000.0_f32 / 65.0).ln()
                                                    * 63.0)
                                                    .round()
                                                    as usize;
                                                pixels[(x, y)] = crate::monitor::spectral_color(
                                                    bins[bin.min(63)],
                                                    gain,
                                                );
                                            }
                                        }
                                    }
                                    if let Some(texture) = &mut g.texture {
                                        texture.set(pixels, egui::TextureOptions::NEAREST);
                                    } else {
                                        g.texture = Some(ui.ctx().load_texture(
                                            format!("guide_{index}"),
                                            pixels,
                                            egui::TextureOptions::NEAREST,
                                        ));
                                    }
                                    g.texture_key = Some(key);
                                }
                                if let Some(texture) = &g.texture {
                                    p.image(
                                        texture.id(),
                                        plot,
                                        egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                }
                            } else {
                                let mut previous: Option<(Pos2, f64)> = None;
                                let duration = frames.last().map_or(1.0, |f| f.time + 0.05);
                                let notes: Vec<_> =
                                    frames.iter().filter(|f| crate::melody::reliable(f)).filter_map(|f| f.hz.map(dsp::midi)).collect();
                                let bottom =
                                    notes.iter().copied().fold(g.note as f32, f32::min) - 0.5;
                                let top = notes.iter().copied().fold(g.note as f32, f32::max) + 0.5;
                                if singing && !frames.is_empty() {
                                    let target_y = egui::lerp(plot.bottom()..=plot.top(), (g.note as f32 - bottom) / (top - bottom));
                                    p.line_segment([Pos2::new(plot.left(), target_y), Pos2::new(plot.right(), target_y)], Stroke::new(1.5_f32, PURPLE.gamma_multiply(0.65)));
                                }
                                for f in frames {
                                    if let Some(hz) = f.hz.filter(|_| crate::melody::reliable(f)) {
                                        let point = Pos2::new(
                                            plot.left() + (f.time / duration) as f32 * plot.width(),
                                            egui::lerp(
                                                plot.bottom()..=plot.top(),
                                                (dsp::midi(hz) - bottom) / (top - bottom),
                                            ),
                                        );
                                        if let Some((prev, t)) = previous {
                                            if f.time - t > 0.0
                                                && f.time - t < 0.025
                                                && (point.y - prev.y).abs() <= 2.0 / (top - bottom) * plot.height()
                                            {
                                                p.line_segment(
                                                    [prev, point],
                                                    Stroke::new(1.5_f32, ink),
                                                );
                                            }
                                        }
                                        previous = Some((point, f.time));
                                    } else {
                                        previous = None;
                                    }
                                }
                            }
                            if self.view == View::Full || index == selected {
                                p.line_segment(
                                    [rect.left_bottom(), rect.right_bottom()],
                                    Stroke::new(2.0_f32, color),
                                );
                            }
                        }
                        let response = response.on_hover_text(if singing {
                            format!("Captured microphone · median reliable note: {} · piano target: {}. No voice means no reliable captured pitch.", g.review_label(true), g.label)
                        } else {
                            format!("Piano reference: {} · this is not evidence of singing.", g.label)
                        });
                        if response.clicked() {
                            if self.view == View::Full {
                                self.view = View::Compact;
                            }
                            self.guide = Some(index);
                        }
                    }
                });
            });
    }
    fn exercise_ui(&mut self, ui: &mut egui::Ui) {
        let before = self.config;
        egui::ComboBox::from_id_salt("exercise")
            .width(220.0)
            .selected_text(match self.config.exercise {
                Exercise::Attack => "Clean attacks (ah)",
                Exercise::Transition => "Note transitions (ah)",
                Exercise::Melody => "Melody Coach",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.config.exercise,
                    Exercise::Attack,
                    "Clean attacks (ah)",
                );
                ui.selectable_value(
                    &mut self.config.exercise,
                    Exercise::Transition,
                    "Note transitions (ah)",
                );
                ui.selectable_value(&mut self.config.exercise, Exercise::Melody, "Melody Coach");
            });
        if self.config.exercise != before.exercise {
            let selected = self.config.exercise;
            self.config.exercise = before.exercise;
            self.change_exercise(selected);
        }
        if self.config.exercise == Exercise::Melody {
            self.melody_controls_ui(ui);
            return;
        }
        ui.horizontal(|ui|{
            let text_width=(ui.available_width()-48.0).max(80.0);
            ui.vertical(|ui|{
                ui.set_width(text_width);
                ui.add(egui::Label::new(if self.config.exercise==Exercise::Transition{"Listen, then sing one continuous ‘ah’: hold for two seconds, move to the second note, and hold for two seconds. Then pause."}else if self.config.mode==Mode::Performance{"Listen, then sing the note for about two seconds. Slides are allowed."}else{"Listen, then sing ‘ah’ on the note for about two seconds. Start without sliding."}).wrap());
                ui.small(if self.config.exercise==Exercise::Transition{format!("Example · {} → {} · 2s each",dsp::name(self.config.target),dsp::name(self.config.target+self.config.interval))}else{format!("Example · {} · {:.1} Hz",dsp::name(self.config.target),dsp::hz(self.config.target as f32))});
            });
            if crate::monitor::play_button(ui,self.example.is_some()).clicked(){if self.example.is_some(){self.stop_reference();}else{self.begin_example();}}
        });
        let (cue, color) = if self.guides.len() <= self.sequence_start && self.feedback.is_none() {
            ("Start the example using the play button", BLUE)
        } else {
            self.status()
        };
        ui.colored_label(color, cue);
        ui.add_space(4.0);
        ui.strong("Exercise parameters");
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_label("Target")
                .width(65.0)
                .selected_text(dsp::name(self.config.target))
                .show_ui(ui, |ui| {
                    for note in 36..=84 {
                        ui.selectable_value(&mut self.config.target, note, dsp::name(note));
                    }
                });
            if self.config.exercise == Exercise::Attack {
                egui::ComboBox::from_id_salt("mode")
                    .width(105.0)
                    .selected_text(format!("{:?}", self.config.mode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.config.mode, Mode::Technique, "Technique");
                        ui.selectable_value(
                            &mut self.config.mode,
                            Mode::Performance,
                            "Performance",
                        );
                    });
            } else {
                egui::ComboBox::from_id_salt("interval")
                    .width(110.0)
                    .selected_text(format!("{:+} semitones", self.config.interval))
                    .show_ui(ui, |ui| {
                        for interval in [2, 5, 7, -2, -5, -7] {
                            if (36..=84).contains(&(self.config.target + interval)) {
                                ui.selectable_value(
                                    &mut self.config.interval,
                                    interval,
                                    format!(
                                        "{interval:+} → {}",
                                        dsp::name(self.config.target + interval)
                                    ),
                                );
                            }
                        }
                    });
                self.config.target = self.config.target.clamp(
                    36 - self.config.interval.min(0),
                    84 - self.config.interval.max(0),
                );
            }
        });
        if self.config != before && self.config.exercise == before.exercise {
            self.reset();
            if self.config.exercise == Exercise::Transition {
                self.spectrum_secs = 1.75;
            }
        }
        if self.config.exercise == Exercise::Transition {
            ui.small("Hold first note ≥250 ms · continuous voice");
            ui.small("Arrival ±15 cents · transition ≤80 ms");
        } else {
            if self.config.mode == Mode::Technique {
                ui.small("Start ±20 cents · settle ≤80 ms");
            }
            ui.small("Hold ±15 cents for 100 ms");
            ui.small("≥85% periodicity · ≥350 ms singing");
        }
        ui.small("Piano reference · 5 attempts · pause between each");
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{}/5 attempts", self.attempts.len()));
            for index in 0..5 {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                ui.painter().circle_filled(
                    rect.center(),
                    4.0,
                    if index < self.attempts.len() {
                        GREEN
                    } else {
                        Color32::from_gray(65)
                    },
                );
            }
            if ui.small_button("New drill").clicked() {
                self.reset();
            }
        });
    }
    fn feedback_ui(&self, ui: &mut egui::Ui) {
        ui.strong("Feedback & measurements");
        if self.config.exercise == Exercise::Melody {
            self.melody_feedback_ui(ui);
            return;
        }
        let measured = match &self.feedback {
            Some(Result::Measured(m)) => Some(m),
            _ => None,
        };
        match &self.feedback {
            None => {
                ui.label("Sing an attempt to see your feedback here.");
            }
            Some(Result::Unscored(reason)) => {
                ui.colored_label(
                    Color32::from_rgb(235, 190, 110),
                    "Not scored · insufficient confidence",
                );
                ui.label(reason);
            }
            Some(Result::Measured(m)) => {
                ui.label(RichText::new(&m.feedback).size(15.0));
                ui.strong("Attempt measurements");
            }
        }
        let labels = if self.config.exercise == Exercise::Attack {
            [
                "Initial error",
                "Settling time",
                "Settled error",
                "Sustained spread (SD)",
            ]
        } else {
            [
                "Starting pitch error",
                "Transition time",
                "Arrival pitch error",
                "Arrival spread (SD)",
            ]
        };
        let value = |v: Option<f32>, suffix: &str| {
            v.map(|v| format!("{v:.1}{suffix}")).unwrap_or("—".into())
        };
        egui::Grid::new("measurements")
            .num_columns(2)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                for (label, v, suffix) in [
                    (labels[0], measured.map(|m| m.initial), " cents"),
                    (labels[1], measured.and_then(|m| m.settling), " ms"),
                    (labels[2], measured.and_then(|m| m.settled), " cents"),
                    (labels[3], measured.and_then(|m| m.spread), " cents"),
                    ("Attempt score", measured.map(|m| m.score), " / 100"),
                ] {
                    ui.label(label);
                    ui.label(value(v, suffix));
                    ui.end_row();
                }
                ui.label("Mean periodicity");
                ui.label(
                    measured
                        .map(|m| format!("{:.0}%", m.confidence * 100.0))
                        .unwrap_or("—".into()),
                );
                ui.end_row();
            });
        if self.attempts.len() == 5 {
            let s = training::summarize(&self.attempts);
            ui.separator();
            ui.strong(format!("Reliability: {:.0}/100", s.reliability));
            ui.label(format!(
                "Mean {:.0} · spread {:.1} · best {:.0} · worst {:.0}",
                s.mean, s.spread, s.best, s.worst
            ));
            ui.label(format!(
                "{}: {:.0}%",
                if self.config.exercise == Exercise::Attack {
                    "Clean attacks"
                } else {
                    "Clean transitions"
                },
                s.clean as f32 * 20.0
            ));
        }
    }
    fn change_exercise(&mut self, selected: Exercise) {
        self.config.exercise = selected;
        self.range = None;
        self.reset();
        if selected == Exercise::Melody {
            self.view = View::Compact;
        }
        if selected == Exercise::Transition {
            self.spectrum_secs = 1.75;
        }
    }
    fn melody_elapsed(&self, fallback: f64) -> f64 {
        match self.melody.state {
            crate::melody::State::Recording { start } => self
                .latest
                .as_ref()
                .map_or(fallback - start, |f| f.time - start)
                .max(0.0),
            _ => 0.0,
        }
    }
    fn observe_melody_frame(&mut self, frame: &Frame) {
        use crate::melody::State;
        if self.config.exercise != Exercise::Melody || self.range.is_some() {
            return;
        }
        if matches!(self.melody.state, State::Idle | State::Ready)
            && matches!(self.workflow, Workflow::Listening)
            && !self.player.playing
            && crate::melody::reliable(frame)
        {
            // Voice-only starts use the first reliable captured frame as the
            // attempt epoch, so that first note is retained at time zero.
            self.melody.state = State::Recording { start: frame.time };
        }
        if !self.player.playing && matches!(self.workflow, Workflow::Listening) {
            self.melody.push(frame);
        }
    }
    fn poll_melody(&mut self, now: f64) {
        use crate::melody::State;
        if self.config.exercise != Exercise::Melody || self.range.is_some() {
            return;
        }
        match self.melody.state {
            State::Playback if self.player.ready => {
                let start = *self.melody.playback_start.get_or_insert(now);
                if now - start >= self.melody.duration() + 0.05 {
                    self.stop_reference();
                    self.melody.state = State::Ready;
                    // Playback frames were drained before this transition.
                    // Melody starts on subsequent voice, without a quiet gate.
                    self.workflow = if self.capture.is_some() {
                        Workflow::Listening
                    } else {
                        Workflow::Stopped
                    };
                }
            }
            State::Recording { .. } if self.melody_elapsed(now) >= self.melody.duration() + 0.3 => {
                self.melody.finish();
                self.workflow = if self.capture.is_some() {
                    Workflow::AwaitQuiet {
                        since: None,
                        after: now,
                    }
                } else {
                    Workflow::Stopped
                };
            }
            _ => {}
        }
    }
    fn melody_controls_ui(&mut self, ui: &mut egui::Ui) {
        use crate::melody::State;
        let active = matches!(
            self.melody.state,
            State::Playback | State::Ready | State::Recording { .. }
        );
        let before = (self.melody.pattern, self.melody.transpose);
        ui.add_enabled_ui(!active, |ui| {
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("melody_pattern")
                    .selected_text(if self.melody.pattern == 0 {
                        "Subida de cinco notas"
                    } else {
                        "Arpejo"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.melody.pattern, 0, "Subida de cinco notas");
                        ui.selectable_value(&mut self.melody.pattern, 1, "Arpejo");
                    });
                let maximum = if self.melody.pattern == 0 { 31 } else { 29 };
                self.melody.transpose = self.melody.transpose.min(maximum);
                ui.add(
                    egui::Slider::new(&mut self.melody.transpose, -12..=maximum)
                        .text("Transposição"),
                );
            });
        });
        if before != (self.melody.pattern, self.melody.transpose) {
            self.stop_reference();
            self.melody.reset_variation();
        }
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 48.0).max(80.0));
                ui.label("Listen to the piano, then sing when ready. Or sing to start directly. Use headphones.");
                ui.small(format!("Sequence {} / 4 · sing to start · Play gives the next", self.melody.variation + 1));
            });
            ui.add_enabled_ui(!active || self.melody.state == State::Playback, |ui| {
                if crate::monitor::play_button(ui, self.melody.state == State::Playback).clicked() {
                    if self.melody.state == State::Playback {
                        self.stop_reference();
                        self.melody.state = State::Idle;
                    } else {
                        self.begin_example();
                    }
                }
            });
        });
        if ui
            .add_enabled(active, egui::Button::new("Stop attempt"))
            .clicked()
        {
            self.stop_reference();
            if matches!(self.melody.state, State::Recording { .. }) {
                self.melody.finish();
            } else {
                self.melody.state = State::Idle;
            }
        }
        let status = match self.melody.state {
            State::Idle => "Ready · sing to start when Listening, or press play".into(),
            State::Ready => "Sing when ready · your voice starts the timeline".into(),
            State::Playback => if self.player.ready {
                "A ouvir referência"
            } else {
                "A preparar áudio"
            }
            .into(),
            State::Recording { .. } => format!(
                "Canta · {:.1} / {:.1}s",
                self.melody_elapsed(self.now()),
                self.melody.duration()
            ),
            State::Analyzing => "A analisar".into(),
            State::Results => "Resultado · podes repetir".into(),
            State::Error => "Erro · dados insuficientes".into(),
        };
        ui.strong(status);
        if let Some(e) = &self.melody.error {
            ui.colored_label(Color32::LIGHT_RED, e);
        }
        ui.add_enabled_ui(!active && self.melody.result.is_none(), |ui| {
            ui.collapsing("Tolerâncias de avaliação", |ui| {
                ui.add(
                    egui::Slider::new(&mut self.melody.tolerance, 15.0..=90.0)
                        .text("Afinação ± cents"),
                );
                ui.add(
                    egui::Slider::new(&mut self.melody.minimum_coverage, 0.5..=0.9)
                        .text("Cobertura mínima"),
                );
                ui.add(
                    egui::Slider::new(&mut self.melody.margin, 0.05..=0.2)
                        .text("Margem por limite (s)"),
                );
            });
        });
        if ui.small_button("New drill").clicked() {
            self.reset();
        }
    }
    fn melody_feedback_ui(&self, ui: &mut egui::Ui) {
        use crate::melody;
        if self.melody.result.is_none() {
            ui.label("Sing the melody to see feedback here.");
        }
        if let Some(r) = &self.melody.result {
            ui.heading(r.score.map_or_else(
                || "Score: — · insufficient data".into(),
                |score| format!("Score: {score:.0} / 100"),
            ));
            ui.small("Score = percentage of the full melody sung in tune.");
            ui.strong(&r.feedback);
            ui.label(format!(
                "Cobertura vocal fiável: {:.0}%",
                r.coverage * 100.0
            ));
            ui.label(format!(
                "Tempo afinado (melodia completa): {} · nota mais difícil: {}",
                melody::metric(r.score, "%"),
                r.hardest.map_or_else(
                    || "—".into(),
                    |i| format!("{} ({})", i + 1, dsp::name(self.melody.notes()[i].midi))
                )
            ));
            for (i, a) in r.notes.iter().enumerate() {
                ui.small(format!(
                    "{} · {} · cobertura {:.0}% · viés {} · erro absoluto {} · voz afinada {}",
                    i + 1,
                    dsp::name(self.melody.notes()[i].midi),
                    a.coverage * 100.0,
                    melody::metric(a.bias, "¢"),
                    melody::metric(a.absolute, "¢"),
                    melody::metric(a.in_tune.map(|v| v * 100.0), "%")
                ));
            }
        }
    }
    fn render(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| self.header(ui));
        egui::CentralPanel::default().show(ctx,|ui|{
            let available=ui.available_height();
            let id=ui.id().with(("monitor_layout",self.config.exercise as u8,self.view as u8,ui.available_width().round() as i32));
            let overhead=ctx.data(|data|data.get_temp::<f32>(id)).unwrap_or(320.0);
            let minimum=if self.view==View::Spectrum{120.0}else{160.0};
            let height=(available-overhead).max(minimum);
            let content=egui::ScrollArea::vertical().id_salt("practice_scroll").show(ui,|ui|{
                let top=ui.cursor().top();self.monitor(ui,height);ui.add_space(6.0);ui.separator();ui.add_space(6.0);
                if self.range.is_some(){ui.label("Exercise paused while you find your note — follow the voice check above.");}
                else{
                    ui.horizontal(|ui|{ui.heading("Exercise");ui.label(match self.config.exercise {Exercise::Attack=>"Start directly on one note",Exercise::Transition=>"Move cleanly between two notes",Exercise::Melody=>"Follow a short melody"});});
                    ui.add_space(6.0);
                    if ui.available_width()>=680.0{ui.columns(2,|columns|{columns[0].group(|ui|self.exercise_ui(ui));columns[1].group(|ui|self.feedback_ui(ui));});}
                    else{ui.group(|ui|self.exercise_ui(ui));ui.add_space(8.0);ui.group(|ui|self.feedback_ui(ui));}
                }
                ui.cursor().top()-top
            });
            let measured=content.inner-height;
            if (measured-overhead).abs()>0.5{ctx.data_mut(|data|data.insert_temp(id,measured));ctx.request_repaint();}
        });
        if self.settings {
            self.settings_ui(ctx);
        }
    }
}
impl eframe::App for Coach {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        ctx.request_repaint_after(Duration::from_millis(16));
        if !self.playback_only
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| {
                i.key_pressed(egui::Key::Space)
                    && !i.events.iter().any(|e| {
                        matches!(
                            e,
                            egui::Event::Key {
                                key: egui::Key::Space,
                                repeat: true,
                                ..
                            }
                        )
                    })
            })
        {
            if self.capture.is_some() {
                self.stop_capture();
            } else {
                self.start_capture();
            }
        }
        self.render(ctx);
    }
}
impl Drop for Coach {
    fn drop(&mut self) {
        self.player.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wait_guides(c: &mut Coach) {
        for g in &mut c.guides {
            if let Some(rx) = g.pending.take() {
                g.frames = rx.recv_timeout(Duration::from_secs(2)).unwrap();
            }
        }
    }
    fn render_frames(c: &mut Coach, size: [f32; 2]) -> egui::FullOutput {
        let ctx = egui::Context::default();
        Coach::configure(&ctx);
        let mut output = None;
        for _ in 0..6 {
            output = Some(ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(size[0], size[1]),
                    )),
                    ..Default::default()
                },
                |ctx| c.render(ctx),
            ));
        }
        output.unwrap()
    }
    fn text_bounds(output: &egui::FullOutput, label: &str) -> egui::Rect {
        output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text.contains(label) => {
                    Some(s.shape.visual_bounding_rect())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("Missing {label}"))
    }
    #[test]
    fn melody_exercise_uses_shared_layout_and_safe_lifecycle() {
        use crate::melody::State;
        for size in [[960.0, 760.0], [720.0, 640.0]] {
            let mut c = Coach::init(true);
            c.change_exercise(Exercise::Melody);
            let config = c.config;
            for state in [
                State::Idle,
                State::Playback,
                State::Recording { start: 0.0 },
                State::Results,
                State::Error,
            ] {
                c.melody.state = state;
                if state == State::Results {
                    c.melody.finish();
                }
                let output = render_frames(&mut c, size);
                for label in [
                    "Listen to the piano",
                    "Stop attempt",
                    "Azul tracejado",
                    "Melody Coach",
                    "Feedback & measurements",
                    "New drill",
                    "Compact",
                    "Full scale",
                    "Spectrogram",
                ] {
                    let rect = text_bounds(&output, label);
                    assert!(
                        rect.right() <= size[0] && rect.bottom() <= size[1],
                        "{label}: {rect:?}"
                    );
                }
                assert_eq!(c.config, config);
                assert!(c.attempts.is_empty());
            }
            for view in [View::Compact, View::Full, View::Spectrum] {
                c.view = view;
                let output = render_frames(&mut c, size);
                let path = text_bounds(&output, "Purple: piano path");
                assert!(path.right() <= size[0] && path.bottom() <= size[1]);
                assert_eq!(c.config, config);
            }
            c.melody.state = State::Recording { start: 10.0 };
            c.poll_melody(17.0);
            assert_eq!(c.melody.state, State::Results);
            assert!(c.melody.result.as_ref().unwrap().score.is_none());
            c.change_exercise(Exercise::Attack);
            assert_eq!(c.melody.state, State::Idle);
            assert!(!c.player.playing && c.melody.result.is_none());
            c.change_exercise(Exercise::Melody);
            c.stop_capture();
            assert_eq!(c.melody.state, State::Error);
            assert!(!c.player.playing);
            c.melody.reset();
            c.melody.transpose = 5;
            assert_eq!(c.melody.notes()[0].midi, 53);
            c.start_range();
            assert!(c.range.is_some());
            assert_eq!(c.melody.state, State::Idle);
            assert_eq!(c.config, config);
            assert!(c.attempts.is_empty());
        }
    }
    #[test]
    fn melody_capture_clock_aligns_voice_start_trace_and_completion() {
        use crate::melody::State;
        let mut c = Coach::init(true);
        c.change_exercise(Exercise::Melody);
        c.workflow = Workflow::Listening;
        c.capture_seen = 10.0;
        c.latest = Some(Frame {
            time: 11.6,
            hz: Some(dsp::hz(48.0)),
            confidence: 0.99,
            rms: 0.1,
            clipped: false,
            spectrum: None,
        });
        c.melody.state = State::Ready;
        c.poll_melody(10.0);
        assert_eq!(c.melody.state, State::Ready);
        let mut f = c.latest.clone().unwrap();
        c.observe_melody_frame(&f);
        assert_eq!(c.melody.state, State::Recording { start: 11.6 });
        assert_eq!(c.melody.frames[0].time, 0.0);
        f.time += 2.8;
        c.latest = Some(f.clone());
        c.observe_melody_frame(&f);
        assert!((c.melody_elapsed(12.8) - 2.8).abs() < 1e-6);
        assert!((c.melody.frames.last().unwrap().time - 2.8).abs() < 1e-6);
        c.poll_melody(16.4); // Wall time cannot end an unfinished capture timeline.
        assert!(matches!(c.melody.state, State::Recording { .. }));
        c.latest.as_mut().unwrap().time = 18.0;
        c.poll_melody(16.4);
        assert_eq!(c.melody.state, State::Results);
    }
    #[test]
    fn melody_voice_start_records_first_c3_and_respects_playback_gates() {
        use crate::melody::State;
        let mut c = Coach::init(true);
        c.change_exercise(Exercise::Melody);
        let mut f = Frame {
            time: 10.0,
            hz: Some(dsp::hz(48.0)),
            confidence: 0.99,
            rms: 0.1,
            clipped: false,
            spectrum: None,
        };
        for workflow in [
            Workflow::Stopped,
            Workflow::Reference,
            Workflow::Cooldown { until: 11.0 },
            Workflow::AwaitQuiet {
                since: None,
                after: 9.0,
            },
        ] {
            c.workflow = workflow;
            c.observe_melody_frame(&f);
            assert_eq!(c.melody.state, State::Idle);
            assert!(c.melody.frames.is_empty());
        }
        c.workflow = Workflow::Listening;
        c.player.playing = true;
        c.observe_melody_frame(&f);
        assert_eq!(c.melody.state, State::Idle);
        c.player.playing = false;
        for (confidence, clipped, rms) in [(0.2, false, 0.1), (0.99, true, 0.1), (0.99, false, 0.0)]
        {
            f.confidence = confidence;
            f.clipped = clipped;
            f.rms = rms;
            c.observe_melody_frame(&f);
            assert_eq!(c.melody.state, State::Idle);
        }
        f.confidence = 0.99;
        f.rms = 0.1;
        c.observe_melody_frame(&f);
        assert_eq!(c.melody.state, State::Recording { start: 10.0 });
        assert_eq!(c.melody.frames.len(), 1);
        assert_eq!(c.melody.frames[0].time, 0.0);
        assert_eq!(c.melody.frames[0].hz, f.hz);
        f.time = 10.01;
        c.observe_melody_frame(&f);
        assert!((c.melody.frames[1].time - 0.01).abs() < 1e-6);
        c.melody.finish();
        c.observe_melody_frame(&f);
        assert_eq!(c.melody.state, State::Results);
        assert!(c.attempts.is_empty());
    }
    #[test]
    fn melody_shows_live_c3_outside_an_attempt_without_scoring_it() {
        let mut c = Coach::init(true);
        c.change_exercise(Exercise::Melody);
        c.melody.transpose = 12; // C3 is below this melody's default chart bounds.
        for view in [View::Compact, View::Full, View::Spectrum] {
            c.view = view;
            c.latest = Some(Frame {
                time: c.now(),
                hz: Some(dsp::hz(48.0)),
                confidence: 0.99,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            });
            let output = render_frames(&mut c, [960.0, 760.0]);
            let live = text_bounds(&output, "Live C3");
            assert!(live.is_positive() && live.right() <= 960.0 && live.bottom() <= 760.0);
            assert!(c.melody.frames.is_empty() && c.melody.result.is_none());
        }
        c.latest.as_mut().unwrap().time = c.now() - 1.0;
        let output = render_frames(&mut c, [960.0, 760.0]);
        assert!(!output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::epaint::Shape::Text(text) if text.galley.text().contains("Live C3")
        )));
    }
    #[test]
    fn changing_exercise_cancels_melody_and_keeps_evidence_separate() {
        use crate::melody::State;
        let mut c = Coach::init(true);
        c.change_exercise(Exercise::Melody);
        for state in [
            State::Playback,
            State::Recording { start: 10.0 },
            State::Results,
        ] {
            c.melody.state = state;
            let f = Frame {
                time: 10.1,
                hz: Some(dsp::hz(48.0)),
                confidence: 0.99,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            };
            c.history.push_back(f.clone());
            c.melody.frames.push(f);
            c.player.playing = true; // Pending playback, without opening hardware.
            c.player.ready = true;
            c.change_exercise(Exercise::Transition);
            assert!(!c.player.playing && !c.player.ready);
            assert!(c.melody.frames.is_empty() && c.melody.result.is_none());
            assert!(c.segmenter.started_at().is_none());
            assert!(c.attempts.is_empty() && c.feedback.is_none());
            assert!(!c.history.is_empty());
            c.change_exercise(Exercise::Melody);
        }
        c.melody.state = State::Playback;
        c.player.ready = false;
        c.poll_melody(100.0);
        assert!(c.melody.playback_start.is_none());
        c.player.ready = true;
        c.poll_melody(101.0);
        assert_eq!(c.melody.playback_start, Some(101.0));
        c.poll_melody(108.0);
        assert_eq!(c.melody.state, State::Ready);
        assert!(!c.player.playing);
        c.poll_melody(109.0);
        assert_eq!(c.melody.state, State::Ready); // No voice: keep waiting.
        c.workflow = Workflow::Listening;
        c.poll_melody(110.0);
        assert_eq!(c.melody.state, State::Ready);
        // An ordinary exercise evaluator cannot accidentally score a melody.
        let voice: Vec<_> = (0..200)
            .map(|i| Frame {
                time: i as f64 * 0.01,
                hz: Some(dsp::hz(48.0)),
                confidence: 0.99,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            })
            .collect();
        assert!(
            matches!(training::evaluate(&voice,c.config),Result::Unscored(reason) if reason.contains("timeline"))
        );
    }
    #[test]
    fn essential_controls_fit_reference_layout_sizes() {
        for size in [
            [720.0, 720.0],
            [960.0, 720.0],
            [960.0, 760.0],
            [1040.0, 820.0],
        ] {
            for view in [View::Compact, View::Full, View::Spectrum] {
                for exercise in [Exercise::Attack, Exercise::Transition] {
                    let mut c = Coach::init(true);
                    c.view = view;
                    c.config.exercise = exercise;
                    let output = render_frames(&mut c, size);
                    for label in [
                        "Find my range",
                        "Your range: not checked",
                        "New drill",
                        "Mean periodicity",
                        "Exercise parameters",
                        if exercise == Exercise::Attack {
                            "Settling time"
                        } else {
                            "Transition time"
                        },
                    ] {
                        let rect = text_bounds(&output, label);
                        assert!(
                            rect.bottom() <= size[1] && rect.right() <= size[0],
                            "{view:?} {exercise:?} {label} overflows {size:?}: {rect:?}"
                        );
                    }
                    assert!(
                        text_bounds(&output, "Find my range").right()
                            < text_bounds(&output, "Your range: not checked").left()
                    );
                }
            }
        }
    }
    #[test]
    fn minimum_window_keeps_header_and_monitor_accessible() {
        for view in [View::Compact, View::Full, View::Spectrum] {
            let mut c = Coach::init(true);
            c.view = view;
            let output = render_frames(&mut c, [720.0, 640.0]);
            for label in [
                "Find my range",
                "Your range: not checked",
                "Compact",
                "Full scale",
                "Spectrogram",
            ] {
                let rect = text_bounds(&output, label);
                assert!(rect.bottom() <= 640.0 && rect.right() <= 720.0);
            }
        }
    }
    #[test]
    fn active_compact_range_shows_live_voice_and_thumbnail_selects_saved_review() {
        let mut c = Coach::init(true);
        c.add_guide(48, None);
        wait_guides(&mut c);
        let mut r = Range::new();
        r.target = Some(48);
        r.phase = RangePhase::Higher;
        c.range = Some(r);
        c.review_singing = true;
        c.view = View::Compact;
        c.history.push_back(Frame {
            time: c.now(),
            hz: Some(dsp::hz(48.0)),
            confidence: 0.99,
            rms: 0.1,
            clipped: false,
            spectrum: None,
        });
        c.guides[0].voice = vec![Frame {
            time: 0.0,
            hz: Some(dsp::hz(50.0)),
            confidence: 0.99,
            rms: 0.1,
            clipped: false,
            spectrum: None,
        }];
        let output = render_frames(&mut c, [960.0, 760.0]);
        assert!(text_bounds(&output, "Blue = your live voice").is_positive());
        assert!(text_bounds(&output, "•").is_positive());
        c.guide = Some(0);
        let output = render_frames(&mut c, [960.0, 760.0]);
        assert!(text_bounds(&output, "Blue = captured singing").is_positive());
        assert!(text_bounds(&output, "1 · D3").is_positive());
        assert_eq!(c.history.back().unwrap().hz, Some(dsp::hz(48.0)));
        assert_eq!(c.range.as_ref().unwrap().target, Some(48));
    }
    #[test]
    fn range_review_saves_only_matched_holds_and_hides_empty_prompts() {
        let mut c = Coach::init(true);
        c.add_guide(60, None);
        wait_guides(&mut c);
        let mut r = Range::new();
        r.target = Some(60);
        r.phase = RangePhase::ConfirmStart;
        c.range = Some(r);
        c.review_singing = true;
        c.player.playing = true;
        c.player.ready = true;
        c.range_reference_ready_at = Some(10.0);
        let frame = |time: f64, note: Option<f32>| Frame {
            time,
            hz: note.map(dsp::hz),
            confidence: 0.99,
            rms: if note.is_some() { 0.1 } else { 0.0 },
            clipped: false,
            spectrum: None,
        };
        for i in 0..100 {
            c.observe_range_frame(&frame(11.0 + i as f64 * 0.01, Some(48.0)));
        }
        assert!(
            c.guides[0].voice.is_empty(),
            "Approach must not enter the saved clip"
        );
        let mut matched_at = None;
        for i in 0..130 {
            let time = 12.0 + i as f64 * 0.01;
            c.observe_range_frame(&frame(time, Some(60.0)));
            if c.range.as_ref().unwrap().holding_match() {
                matched_at.get_or_insert(time);
            }
        }
        assert!(c.range.as_ref().unwrap().holding_match());
        assert_eq!(c.guides[0].voice_start, matched_at);
        for i in 0..1700 {
            c.observe_range_frame(&frame(13.3 + i as f64 * 0.01, Some(62.0)));
        }
        assert_eq!(c.guides[0].voice.len(), 1600);
        assert_eq!(c.guides[0].voice[0].time, 0.0);
        assert!(c.guides[0]
            .voice
            .iter()
            .all(|f| dsp::midi(f.hz.unwrap()) >= 59.9));
        assert_eq!(c.guides[0].review_label(true), "D4");
        assert_eq!(c.guides[0].review_label(false), "C4");
        assert!((dsp::midi(c.guides[0].frames[10].hz.unwrap()) - 60.0).abs() < 0.1);
        c.range = None;
        c.player.stop();
        c.add_guide(64, None); // An unanswered prompt must not create a singing entry.
        wait_guides(&mut c);
        for view in [View::Compact, View::Full, View::Spectrum] {
            c.view = view;
            c.review_singing = true;
            let output = render_frames(&mut c, [960.0, 760.0]);
            assert!(text_bounds(&output, "1 · D4").is_positive());
            if view != View::Spectrum {
                assert!(output.shapes.iter().any(|s| matches!(&s.shape,
                    egui::epaint::Shape::LineSegment { stroke, .. }
                    if stroke.color == PURPLE.gamma_multiply(0.65) && stroke.width == 1.5)));
            }

            assert!(!output.shapes.iter().any(|s| matches!(&s.shape,
                egui::epaint::Shape::Text(t) if t.galley.text().contains("No voice"))));
            c.review_singing = false;
            let output = render_frames(&mut c, [960.0, 760.0]);
            assert!(text_bounds(&output, "1 · C4").is_positive());
            assert!(text_bounds(&output, "2 · E4").is_positive());
        }
    }
    #[test]
    fn range_release_trims_saved_silence_while_live_history_continues() {
        let mut c = Coach::init(true);
        c.add_guide(48, None);
        let mut r = Range::new();
        r.target = Some(48);
        r.phase = RangePhase::ConfirmStart;
        c.range = Some(r);
        c.player.playing = true;
        c.player.ready = true;
        c.range_reference_ready_at = Some(10.0);
        c.review_singing = true;
        c.view = View::Compact;
        for i in 0..230 {
            let f = Frame {
                time: 11.0 + i as f64 * 0.01,
                hz: Some(dsp::hz(48.0)),
                confidence: 0.99,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            };
            c.history.push_back(f.clone());
            c.observe_range_frame(&f);
        }
        let end = c.history.back().unwrap().time;
        for i in 0..30 {
            let f = Frame {
                time: 13.3 + i as f64 * 0.01,
                hz: None,
                confidence: 0.0,
                rms: 0.0,
                clipped: false,
                spectrum: None,
            };
            c.history.push_back(f.clone());
            c.observe_range_frame(&f);
        }
        assert!(!c.range.as_ref().unwrap().holding_match());
        assert!(c.guides[0].voice.last().unwrap().hz.is_some());
        assert!(c.history.back().unwrap().time > end);
        let saved = &c.guides[0].voice;
        let fitted = crate::monitor::overview_pitch(&[saved], 20.0);
        let pitched: Vec<_> = fitted.iter().filter(|f| f.hz.is_some()).collect();
        assert!(pitched.last().unwrap().time - pitched[0].time > 2.5);
    }
    #[test]
    fn saved_guide_review_changes_neither_training_nor_voice_history() {
        let mut c = Coach::init(true);
        c.add_guide(48, None);
        c.add_guide(50, None);
        wait_guides(&mut c);
        c.history.extend(c.guides[0].frames.clone());
        let before: Vec<_> = c.history.iter().map(|f| (f.time, f.hz)).collect();
        let config = c.config;
        for view in [View::Compact, View::Full, View::Spectrum] {
            c.view = view;
            c.guide = Some(0);
            let output = render_frames(&mut c, [960.0, 760.0]);
            assert!(!output.shapes.is_empty());
            assert_eq!(c.config, config);
            assert_eq!(
                before,
                c.history.iter().map(|f| (f.time, f.hz)).collect::<Vec<_>>()
            );
            assert!(c.attempts.is_empty());
        }
        assert_eq!(c.guides[0].frames[0].time, 0.0);
    }
    #[test]
    fn completed_summary_fits_default_window() {
        let mut c = Coach::init(true);
        let m = training::Measured {
            score: 95.0,
            clean: true,
            initial: 5.0,
            settling: Some(0.0),
            settled: Some(2.0),
            spread: Some(3.0),
            confidence: 0.99,
            feedback: "Clean attack. Repeat it reliably.".into(),
        };
        c.feedback = Some(Result::Measured(m.clone()));
        c.attempts = vec![m; 5];
        for view in [View::Compact, View::Full, View::Spectrum] {
            c.view = view;
            let output = render_frames(&mut c, [960.0, 760.0]);
            for label in ["Reliability:", "New drill", "Mean periodicity"] {
                assert!(text_bounds(&output, label).bottom() <= 760.0);
            }
        }
    }
    #[test]
    fn drill_reset_preserves_history_and_guides() {
        let mut c = Coach::init(true);
        c.add_guide(48, None);
        wait_guides(&mut c);
        c.history.extend(c.guides[0].frames.clone());
        c.feedback = Some(Result::Unscored("unclear".into()));
        c.reset();
        assert_eq!(c.history.len(), 196);
        assert_eq!(c.guides.len(), 1);
        assert!(c.feedback.is_none());
        assert!(c.attempts.is_empty());
    }
    #[test]
    fn range_starts_from_voice_without_audio_or_a_target_and_preserves_the_drill() {
        let mut c = Coach::init(true);
        c.accepted = Some((44, 60, 52));
        c.config.target = 52;
        c.feedback = Some(Result::Unscored("keep prior feedback".into()));
        c.start_range();
        assert!(
            c.range_input_after <= c.now(),
            "Voice-only discovery should listen immediately"
        );
        assert!(!c.player.playing);
        assert!(c.example.is_none());
        assert!(c.guides.is_empty());
        assert_eq!(c.active_target(), None);
        assert_eq!(c.range.as_ref().unwrap().phase, RangePhase::Baseline);
        assert!(c.feedback.is_some());
        assert_eq!(c.config.target, 52);
        let cutoff = c.range_input_after;
        for i in 0..130 {
            c.observe_range_frame(&Frame {
                time: cutoff - 2.0 + i as f64 * 0.01,
                hz: Some(dsp::hz(48.0)),
                confidence: 1.0,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            });
        }
        assert_eq!(c.range.as_ref().unwrap().target, None);
        let output = render_frames(&mut c, [720.0, 640.0]);
        for label in [
            "Sing an easy note",
            "Finding your starting note",
            "Use headphones",
        ] {
            assert!(text_bounds(&output, label).bottom() < 640.0);
        }
        c.range = None;
        c.stop_reference();
        assert!(c.feedback.is_some());
        assert_eq!(c.config.target, 52);
    }
    #[test]
    fn range_piano_stops_after_three_runs_and_silent_hold_keeps_tracking() {
        let mut r = Range::new();
        let frame = |time: f64, voiced: bool| Frame {
            time,
            hz: voiced.then_some(dsp::hz(48.0)),
            confidence: if voiced { 0.99 } else { 0.0 },
            rms: if voiced { 0.1 } else { 0.0 },
            clipped: false,
            spectrum: None,
        };
        for i in 0..60 {
            r.observe(&frame(i as f64 * 0.01, true), false);
        }
        let mut c = Coach::init(true);
        c.range = Some(r);
        c.player.playing = true;
        c.player.ready = true;
        c.range_reference_ready_at = Some(10.0);
        for i in 0..130 {
            c.observe_range_frame(&frame(11.0 + i as f64 * 0.01, true));
        }
        assert!(c.range.as_ref().unwrap().holding_match());
        c.poll_range_reference(15.99);
        assert!(c.player.playing);
        c.poll_range_reference(16.0);
        assert!(!c.player.playing && c.range_piano_finished);
        let before = c.range.as_ref().unwrap().held_seconds();
        for i in 130..950 {
            c.observe_range_frame(&frame(11.0 + i as f64 * 0.01, true));
        }
        c.poll_range_reference(20.5);
        let r = c.range.as_ref().unwrap();
        assert!(r.holding_match());
        assert!(r.held_seconds() > before + 8.0);
        assert!(!c.player.playing);
        for i in 0..15 {
            c.observe_range_frame(&frame(20.5 + i as f64 * 0.01, false));
        }
        let r = c.range.as_mut().unwrap();
        assert!(!r.holding_match());
        assert!(r.resting(20.65));
        assert_eq!(r.advance(21.8), RangeEvent::ReferenceChanged);
        assert!(r.highest_sustain.unwrap().1 > 9.0);
    }
    #[test]
    fn queued_voice_from_before_reference_readiness_does_not_count() {
        let mut r = Range::new();
        for i in 0..130 {
            r.observe(
                &Frame {
                    time: i as f64 * 0.01,
                    hz: Some(dsp::hz(48.0)),
                    confidence: 1.0,
                    rms: 0.1,
                    clipped: false,
                    spectrum: None,
                },
                false,
            );
        }
        let mut c = Coach::init(true);
        c.range = Some(r);
        c.player.playing = true;
        c.player.ready = true;
        c.range_reference_ready_at = Some(10.0);
        for i in 0..150 {
            c.observe_range_frame(&Frame {
                time: 8.0 + i as f64 * 0.01,
                hz: Some(dsp::hz(48.0)),
                confidence: 1.0,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            });
        }
        assert_eq!(c.range.as_ref().unwrap().successes, 0);
        assert!(c.range.as_ref().unwrap().matched.is_empty());
        for i in 0..30 {
            c.observe_range_frame(&Frame {
                time: 11.0 + i as f64 * 0.01,
                hz: Some(dsp::hz(48.0)),
                confidence: 1.0,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            });
        }
        assert!(c.range.as_ref().unwrap().hold > 0.25);
        c.player.ready = false;
        c.observe_range_frame(&Frame {
            time: 11.3,
            hz: Some(dsp::hz(48.0)),
            confidence: 1.0,
            rms: 0.1,
            clipped: false,
            spectrum: None,
        });
        assert_eq!(c.range.as_ref().unwrap().hold, 0.0);
    }
    #[test]
    fn range_ui_shows_confirmation_and_direction_controls_at_minimum_size() {
        for phase in [
            RangePhase::ConfirmStart,
            RangePhase::Higher,
            RangePhase::UpperEdge,
            RangePhase::Lower,
            RangePhase::LowerEdge,
        ] {
            let mut c = Coach::init(true);
            let mut r = Range::new();
            r.phase = phase;
            r.target = Some(55);
            r.matched = vec![53, 55];
            c.range = Some(r);
            let output = render_frames(&mut c, [720.0, 640.0]);
            for label in [
                "One-second match",
                "Reference not ready",
                "Replay note",
                "Finish",
            ] {
                let bounds = text_bounds(&output, label);
                assert!(
                    bounds.bottom() < 640.0 && bounds.right() < 720.0,
                    "{phase:?}: {label} {bounds:?}"
                );
            }
        }
    }
    #[test]
    fn coaching_callouts_fit_wide_and_narrow_headers() {
        for size in [[960.0, 760.0], [720.0, 640.0]] {
            for (phase, title) in [
                (RangePhase::Higher, "Go higher"),
                (RangePhase::Lower, "Go lower"),
            ] {
                let mut c = Coach::init(true);
                let mut r = Range::new();
                r.phase = phase;
                r.target = Some(55);
                r.matched = vec![48, 55];
                c.range = Some(r);
                let output = render_frames(&mut c, size);
                let bounds = text_bounds(&output, title);
                assert!(
                    bounds.right() <= size[0] && bounds.bottom() < 220.0,
                    "{size:?}: {bounds:?}"
                );
                if size[0] == 960.0 {
                    assert!(bounds.left() > 600.0);
                }
            }
            let mut c = Coach::init(true);
            c.accepted = Some((45, 65, 55));
            c.accepted_sustain = Some((65, 4.2));
            let output = render_frames(&mut c, size);
            assert!(text_bounds(&output, "Complete").bottom() < 220.0);
            assert!(text_bounds(&output, "Highest sustain: F4").right() <= size[0]);
            let estimate = text_bounds(&output, "Estimate: Baritone");
            assert!(estimate.right() <= size[0] && estimate.bottom() < 220.0);
        }
    }
    #[test]
    fn callout_size_and_alignment_stay_stable_across_messages_and_widths() {
        for width in [720.0, 800.0, 960.0, 1200.0] {
            let mut previous: Option<egui::Rect> = None;
            for state in 0..7 {
                let mut c = Coach::init(true);
                let mut r = Range::new();
                r.phase = match state {
                    0 => RangePhase::Baseline,
                    1 => RangePhase::Higher,
                    2 => RangePhase::Lower,
                    _ => RangePhase::ConfirmStart,
                };
                r.target = Some(55);
                r.matched = vec![48, 55];
                if state == 3 {
                    for i in 0..210 {
                        r.observe_guided(
                            &Frame {
                                time: i as f64 * 0.01,
                                hz: Some(dsp::hz(55.0)),
                                confidence: 0.99,
                                rms: 0.1,
                                clipped: false,
                                spectrum: None,
                            },
                            true,
                            Some(0.0),
                        );
                    }
                }
                c.range = Some(r);
                if state == 4 {
                    c.range_resume_at = Some(999.0);
                }
                if state == 5 {
                    c.range = None;
                    c.accepted = Some((45, 65, 55));
                    c.accepted_sustain = Some((65, 4.2));
                }
                if state == 6 {
                    c.range = None;
                    c.accepted = None;
                }
                let output = render_frames(&mut c, [width, 760.0]);
                let card = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Rect(rect) if rect.fill == Color32::from_rgb(24, 38, 49) => {
                            Some(rect.rect)
                        }
                        _ => None,
                    })
                    .expect("Coaching card must be visible");
                assert!(
                    card.left() > width * 0.5 && card.right() <= width,
                    "{width} state {state}: {card:?}"
                );
                if let Some(prev) = previous {
                    assert!(
                        (prev.min - card.min).length() < 1.0
                            && (prev.size() - card.size()).length() < 1.0,
                        "{width} state {state}: {prev:?} -> {card:?}"
                    );
                }
                previous = Some(card);
                assert!(
                    (card.height() - 60.0).abs() < 0.1,
                    "Instruction cue stays twice the toolbar button height"
                );
            }
        }
    }
    #[test]
    fn direction_change_pauses_and_ignores_microphone_backlog() {
        let mut c = Coach::init(true);
        let mut r = Range::new();
        r.phase = RangePhase::Higher;
        r.target = Some(67);
        r.matched = vec![60, 65];
        let event = r.limit();
        c.range = Some(r);
        let before = c.now();
        c.range_event(event);
        assert!(c.range_resume_at.unwrap() >= before + 2.0);
        assert!(!c.player.playing);
        assert!(c.range_descending);
        for i in 0..200 {
            c.observe_range_frame(&Frame {
                time: before + i as f64 * 0.01,
                hz: Some(dsp::hz(63.0)),
                confidence: 0.99,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            });
        }
        assert_eq!(c.range.as_ref().unwrap().target, Some(63));
        assert_eq!(c.range.as_ref().unwrap().hold, 0.0);
        let output = render_frames(&mut c, [720.0, 640.0]);
        assert!(text_bounds(&output, "Breathe · then lower").bottom() < 220.0);
    }
    #[test]
    fn range_completion_applies_matched_middle() {
        let mut c = Coach::init(true);
        let mut r = Range::new();
        r.matched = vec![48, 50, 52, 46, 44];
        r.highest_sustain = Some((52, 3.2));
        c.range = Some(r);
        c.finish_range();
        assert_eq!(c.accepted, Some((44, 52, 48)));
        assert_eq!(c.accepted_sustain, Some((52, 3.2)));
        assert_eq!(c.config.target, 48);
        assert!(c.range.is_none());
        assert!(!c.player.playing);
        for size in [[960.0, 760.0], [720.0, 640.0]] {
            let output = render_frames(&mut c, size);
            assert!(text_bounds(&output, "Suggested practice note: C3").right() <= size[0]);
            assert!(text_bounds(&output, "Try C3 · target set").bottom() < 220.0);
            assert!(text_bounds(&output, "Highest sustain: E3").right() <= size[0]);
            assert_eq!(c.config.target, 48);
        }
    }
}
