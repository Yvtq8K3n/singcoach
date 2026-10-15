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
    pending: Option<std::sync::mpsc::Receiver<Vec<Frame>>>,
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
    sequence_start: usize,
    example: Option<Example>,
    range: Option<Range>,
    range_reference_ready_at: Option<f64>,
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
            sequence_start: 0,
            example: None,
            range: None,
            range_reference_ready_at: None,
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
        self.sequence_start = self.guides.len();
        self.guide = None;
        self.stop_reference();
        self.segmenter.clear();
        self.attempts.clear();
        self.feedback = None;
        self.workflow = if self.capture.is_some() {
            Workflow::AwaitQuiet {
                since: None,
                after: self.now(),
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
            pending: Some(receiver),
        });
        if self.guides.len() > 24 {
            self.guides.remove(0);
            self.sequence_start = self.sequence_start.saturating_sub(1);
        }
        self.guide = None;
    }
    fn start_range(&mut self) {
        let tail = if self.player.playing { 0.5 } else { 0.0 };
        self.sequence_start = self.guides.len();
        self.guide = None;
        self.stop_reference();
        self.segmenter.clear();
        self.range_reference_ready_at = None;
        self.range_input_after = self.now() + tail;
        self.range_resume_at = None;
        self.range_descending = false;
        self.range = Some(Range::new());
    }
    fn range_reference(&mut self) {
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
        let ready = self.player.playing
            && self.player.ready
            && self.range_reference_ready_at.is_some_and(|t| f.time >= t);
        if let Some(range) = &mut self.range {
            let event = range.observe_guided(f, ready, self.range_reference_ready_at);
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
        if !self.player.playing || !self.player.ready {
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
            match &mut self.workflow {
                Workflow::Cooldown { until } if f.time >= *until => {
                    self.workflow = Workflow::AwaitQuiet {
                        since: None,
                        after: *until,
                    };
                }
                Workflow::AwaitQuiet { since, after } => {
                    if f.time >= *after && f.rms < 0.008 {
                        let start = *since.get_or_insert(f.time);
                        if f.time - start >= 0.25 {
                            self.workflow = if self.attempts.len() >= 5 {
                                Workflow::Complete
                            } else {
                                Workflow::Listening
                            };
                        }
                    } else {
                        *since = None;
                    }
                }
                Workflow::Listening => {
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
        if self.range_resume_at.is_none() && self.player.playing && self.player.ready {
            if let (Some(range), Some(start)) = (&mut self.range, self.range_reference_ready_at) {
                let event = range.reference_elapsed(now - start, now, true);
                self.range_event(event);
            }
        }
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
        let (icon, title, message, color) = if let Some(range) = &self.range {
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
        } else if let Some((low, high, _)) = self.accepted {
            (
                InstructionIcon::Done,
                "Complete",
                format!(
                    "Highest: {} → lowest: {}. Well done!{}",
                    dsp::name(high),
                    dsp::name(low),
                    self.accepted_sustain
                        .map(|(note, seconds)| format!(
                            "\nHighest sustain: {} · {seconds:.1}s",
                            dsp::name(note)
                        ))
                        .unwrap_or_default()
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
                    ui.small(format!("Exercise starting note: {}", dsp::name(start)));
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
        if matches!(r.phase, RangePhase::Higher | RangePhase::UpperEdge) {
            ui.small(
                "3 unanswered repetitions → lower; singing above baseline gets one extra set.",
            );
        }
        if matches!(r.phase, RangePhase::Lower | RangePhase::LowerEdge) {
            ui.small("After 3 repetitions without a match, we finish with your matched notes.");
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
        } else if self.guides.is_empty() {
            "Panel · Your voice"
        } else {
            "Panel · Your voice + example"
        });
        crate::monitor::tabs(ui, &mut self.view);
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
        } else if self.view != View::Spectrum
            && self.range.is_none()
            && self.config.exercise == Exercise::Transition
        {
            self.guides.last().into_iter().collect()
        } else {
            let current = &self.guides[self.sequence_start.min(self.guides.len())..];
            if current.is_empty() {
                self.guides.last().into_iter().collect()
            } else {
                current.iter().collect()
            }
        };
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
                    .frames
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
            let label = if selected.len() > 1 {
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
                    cols[0].label(RichText::new(&label).color(PURPLE).strong());
                    crate::monitor::spectrum(
                        &mut cols[0],
                        &reference,
                        0.0..seconds,
                        height,
                        bounds,
                        gain,
                        true,
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
                ui.label(RichText::new(label).color(PURPLE).strong());
                crate::monitor::spectrum(
                    ui,
                    &reference,
                    0.0..seconds,
                    height * 0.5,
                    bounds,
                    gain,
                    true,
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
            let voice: VecDeque<_> = self
                .history
                .iter()
                .filter(|f| f.time >= end - 3.0 && f.time <= end)
                .cloned()
                .collect();
            let clips: Vec<_> = selected.iter().map(|g| g.frames.as_slice()).collect();
            let pinned = if self.view == View::Full {
                crate::monitor::overview_pitch(&clips, end)
            } else {
                crate::monitor::pinned_pitch(&clips, end)
            };
            crate::monitor::pitch_monitor(
                ui,
                &voice,
                &pinned,
                self.active_target()
                    .map(|note| crate::monitor::PitchTarget {
                        note,
                        tolerance_cents: if self.range.is_some() {
                            crate::range::MATCH_CENTS
                        } else {
                            15.0
                        },
                    }),
                self.view == View::Full,
                height,
                end,
            );
            ui.small(if self.active_target().is_none(){"Blue = your voice · hold an easy note to start · the piano joins afterward"}else{"Green = target · blue = microphone · purple = pinned example · session history retained"});
            if height >= 180.0 {
                ui.small(if self.view == View::Full {
                    "Full scale: C2–C6 · all saved guides fitted together · blue voice is live"
                } else {
                    "Compact hides notes between distant ranges · hover the gap label for distance"
                });
            }
        }
    }
    fn reference_history_ui(&mut self, ui: &mut egui::Ui) {
        if self.guides.len() < 2 {
            return;
        }
        ui.horizontal(|ui| {
            ui.small(if self.view == View::Full {
                "Saved guides · all shown · click for Compact review"
            } else {
                "Saved guides · click a note to review it"
            });
            if self.guide.is_some() && ui.small_button("All notes").clicked() {
                self.guide = None;
            }
        });
        let spectrum = self.view == View::Spectrum;
        let gain = self.spectrum_gain;
        let low = self.spectrum_low;
        let high = self.spectrum_high;
        let selected = self.guide.unwrap_or(self.guides.len() - 1);
        egui::ScrollArea::horizontal()
            .id_salt("saved_guides")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, g) in self.guides.iter_mut().enumerate() {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::new(96.0, 58.0), egui::Sense::click());
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                true,
                                format!("Review {}", g.label),
                            )
                        });
                        if ui.is_rect_visible(rect) {
                            let p = ui.painter_at(rect);
                            let ink = if self.view == View::Full || index == selected {
                                PURPLE
                            } else {
                                Color32::GRAY
                            };
                            p.rect_filled(rect, 5.0, Color32::from_rgb(24, 30, 42));
                            p.text(
                                Pos2::new(rect.center().x, rect.top() + 9.0),
                                egui::Align2::CENTER_CENTER,
                                format!("{} · {}", index + 1, g.label),
                                egui::FontId::proportional(11.0),
                                ink,
                            );
                            let plot = egui::Rect::from_min_max(
                                rect.min + Vec2::new(5.0, 20.0),
                                rect.max - Vec2::new(5.0, 5.0),
                            );
                            if spectrum {
                                let key = (g.frames.len(), low, high, gain);
                                if g.texture_key != Some(key) {
                                    let mut pixels = egui::ColorImage::new(
                                        [96, 32],
                                        crate::monitor::spectral_color(-120.0, gain),
                                    );
                                    let duration = g.frames.last().map_or(1.0, |f| f.time + 0.05);
                                    for x in 0..96 {
                                        let t = x as f64 / 96.0 * duration;
                                        if let Some(bins) = g
                                            .frames
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
                                let duration = g.frames.last().map_or(1.0, |f| f.time + 0.05);
                                let notes: Vec<_> = g
                                    .frames
                                    .iter()
                                    .filter_map(|f| f.hz.map(dsp::midi))
                                    .collect();
                                let bottom =
                                    notes.iter().copied().fold(g.note as f32, f32::min) - 0.5;
                                let top = notes.iter().copied().fold(g.note as f32, f32::max) + 0.5;
                                for f in &g.frames {
                                    if let Some(hz) = f.hz.filter(|_| f.confidence >= 0.85) {
                                        let point = Pos2::new(
                                            plot.left() + (f.time / duration) as f32 * plot.width(),
                                            egui::lerp(
                                                plot.bottom()..=plot.top(),
                                                (dsp::midi(hz) - bottom) / (top - bottom),
                                            ),
                                        );
                                        if let Some((prev, t)) = previous {
                                            if f.time - t < 0.025 {
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
                                    Stroke::new(2.0_f32, PURPLE),
                                );
                            }
                        }
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
            .selected_text(if self.config.exercise == Exercise::Attack {
                "Clean attacks (ah)"
            } else {
                "Note transitions (ah)"
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
            });
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
        if self.config != before {
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
    fn render(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| self.header(ui));
        egui::CentralPanel::default().show(ctx,|ui|{
            let available=ui.available_height();
            let id=ui.id().with(("monitor_layout",self.view as u8,ui.available_width().round() as i32));
            let overhead=ctx.data(|data|data.get_temp::<f32>(id)).unwrap_or(320.0);
            let minimum=if self.view==View::Spectrum{120.0}else{160.0};
            let height=(available-overhead).max(minimum);
            let content=egui::ScrollArea::vertical().id_salt("practice_scroll").show(ui,|ui|{
                let top=ui.cursor().top();self.monitor(ui,height);ui.add_space(6.0);ui.separator();ui.add_space(6.0);
                if self.range.is_some(){ui.label("Exercise paused while you find your note — follow the voice check above.");}
                else{
                    ui.horizontal(|ui|{ui.heading("Exercise");ui.label(if self.config.exercise==Exercise::Attack{"Start directly on one note"}else{"Move cleanly between two notes"});});
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
    }
}
