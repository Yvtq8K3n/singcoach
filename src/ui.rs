use crate::monitor::{View, BLUE, GREEN, PURPLE};
use crate::{
    audio::{self, Capture, Player},
    dsp::{self, Frame},
    training::{self, Config, Exercise, Mode, Range, RangePhase, Result, Segmenter},
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
    accepted: Option<(i32, i32, i32)>,
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
            accepted: None,
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
                until: self.now() + 0.5,
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
        self.sequence_start = self.guides.len();
        self.guide = None;
        self.stop_reference();
        self.segmenter.clear();
        let note = self.accepted.map(|r| r.2).unwrap_or(48);
        self.range = Some(Range::new(note));
        self.range_reference();
    }
    fn range_reference(&mut self) {
        self.segmenter.clear();
        if let Some(r) = &mut self.range {
            r.reset_hold();
            let n = r.target;
            self.add_guide(n, None);
            if let Err(e) = self.player.play(n) {
                self.error = Some(e);
                self.player.stop();
            } else {
                self.error = None;
            }
        }
    }
    fn finish_range(&mut self) {
        if let Some(r) = self.range.take() {
            if let Some(result) = r.recommendation() {
                self.accepted = Some(result);
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
                let changed = self
                    .range
                    .as_mut()
                    .unwrap()
                    .observe(&f, self.player.playing && self.player.ready);
                if changed {
                    let r = self.range.as_ref().unwrap();
                    let bound = (r.phase == RangePhase::Higher && r.matched.contains(&84))
                        || (r.phase == RangePhase::Lower && r.matched.contains(&36));
                    if bound {
                        if self.range.as_mut().unwrap().limit() {
                            self.finish_range();
                        } else {
                            self.range_reference();
                        }
                    } else {
                        self.range_reference();
                    }
                }
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
                .filter(|_| example.stage == 0 && elapsed >= 0.8)
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
            } else if elapsed >= if example.stage == 1 { 0.8 } else { 1.0 } {
                self.stop_reference();
            }
        }
        if let Some(r) = &self.range {
            if self.player.ready && self.player.playing {
                self.reference.push_back(Frame {
                    time: now,
                    hz: Some(dsp::hz(r.target as f32)),
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
        if self.range.is_some() {
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
            ui.separator();ui.label("Reference output");ui.colored_label(PURPLE,audio::route());ui.small("Pure sine tone. Use headphones during range checking.");
            ui.separator();ui.label("Session data");ui.small("No voice recordings, accounts or network services. Charts retain up to 10 minutes; up to 24 reference guides. Everything clears when you close the app.");
        });
        self.settings = open;
    }
    fn active_target(&self) -> i32 {
        if let Some(range) = &self.range {
            return range.target;
        }
        if self.config.exercise == Exercise::Transition
            && (self.example.as_ref().is_some_and(|e| e.stage == 1)
                || self
                    .segmenter
                    .started_at()
                    .is_some_and(|t| self.latest.as_ref().is_some_and(|f| f.time - t >= 0.8)))
        {
            self.config.target + self.config.interval
        } else {
            self.config.target
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
        ui.horizontal(|ui| {
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
                        format!("Lowest: {} — Highest: {}", dsp::name(low), dsp::name(high)),
                    );
                    ui.small(format!("Exercise starting note: {}", dsp::name(start)));
                } else {
                    ui.label("Your range: not checked");
                    ui.small("Lowest: — · Highest: —");
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
            let error = hz.map(|h| dsp::cents(h, self.active_target()));
            if let Some(cents) = error {
                ui.label(format!(
                    "{} · {cents:+.0} cents",
                    if cents.abs() <= 15.0 {
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
            ui.colored_label(BLUE, dsp::name(r.target));
            let cue = if self.player.ready {
                "Sing ‘ah’ and hold"
            } else {
                "Reference not ready"
            };
            ui.add(
                egui::ProgressBar::new(r.hold.min(1.0))
                    .desired_width(140.0)
                    .text(cue),
            );
            ui.small(match r.phase {
                RangePhase::Baseline => "Find an easy note",
                RangePhase::Higher => "Exploring higher",
                RangePhase::Lower => "Exploring lower",
            });
            if let (Some(low), Some(high)) = (r.matched.iter().min(), r.matched.iter().max()) {
                ui.small(format!("Matched {}–{}", dsp::name(*low), dsp::name(*high)));
            }
        });
        ui.horizontal_wrapped(|ui| {
            if self.range.as_ref().unwrap().phase == RangePhase::Baseline {
                for (label, delta) in [("Lower", -1), ("Higher", 1)] {
                    if ui.small_button(label).clicked() {
                        let r = self.range.as_mut().unwrap();
                        r.target = (r.target + delta).clamp(36, 84);
                        self.range_reference();
                    }
                }
            }
            if ui
                .add_enabled(
                    !self.range.as_ref().unwrap().matched.is_empty(),
                    egui::Button::new("Feels strained / can't reach").small(),
                )
                .clicked()
            {
                if self.range.as_mut().unwrap().limit() {
                    self.finish_range();
                } else {
                    self.range_reference();
                }
            }
            if ui.small_button("Replay note").clicked() {
                self.range_reference();
            }
            if ui.small_button("Finish").clicked() {
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
        self.reference_history_ui(ui);
        let end = latest - self.seconds_back;
        let selected: Vec<&Guide> = if let Some(index) = self.guide {
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
            let pinned = crate::monitor::pinned_pitch(&clips, end);
            crate::monitor::pitch_monitor(
                ui,
                &voice,
                &pinned,
                Some(self.active_target()),
                self.view == View::Full,
                height,
                end,
            );
            ui.small("Green = target · blue = microphone · purple = pinned example · session history retained");
            if height >= 180.0 {
                ui.small(if self.view == View::Full {
                    "Full scale: C2–C6 · hover for any note · labels adapt to available space"
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
            ui.small("Saved guides · click a note to review it");
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
                            let ink = if index == selected {
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
                            if index == selected {
                                p.line_segment(
                                    [rect.left_bottom(), rect.right_bottom()],
                                    Stroke::new(2.0_f32, PURPLE),
                                );
                            }
                        }
                        if response.clicked() {
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
                ui.add(egui::Label::new(if self.config.exercise==Exercise::Transition{"Listen, then sing one continuous ‘ah’: hold, move to the second note, and hold. Then pause."}else if self.config.mode==Mode::Performance{"Listen, then sing the note for one second. Slides are allowed."}else{"Listen, then sing ‘ah’ on the note for one second. Start without sliding."}).wrap());
                ui.small(if self.config.exercise==Exercise::Transition{format!("Example · {} → {} · 0.8s each",dsp::name(self.config.target),dsp::name(self.config.target+self.config.interval))}else{format!("Example · {} · {:.1} Hz",dsp::name(self.config.target),dsp::hz(self.config.target as f32))});
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
        ui.small("Sine reference · 5 attempts · pause between each");
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
        assert_eq!(c.history.len(), 96);
        assert_eq!(c.guides.len(), 1);
        assert!(c.feedback.is_none());
        assert!(c.attempts.is_empty());
    }
    #[test]
    fn range_completion_applies_matched_middle() {
        let mut c = Coach::init(true);
        let mut r = Range::new(48);
        r.matched = vec![48, 50, 52, 46, 44];
        c.range = Some(r);
        c.finish_range();
        assert_eq!(c.accepted, Some((44, 52, 48)));
        assert_eq!(c.config.target, 48);
        assert!(c.range.is_none());
        assert!(!c.player.playing);
    }
}
