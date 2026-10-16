//! Voice-first, briefly matched range exploration. Silence and unclear audio
//! never establish a boundary. Misses may reduce the step size; the singer
//! chooses when to change direction. Results are matched notes, not limits.
use crate::{
    dsp::{self, Frame},
    training::{median, sd},
};
pub const HOLD_SECONDS: f64 = 1.0;
const BASELINE_SECONDS: f64 = 0.4;
const WOBBLE_SECONDS: f64 = 0.1;
const QUIET_SECONDS: f64 = 0.25;
const RELEASE_SECONDS: f64 = 0.12;
pub const MATCH_CENTS: f32 = 90.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RangePhase {
    Baseline,
    ConfirmStart,
    Higher,
    UpperEdge,
    Lower,
    LowerEdge,
    Complete,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RangeEvent {
    None,
    ReferenceChanged,
    Complete,
}

pub struct Range {
    pub target: Option<i32>,
    pub phase: RangePhase,
    pub matched: Vec<i32>,
    pub hold: f32,
    pub successes: usize,
    pub trials: usize,
    pub hint: &'static str,
    cue_started: Option<f64>,
    pending_match: Option<(f64, f64)>,
    release_since: Option<f64>,
    sustain_seconds: f64,
    sustain_gap: f64,
    pub highest_sustain: Option<(i32, f64)>,
    baseline_note: Option<i32>,
    above_baseline: Vec<(f64, f32)>,
    last_above_baseline: Option<f64>,
    anchor: Option<i32>,
    baseline: Vec<(f64, f32)>,
    last_time: Option<f64>,
    window: Vec<(f64, Option<f32>)>,
    step: i32,
    quiet_since: Option<f64>,
    trial_start: Option<f64>,
    trial_frames: usize,
    valid_frames: usize,
    valid_seconds: f64,
    trial_clipped: bool,
}
impl Range {
    pub fn new() -> Self {
        Self {
            target: None,
            phase: RangePhase::Baseline,
            matched: Vec::new(),
            hold: 0.0,
            successes: 0,
            trials: 0,
            hint: "Sing an easy note in your usual voice; the piano will follow.",
            cue_started: None,
            pending_match: None,
            release_since: None,
            sustain_seconds: 0.0,
            sustain_gap: 0.0,
            highest_sustain: None,
            baseline_note: None,
            above_baseline: Vec::new(),
            last_above_baseline: None,
            anchor: None,
            baseline: Vec::new(),
            last_time: None,
            window: Vec::new(),
            step: 2,
            quiet_since: None,
            trial_start: None,
            trial_frames: 0,
            valid_frames: 0,
            valid_seconds: 0.0,
            trial_clipped: false,
        }
    }
    /// Playback retries discard partial evidence, but keep completed trials.
    pub fn reset_hold(&mut self) {
        self.cue_started = None;
        self.pending_match = None;
        self.release_since = None;
        self.sustain_seconds = 0.0;
        self.sustain_gap = 0.0;
        self.above_baseline.clear();
        self.last_above_baseline = None;
        self.hold = 0.0;
        self.window.clear();
        self.quiet_since = None;
        self.baseline.clear();
        self.trial_start = None;
        self.trial_frames = 0;
        self.valid_frames = 0;
        self.valid_seconds = 0.0;
        self.trial_clipped = false;
    }
    fn clear_trial(&mut self) {
        self.hold = 0.0;
        self.window.clear();
        self.quiet_since = None;
        self.trial_start = None;
        self.trial_frames = 0;
        self.valid_frames = 0;
        self.valid_seconds = 0.0;
        self.trial_clipped = false;
    }
    fn change(&mut self, note: i32, phase: RangePhase, hint: &'static str) -> RangeEvent {
        self.target = Some(note.clamp(36, 84));
        self.phase = phase;
        let last_above = self.last_above_baseline;
        self.successes = 0;
        self.trials = 0;
        self.reset_hold();
        self.last_above_baseline = last_above;
        self.hint = hint;
        RangeEvent::ReferenceChanged
    }
    fn complete(&mut self) -> RangeEvent {
        self.phase = RangePhase::Complete;
        self.target = None;
        self.reset_hold();
        self.hint = "Your confirmed range is ready.";
        RangeEvent::Complete
    }
    fn begin_lower(&mut self) -> RangeEvent {
        let Some(high) = self.matched.iter().max().copied() else {
            return RangeEvent::None;
        };
        self.anchor = Some(high);
        self.step = 2;
        if high <= 36 {
            return self.complete();
        }
        self.change(
            (high - 2).max(36),
            RangePhase::Lower,
            "Now follow the piano down. Hold a nearby note for one second.",
        )
    }
    fn confirmed(&mut self) -> RangeEvent {
        let note = self.target.expect("Guided trials have a target");
        if !self.matched.contains(&note) {
            self.matched.push(note);
        }
        self.anchor = Some(note);
        match self.phase {
            RangePhase::ConfirmStart | RangePhase::Higher | RangePhase::UpperEdge => {
                if note >= 84 {
                    self.begin_lower()
                } else {
                    self.change(
                        (note + self.step).min(84),
                        RangePhase::Higher,
                        "Matched. Follow the piano a little higher.",
                    )
                }
            }
            RangePhase::Lower | RangePhase::LowerEdge => {
                if note <= 36 {
                    self.complete()
                } else {
                    self.change(
                        (note - self.step).max(36),
                        RangePhase::Lower,
                        "Matched. Follow the piano a little lower.",
                    )
                }
            }
            _ => RangeEvent::None,
        }
    }
    fn smaller_step(&mut self) -> RangeEvent {
        let note = self.target.expect("Guided trials have a target");
        let anchor = self.anchor.unwrap_or(note);
        match self.phase {
            RangePhase::Higher if note - anchor > 1 => {
                self.step = 1;
                self.change(
                    anchor + 1,
                    RangePhase::UpperEdge,
                    "Let's use smaller steps. A one-second ‘ah’ is enough.",
                )
            }
            RangePhase::Lower if anchor - note == 2 => {
                self.step = 1;
                self.change(
                    anchor - 1,
                    RangePhase::LowerEdge,
                    "Let's use smaller steps. A one-second ‘ah’ is enough.",
                )
            }
            _ => RangeEvent::None,
        }
    }
    fn record(&mut self, success: bool) -> RangeEvent {
        if success && self.cue_started.is_some() {
            let matched_at = self.last_time.unwrap_or(0.0);
            let note = self.target.expect("Guided match has a target");
            if !self.matched.contains(&note) {
                self.matched.push(note);
            }
            self.anchor = Some(note);
            self.sustain_seconds = self
                .window
                .windows(2)
                .filter(|w| {
                    w.iter()
                        .all(|(_, p)| p.is_some_and(|c| c.abs() <= MATCH_CENTS + 0.01))
                })
                .map(|w| (w[1].0 - w[0].0).clamp(0.0, 0.025))
                .sum();
            self.update_sustain(note);
            self.pending_match = Some((matched_at, f64::INFINITY));
            self.hold = 1.0;
            return RangeEvent::None;
        }
        self.trials += 1;
        if success {
            self.successes += 1;
        }
        self.clear_trial();
        if success {
            return self.confirmed();
        }
        if !success && self.trials - self.successes >= 2 {
            let event = self.smaller_step();
            if event != RangeEvent::None {
                return event;
            }
        }
        self.hint =
            "Keep trying near the piano note; hold anywhere in the green band for one second.";
        RangeEvent::None
    }
    fn discover(&mut self, f: &Frame, valid: bool) -> RangeEvent {
        if !valid {
            if f.clipped
                || self
                    .baseline
                    .last()
                    .is_none_or(|(t, _)| f.time - t > WOBBLE_SECONDS)
            {
                self.baseline.clear();
                self.hold = 0.0;
            }
            return RangeEvent::None;
        }
        let pitch = dsp::midi(f.hz.unwrap());
        if !(36..=84).contains(&(pitch.round() as i32)) {
            self.baseline.clear();
            self.hold = 0.0;
            self.hint = "Choose an easy note within the supported C2–C6 span.";
            return RangeEvent::None;
        }
        if self
            .baseline
            .last()
            .is_some_and(|(_, p)| (pitch - p).abs() > 2.0)
        {
            self.baseline.clear();
        }
        self.baseline.push((f.time, pitch));
        self.baseline.retain(|(time, _)| f.time - time <= 0.6);
        let pitches: Vec<_> = self.baseline.iter().map(|(_, p)| *p).collect();
        if sd(&pitches) > 0.45 {
            self.baseline.clear();
            self.baseline.push((f.time, pitch));
            self.hold = 0.0;
            return RangeEvent::None;
        }
        let elapsed = f.time - self.baseline[0].0;
        let voiced = self
            .baseline
            .windows(2)
            .map(|w| {
                let dt = w[1].0 - w[0].0;
                if dt <= 0.025 {
                    dt
                } else {
                    0.0
                }
            })
            .sum::<f64>();
        self.hold = (elapsed / BASELINE_SECONDS)
            .min(voiced / (BASELINE_SECONDS * 0.8))
            .min(1.0) as f32;
        if elapsed >= BASELINE_SECONDS && voiced >= BASELINE_SECONDS * 0.8 {
            let note = median(&pitches).round() as i32;
            self.baseline_note = Some(note);
            return self.change(
                note,
                RangePhase::ConfirmStart,
                "Match the piano briefly. Close is enough; then follow the next note.",
            );
        }
        RangeEvent::None
    }
    /// Hear one second, then hold a matched note until release.
    /// After release, breathe for one second before moving.
    pub fn observe_guided(
        &mut self,
        f: &Frame,
        ready: bool,
        reference_started: Option<f64>,
    ) -> RangeEvent {
        if let Some((_, until)) = self.pending_match {
            if until.is_finite() {
                return RangeEvent::None;
            }
            if !ready || !f.time.is_finite() {
                self.release_since = None;
                self.last_time = None;
                self.sustain_seconds = 0.0;
                return RangeEvent::None;
            }
            let dt = self.last_time.map_or(0.0, |t| f.time - t);
            if dt <= 0.0 || dt > 0.025 {
                self.release_since = None;
                self.sustain_seconds = 0.0;
            }
            self.last_time = Some(f.time);
            let note = self.target.expect("Matched hold has a target");
            let valid = !f.clipped
                && f.rms.is_finite()
                && f.rms >= 0.008
                && f.confidence.is_finite()
                && f.confidence >= 0.85
                && f.hz.is_some_and(|hz| {
                    hz.is_finite() && hz > 0.0 && dsp::cents(hz, note).abs() <= MATCH_CENTS + 0.01
                });
            if valid {
                self.sustain_gap = 0.0;
                if (0.0..=0.025).contains(&dt) {
                    self.sustain_seconds += dt;
                }
                self.update_sustain(note);
            } else {
                self.sustain_gap += dt.clamp(0.0, 0.025);
                if self.sustain_gap > WOBBLE_SECONDS {
                    self.sustain_seconds = 0.0;
                }
            }
            // After a confirmed match, detect the end of singing rather than
            // demanding digital silence: room noise can remain above the gate.
            let released = !f.clipped
                && f.rms.is_finite()
                && f.confidence.is_finite()
                && (f.rms < 0.008 || f.hz.is_none() || f.confidence < 0.85);
            if released {
                let start = *self.release_since.get_or_insert(f.time);
                if f.time - start + 1e-6 >= RELEASE_SECONDS {
                    let rest = f.time;
                    self.pending_match = Some((rest, rest + 1.0));
                }
            } else {
                self.release_since = None;
            }
            return RangeEvent::None;
        }
        if self.phase != RangePhase::Baseline && ready {
            let Some(start) = reference_started else {
                return RangeEvent::None;
            };
            if f.time < start + 1.0 {
                return RangeEvent::None;
            }
            self.cue_started = Some(start);
        }
        self.observe(f, ready)
    }
    fn update_sustain(&mut self, note: i32) {
        match self.highest_sustain {
            Some((high, _)) if note < high => {}
            Some((high, seconds)) if note == high => {
                self.highest_sustain = Some((note, seconds.max(self.sustain_seconds)))
            }
            _ => self.highest_sustain = Some((note, self.sustain_seconds)),
        }
    }
    pub fn holding_match(&self) -> bool {
        self.pending_match
            .is_some_and(|(_, until)| !until.is_finite())
    }
    pub fn held_seconds(&self) -> f64 {
        self.sustain_seconds
    }
    pub fn resting(&self, now: f64) -> bool {
        self.pending_match
            .is_some_and(|(hold_until, until)| until.is_finite() && now >= hold_until)
    }
    pub fn advance(&mut self, now: f64) -> RangeEvent {
        if now.is_finite() && self.pending_match.is_some_and(|(_, until)| now >= until) {
            self.pending_match = None;
            return self.confirmed();
        }
        RangeEvent::None
    }
    pub fn pacing_cue(&self, now: f64, reference_started: Option<f64>) -> Option<&'static str> {
        if let Some((hold_until, until)) = self.pending_match {
            Some(if !until.is_finite() || now < hold_until {
                "Matched — finish gently"
            } else {
                "Matched — take a breath"
            })
        } else if reference_started.is_some_and(|start| now < start + 1.0) {
            Some("Listen to the piano")
        } else {
            None
        }
    }
    pub fn observe(&mut self, f: &Frame, ready: bool) -> RangeEvent {
        if self.phase == RangePhase::Complete {
            return RangeEvent::None;
        }
        if !f.time.is_finite() {
            self.reset_hold();
            self.last_time = None;
            return RangeEvent::None;
        }
        let dt = self.last_time.map_or(0.0, |t| f.time - t);
        if self.last_time.is_some() && !(0.0..=0.025).contains(&dt)
            || self.last_time.is_some() && dt == 0.0
        {
            self.reset_hold();
            self.hint = "Audio was interrupted. Start a fresh hold.";
        }
        self.last_time = Some(f.time);
        let valid = !f.clipped
            && f.confidence.is_finite()
            && f.confidence >= 0.85
            && f.rms.is_finite()
            && f.rms >= 0.008
            && f.hz.is_some_and(|hz| hz.is_finite() && hz > 0.0);
        if self.phase == RangePhase::Baseline {
            return self.discover(f, valid);
        }
        if !ready {
            self.reset_hold();
            return RangeEvent::None;
        }
        // A sustained attempt above the initial baseline keeps the climb open,
        // even when it misses the current piano target or waits for a retry.
        self.above_baseline.retain(|(t, _)| f.time - t <= 0.25);
        if valid {
            self.above_baseline.push((f.time, dsp::midi(f.hz.unwrap())));
        } else if f.clipped {
            self.above_baseline.clear();
        }
        let pitches: Vec<_> = self.above_baseline.iter().map(|(_, p)| *p).collect();
        if self.above_baseline.len() >= 16
            && self
                .baseline_note
                .is_some_and(|base| median(&pitches) > base as f32 + 0.25)
            && sd(&pitches) <= 0.5
            && valid
        {
            self.last_above_baseline = Some(f.time);
        }
        let quiet = f.rms.is_finite() && f.rms < 0.008;

        if quiet {
            self.hold = 0.0;
            self.window.clear();
            if self.trial_start.is_some() {
                let start = *self.quiet_since.get_or_insert(f.time);
                if f.time - start >= QUIET_SECONDS {
                    let measured = !self.trial_clipped
                        && self.valid_seconds >= 1.0
                        && self.valid_frames as f32 / self.trial_frames.max(1) as f32 >= 0.8;
                    if measured {
                        let event = self.record(false);
                        return event;
                    }
                    self.clear_trial();
                    self.hint = "Not counted: try a short clear ‘ah’, then pause.";
                }
            }
            return RangeEvent::None;
        }
        self.quiet_since = None;
        self.trial_start.get_or_insert(f.time);
        self.trial_frames += 1;
        self.trial_clipped |= f.clipped || !f.rms.is_finite() || !f.confidence.is_finite();
        if f.clipped {
            self.hint = "Input clipped. Lower microphone gain, pause, then retry.";
        }
        if valid {
            self.valid_frames += 1;
            self.valid_seconds += dt.clamp(0.0, 0.025);
        }
        let target = self.target.expect("Active guided phase has a target");
        // Judge the recent phrase, rather than restarting on each pitch wobble.
        // Silence, clipping and playback changes discard the window entirely.
        if f.clipped {
            self.window.clear();
            self.hold = 0.0;
        } else {
            let cents = valid.then(|| dsp::cents(f.hz.unwrap(), target));
            self.window.push((f.time, cents));
            self.window
                .retain(|(t, _)| f.time - t <= HOLD_SECONDS + 0.015);
            let pitches: Vec<_> = self.window.iter().filter_map(|(_, p)| *p).collect();
            let near = pitches
                .iter()
                .filter(|p| p.abs() <= MATCH_CENTS + 0.01)
                .count();
            let coverage = pitches.len() as f32 / self.window.len() as f32;
            let nearby = near as f32 / pitches.len().max(1) as f32;
            let centered = !pitches.is_empty() && median(&pitches).abs() <= MATCH_CENTS + 0.01;
            let elapsed = f.time - self.window[0].0;
            self.hold = if centered && coverage >= 0.8 && nearby >= 0.8 {
                (elapsed / HOLD_SECONDS).min(1.0) as f32
            } else {
                0.0
            };
            if elapsed + 1e-6 >= HOLD_SECONDS
                && self.hold >= 1.0 - 1e-5
                && cents.is_some_and(|p| p.abs() <= MATCH_CENTS + 0.01)
            {
                return self.record(true);
            }
        }
        if f.time - self.trial_start.unwrap() >= 4.0 {
            if !self.trial_clipped
                && self.valid_seconds >= 1.0
                && self.valid_frames as f32 / self.trial_frames as f32 >= 0.8
            {
                return self.record(false);
            }
            self.clear_trial();
            self.hint = "Keep singing a clearer note in the green band.";
        }
        RangeEvent::None
    }
    /// Three ready piano repetitions skip an unanswered prompt. Higher
    /// singing protects the climb only; unanswered lower prompts step down.
    pub fn reference_elapsed(&mut self, seconds: f64, now: f64, ready: bool) -> RangeEvent {
        if self.pending_match.is_some()
            || !ready
            || !seconds.is_finite()
            || seconds < 6.0
            || !now.is_finite()
        {
            return RangeEvent::None;
        }
        match self.phase {
            RangePhase::Higher | RangePhase::UpperEdge
                if seconds >= 12.0 || self.last_above_baseline.is_none_or(|t| now - t >= 6.0) =>
            {
                self.begin_lower()
            }
            RangePhase::Lower | RangePhase::LowerEdge => {
                let note = self.target.expect("Lower search has a target");
                if note <= 36 || self.baseline_note.is_some_and(|baseline| note < baseline) {
                    self.complete()
                } else {
                    self.change(
                        (note - self.step).max(36),
                        RangePhase::Lower,
                        "No match yet. Try this lower note; only matched notes enter your result.",
                    )
                }
            }
            _ => RangeEvent::None,
        }
    }
    pub fn stage_message(&self) -> String {
        let last = self.anchor.map(dsp::name).unwrap_or_else(|| "—".into());
        match self.phase {
            RangePhase::Baseline => "Sing an easy note; the piano will follow you.".into(),
            RangePhase::ConfirmStart => "Match your starting note, then let's go up.".into(),
            RangePhase::Higher | RangePhase::UpperEdge => {
                format!("Let's go up. Last matched note: {last}.")
            }
            RangePhase::Lower | RangePhase::LowerEdge => {
                let high = self
                    .matched
                    .iter()
                    .max()
                    .copied()
                    .map(dsp::name)
                    .unwrap_or_else(|| "—".into());
                format!("Highest matched: {high}. Let's go down. Last matched note: {last}.")
            }
            RangePhase::Complete => match self.recommendation() {
                Some((low, high, _)) => format!(
                    "Range check finished. Highest matched: {} → Lowest matched: {}.",
                    dsp::name(high),
                    dsp::name(low)
                ),
                None => "Range check finished; not enough different matched notes yet.".into(),
            },
        }
    }
    /// The user's struggle/comfort report changes direction immediately. It
    /// never records the unconfirmed target as a reached note.
    pub fn limit(&mut self) -> RangeEvent {
        match self.phase {
            RangePhase::Higher | RangePhase::UpperEdge => self.begin_lower(),
            RangePhase::Lower | RangePhase::LowerEdge => self.complete(),
            _ => RangeEvent::None,
        }
    }
    pub fn recommendation(&self) -> Option<(i32, i32, i32)> {
        let low = *self.matched.iter().min()?;
        let high = *self.matched.iter().max()?;
        if low == high {
            return None;
        }
        let mid = (low + high) as f32 / 2.0;
        let note = *self.matched.iter().min_by(|a, b| {
            ((**a as f32 - mid).abs())
                .total_cmp(&(**b as f32 - mid).abs())
                .then(a.cmp(b))
        })?;
        Some((low, high, note))
    }
}

/// An educational endpoint comparison, never a confirmed voice classification.
/// Typical spans: Rochdale Music Service's voice-range guides (see README).
pub fn range_estimate(low: i32, high: i32) -> String {
    const REFERENCES: [(&str, i32, i32); 6] = [
        ("Bass", 38, 64),           // D2–E4
        ("Baritone", 43, 67),       // G2–G4
        ("Tenor", 48, 71),          // C3–B4
        ("Alto/Contralto", 52, 77), // E3–F5
        ("Mezzo-soprano", 55, 81),  // G3–A5
        ("Soprano", 59, 84),        // B3–C6
    ];
    if !(36..=84).contains(&low) || !(36..=84).contains(&high) || high - low < 12 {
        return "Not enough range yet".into();
    }
    if high - low > 36 {
        return "Multiple ranges".into();
    }
    let distance = |a: i32, b: i32| (low - a).abs() + (high - b).abs();
    let best = REFERENCES
        .iter()
        .map(|(_, a, b)| distance(*a, *b))
        .min()
        .unwrap();
    if best > 12 {
        return "No close range match".into();
    }
    REFERENCES
        .iter()
        .filter(|(_, a, b)| distance(*a, *b) <= best + 2)
        .map(|(name, _, _)| *name)
        .collect::<Vec<_>>()
        .join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Singer {
        range: Range,
        time: f64,
        events: Vec<RangeEvent>,
    }
    impl Singer {
        fn new() -> Self {
            Self {
                range: Range::new(),
                time: 0.0,
                events: Vec::new(),
            }
        }
        fn frame(
            &mut self,
            note: Option<f32>,
            confidence: f32,
            rms: f32,
            clipped: bool,
            ready: bool,
        ) {
            let frame = Frame {
                time: self.time,
                hz: note.map(dsp::hz),
                confidence,
                rms,
                clipped,
                spectrum: None,
            };
            self.time += 0.01;
            let event = self.range.observe(&frame, ready);
            if event != RangeEvent::None {
                self.events.push(event);
            }
        }
        fn sing(&mut self, note: f32, seconds: f64, ready: bool) {
            for _ in 0..(seconds * 100.0) as usize {
                self.frame(Some(note), 0.99, 0.1, false, ready);
            }
        }
        fn quiet(&mut self) {
            for _ in 0..35 {
                self.frame(None, 0.0, 0.0, false, true);
            }
        }
        fn discover(&mut self, note: i32) {
            self.sing(note as f32, 1.3, false);
            assert_eq!(self.range.target, Some(note));
            assert_eq!(self.range.phase, RangePhase::ConfirmStart);
        }
        fn confirm(&mut self) {
            let note = self.range.target.unwrap();
            self.sing(note as f32, 1.1, true);
            self.quiet();
        }
        fn miss(&mut self, note: i32) {
            self.sing(note as f32, 1.2, true);
            self.quiet();
        }
    }
    #[test]
    fn guided_hold_waits_for_release_and_counts_sustain_without_reusing_evidence() {
        let mut s = Singer::new();
        s.discover(48);
        let start = s.time;
        for i in 0..700 {
            let f = Frame {
                time: start + i as f64 * 0.01,
                hz: Some(dsp::hz(48.7)),
                confidence: 0.99,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            };
            s.range.observe_guided(&f, true, Some(start));
            assert_eq!(s.range.advance(f.time), RangeEvent::None);
            if i < 100 {
                assert_eq!(s.range.hold, 0.0);
            }
        }
        assert_eq!(s.range.target, Some(48));
        assert_eq!(s.range.matched, vec![48]);
        assert!(!s.range.resting(start + 7.0));
        let (note, seconds) = s.range.highest_sustain.unwrap();
        assert_eq!(note, 48);
        assert!((5.8..=6.1).contains(&seconds), "{seconds}");
        assert_eq!(
            s.range.reference_elapsed(30.0, start + 7.0, true),
            RangeEvent::None
        );
        // Brief gaps and clipped quiet frames must not release a matched hold.
        for i in 0..10 {
            let f = Frame {
                time: start + 7.0 + i as f64 * 0.01,
                hz: None,
                confidence: 0.0,
                rms: 0.0,
                clipped: false,
                spectrum: None,
            };
            s.range.observe_guided(&f, true, Some(start));
        }
        for i in 10..50 {
            let f = Frame {
                time: start + 7.0 + i as f64 * 0.01,
                hz: None,
                confidence: 0.0,
                rms: 0.0,
                clipped: true,
                spectrum: None,
            };
            s.range.observe_guided(&f, true, Some(start));
        }
        assert!(!s.range.resting(start + 7.5));
        for i in 0..35 {
            let f = Frame {
                time: start + 7.5 + i as f64 * 0.01,
                hz: None,
                confidence: 0.0,
                rms: 0.0,
                clipped: false,
                spectrum: None,
            };
            s.range.observe_guided(&f, true, Some(start));
        }
        assert!(s.range.resting(start + 7.85));
        assert_eq!(
            s.range.pacing_cue(start + 7.85, Some(start)),
            Some("Matched — take a breath")
        );
        assert_eq!(s.range.advance(start + 8.6), RangeEvent::None);
        assert_eq!(s.range.advance(start + 8.7), RangeEvent::ReferenceChanged);
        assert_eq!(s.range.target, Some(50));
        assert_eq!(s.range.highest_sustain, Some((48, seconds)));
        assert_eq!(s.range.held_seconds(), 0.0);
    }
    #[test]
    fn release_is_fast_before_the_old_slot_ends_even_with_background_noise() {
        let mut s = Singer::new();
        s.discover(48);
        let start = s.time;
        for i in 0..210 {
            let f = Frame {
                time: start + i as f64 * 0.01,
                hz: Some(dsp::hz(48.0)),
                confidence: 0.99,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            };
            s.range.observe_guided(&f, true, Some(start));
        }
        for i in 0..13 {
            let f = Frame {
                time: start + 2.1 + i as f64 * 0.01,
                hz: None,
                confidence: 0.2,
                rms: 0.015,
                clipped: false,
                spectrum: None,
            };
            s.range.observe_guided(&f, true, Some(start));
            if i < 12 {
                assert!(!s.range.resting(f.time));
            }
        }
        assert!(s.range.resting(start + 2.23));
        assert_eq!(
            s.range.pacing_cue(start + 2.23, Some(start)),
            Some("Matched — take a breath")
        );
        assert_eq!(s.range.advance(start + 3.21), RangeEvent::None);
        assert_eq!(s.range.advance(start + 3.23), RangeEvent::ReferenceChanged);
    }
    #[test]
    fn above_baseline_grace_has_a_hard_limit_for_unmatched_notes() {
        let mut s = Singer::new();
        s.discover(60);
        s.confirm();
        s.sing(65.0, 7.0, true);
        assert_eq!(
            s.range.reference_elapsed(11.99, s.time, true),
            RangeEvent::None
        );
        assert_eq!(
            s.range.reference_elapsed(12.0, s.time, true),
            RangeEvent::ReferenceChanged
        );
        assert_eq!(s.range.phase, RangePhase::Lower);
        assert_eq!(s.range.matched, vec![60]);
    }
    #[test]
    fn range_estimates_compare_endpoints_and_allow_ambiguity() {
        assert_eq!(range_estimate(45, 65), "Baritone");
        assert_eq!(range_estimate(48, 71), "Tenor");
        assert_eq!(range_estimate(52, 77), "Alto/Contralto");
        assert_eq!(range_estimate(59, 84), "Soprano");
        assert_eq!(range_estimate(41, 65), "Bass / Baritone");
        assert_eq!(range_estimate(36, 84), "Multiple ranges");
        assert_eq!(range_estimate(60, 65), "Not enough range yet");
        assert_eq!(range_estimate(65, 45), "Not enough range yet");
    }
    #[test]
    fn discovers_the_singers_note_before_requesting_any_piano() {
        for note in [38, 48, 55, 72, 82] {
            let mut s = Singer::new();
            assert_eq!(s.range.target, None);
            s.sing(note as f32, 0.2, false);
            assert_eq!(s.range.target, None);
            s.sing(note as f32, 0.3, false);
            assert_eq!(s.range.target, Some(note));
            assert!(s.range.matched.is_empty());
            assert_eq!(s.events, vec![RangeEvent::ReferenceChanged]);
        }
    }
    #[test]
    fn green_band_matches_after_failed_search_without_silence() {
        for offset in [
            -0.90, -0.85, -0.79, -0.75, -0.64, 0.64, 0.75, 0.79, 0.85, 0.90,
        ] {
            let mut s = Singer::new();
            s.discover(48);
            for _ in 0..500 {
                s.frame(None, 0.2, 0.1, false, true);
            }
            s.sing(48.0 + offset, 0.5, true);
            assert!(s.range.matched.is_empty());
            s.sing(48.0 + offset, 0.6, true);
            assert_eq!(s.range.matched, vec![48]);
        }
    }
    #[test]
    fn wider_band_still_rejects_a_sustained_neighboring_note() {
        for offset in [-1.0, 1.0] {
            let mut s = Singer::new();
            s.discover(60);
            s.sing(60.0 + offset, 1.5, true);
            assert!(s.range.matched.is_empty());
            assert_eq!(s.range.target, Some(60));
        }
    }
    #[test]
    fn screenshot_like_wobble_advances_without_a_pause() {
        let mut s = Singer::new();
        s.discover(48);
        for i in 0..120 {
            let pitch = 48.55 + 0.2 * (i as f32 * 0.25).sin();
            s.frame(
                Some(pitch),
                if i == 20 { 0.2 } else { 0.99 },
                0.1,
                false,
                true,
            );
        }
        assert_eq!(s.range.matched, vec![48]);
        assert_eq!(s.range.target, Some(50));
        s.sing(50.5, 1.1, true);
        assert_eq!(s.range.matched, vec![48, 50]);
        assert_eq!(s.range.target, Some(52));
    }
    #[test]
    fn clean_recent_phrase_recovers_from_uncertain_onset() {
        let mut s = Singer::new();
        s.discover(48);
        for _ in 0..100 {
            s.frame(None, 0.2, 0.1, false, true);
        }
        s.sing(48.6, 1.1, true);
        assert_eq!(s.range.matched, vec![48]);
        // Continuing the old note must not qualify the next guide.
        s.sing(48.6, 1.0, true);
        assert_eq!(s.range.target, Some(50));
        assert_eq!(s.range.matched, vec![48]);
    }
    #[test]
    fn playback_retry_discards_partial_window() {
        let mut s = Singer::new();
        s.discover(48);
        s.sing(48.0, 0.3, true);
        s.frame(Some(48.0), 0.99, 0.1, false, false);
        s.sing(48.0, 0.2, true);
        assert!(s.range.matched.is_empty());
        s.sing(48.0, 0.85, true);
        assert_eq!(s.range.matched, vec![48]);
    }
    #[test]
    fn guided_matches_require_current_ready_reference() {
        let mut s = Singer::new();
        s.discover(48);
        s.sing(48.0, 4.0, false);
        assert_eq!(s.range.trials, 0);
        assert!(s.range.matched.is_empty());
        s.sing(48.0, 0.3, true);
        assert!(s.range.hold > 0.25);
        s.frame(Some(48.0), 0.99, 0.1, false, false);
        assert_eq!(s.range.hold, 0.0);
        assert_eq!(s.range.successes, 0);
        s.confirm();
        assert_eq!(s.range.matched, vec![48]);
    }
    #[test]
    fn smaller_steps_continue_until_the_singer_chooses_to_descend_or_finish() {
        let mut s = Singer::new();
        s.discover(55);
        s.confirm();
        s.confirm();
        assert_eq!(s.range.target, Some(59));
        s.miss(57);
        s.miss(57);
        assert_eq!(
            (s.range.phase, s.range.target),
            (RangePhase::UpperEdge, Some(58))
        );
        s.confirm();
        assert_eq!(
            (s.range.phase, s.range.target),
            (RangePhase::Higher, Some(59))
        );
        for _ in 0..4 {
            s.miss(57);
        }
        assert_eq!(s.range.target, Some(59));
        assert_eq!(s.range.phase, RangePhase::Higher);
        assert_eq!(s.range.limit(), RangeEvent::ReferenceChanged);
        assert_eq!(s.range.target, Some(56));
        for target in [56, 54, 52] {
            assert_eq!(s.range.target, Some(target));
            s.confirm();
        }
        s.miss(52);
        s.miss(52);
        assert_eq!(
            (s.range.phase, s.range.target),
            (RangePhase::LowerEdge, Some(51))
        );
        s.confirm();
        assert_eq!(
            (s.range.phase, s.range.target),
            (RangePhase::Lower, Some(50))
        );
        s.miss(52);
        s.miss(52);
        assert_eq!(s.range.target, Some(50));
        assert_eq!(s.range.phase, RangePhase::Lower);
        assert_eq!(s.range.limit(), RangeEvent::Complete);
        assert_eq!(s.range.recommendation(), Some((51, 58, 54)));
        assert!(!s.range.matched.contains(&59));
        assert!(!s.range.matched.contains(&50));
    }
    #[test]
    fn higher_than_baseline_singing_postpones_descent_even_when_off_target() {
        let mut s = Singer::new();
        s.discover(60);
        s.confirm(); // target D4
        s.sing(65.0, 7.0, true); // above baseline but misses the guide
        assert_eq!(
            s.range.reference_elapsed(7.0, s.time, true),
            RangeEvent::None
        );
        assert_eq!(s.range.phase, RangePhase::Higher);
        assert_eq!(s.range.matched, vec![60]);
        s.quiet();
        assert_eq!(
            s.range.reference_elapsed(7.35, s.time, true),
            RangeEvent::None
        );
        s.time += 6.0;
        assert_eq!(
            s.range.reference_elapsed(13.35, s.time, true),
            RangeEvent::ReferenceChanged
        );
        assert_eq!(s.range.phase, RangePhase::Lower);
    }
    #[test]
    fn a_brief_high_glitch_does_not_disable_the_timeout() {
        let mut s = Singer::new();
        s.discover(60);
        s.confirm();
        s.sing(65.0, 0.05, true);
        assert_eq!(
            s.range.reference_elapsed(6.0, s.time, true),
            RangeEvent::ReferenceChanged
        );
    }
    #[test]
    fn three_ready_piano_runs_descend_without_counting_unmatched_note() {
        let mut s = Singer::new();
        s.discover(63);
        assert_eq!(
            s.range.reference_elapsed(6.0, s.time, true),
            RangeEvent::None
        );
        s.confirm();
        assert_eq!(s.range.target, Some(65)); // F4
        assert_eq!(
            s.range.reference_elapsed(5.99, s.time, true),
            RangeEvent::None
        );
        assert_eq!(
            s.range.reference_elapsed(60.0, s.time, false),
            RangeEvent::None
        );
        assert_eq!(
            s.range.reference_elapsed(f64::NAN, s.time, true),
            RangeEvent::None
        );
        assert_eq!(
            s.range.reference_elapsed(6.0, s.time, true),
            RangeEvent::ReferenceChanged
        );
        assert_eq!(s.range.phase, RangePhase::Lower);
        assert_eq!(s.range.target, Some(61));
        assert_eq!(s.range.matched, vec![63]);
        // This prompt is already below the discovered start: end after three runs.
        assert_eq!(
            s.range.reference_elapsed(6.0, s.time, true),
            RangeEvent::Complete
        );
    }
    #[test]
    fn low_prompts_retry_down_from_high_but_finish_below_start() {
        let mut s = Singer::new();
        s.discover(48);
        s.confirm(); // C3
        s.confirm(); // D3
        s.confirm(); // E3
        s.confirm(); // F#3
        s.range.limit();
        assert_eq!(s.range.target, Some(52));
        let matched = s.range.matched.clone();
        for target in [50, 48, 46] {
            assert_eq!(
                s.range.reference_elapsed(5.99, s.time, true),
                RangeEvent::None
            );
            assert_eq!(
                s.range.reference_elapsed(60.0, s.time, false),
                RangeEvent::None
            );
            assert_eq!(
                s.range.reference_elapsed(6.0, s.time, true),
                RangeEvent::ReferenceChanged
            );
            assert_eq!(s.range.target, Some(target));
            assert_eq!(s.range.matched, matched);
        }
        // No endless descent below the discovered starting note.
        s.sing(55.0, 1.1, true);
        assert_eq!(
            s.range.reference_elapsed(6.0, s.time, true),
            RangeEvent::Complete
        );
        assert_eq!(s.range.matched, matched);
        assert_eq!(s.range.recommendation().map(|r| (r.0, r.1)), Some((48, 54)));
        assert!(s.range.stage_message().contains("Lowest matched: C3"));
    }
    #[test]
    fn matched_lower_notes_continue_until_first_unanswered_low_prompt() {
        let mut s = Singer::new();
        s.discover(48);
        s.confirm();
        s.range.limit();
        s.confirm(); // A#2
        assert_eq!(s.range.target, Some(44));
        assert_eq!(
            s.range.reference_elapsed(6.0, s.time, true),
            RangeEvent::Complete
        );
        assert_eq!(s.range.recommendation().map(|r| (r.0, r.1)), Some((46, 48)));
        assert!(!s.range.matched.contains(&44));
    }
    #[test]
    fn ordinary_misses_never_decide_an_upper_limit() {
        let mut s = Singer::new();
        s.discover(48);
        s.confirm();
        s.miss(48);
        s.miss(48);
        assert_eq!(s.range.target, Some(49));
        for _ in 0..5 {
            s.miss(48);
        }
        assert_eq!(
            (s.range.phase, s.range.target),
            (RangePhase::UpperEdge, Some(49))
        );
        assert_eq!(s.range.matched, vec![48]);
        s.confirm();
        assert_eq!(s.range.target, Some(50));
        for _ in 0..5 {
            s.miss(48);
        }
        assert_eq!(
            (s.range.phase, s.range.target),
            (RangePhase::Higher, Some(50))
        );
    }
    #[test]
    fn struggle_button_descends_immediately_and_never_adds_an_unconfirmed_note() {
        let mut s = Singer::new();
        s.discover(48);
        s.confirm();
        s.confirm();
        assert_eq!(s.range.target, Some(52));
        assert_eq!(s.range.limit(), RangeEvent::ReferenceChanged);
        assert_eq!(
            (s.range.phase, s.range.target),
            (RangePhase::Lower, Some(48))
        );
        assert_eq!(s.range.matched, vec![48, 50]);
        assert_eq!(s.range.limit(), RangeEvent::Complete);
        assert_eq!(s.range.recommendation(), Some((48, 50, 48)));
    }
    #[test]
    fn silence_noise_clipping_and_short_notes_never_establish_limits() {
        let mut s = Singer::new();
        s.discover(48);
        s.confirm();
        let target = s.range.target;
        for _ in 0..2000 {
            s.frame(None, 0.0, 0.0, false, true);
        }
        assert_eq!(s.range.trials, 0);
        for _ in 0..500 {
            s.frame(Some(46.0), 0.2, 0.1, false, true);
        }
        s.quiet();
        assert_eq!(s.range.trials, 0);
        for _ in 0..120 {
            s.frame(Some(46.0), 0.99, 0.1, true, true);
        }
        s.quiet();
        assert_eq!(s.range.trials, 0);
        s.sing(46.0, 0.8, true);
        s.quiet();
        assert_eq!(s.range.trials, 0);
        assert_eq!(s.range.target, target);
        assert_eq!(s.range.phase, RangePhase::Higher);
    }
    #[test]
    fn discontinuity_discards_partial_hold_and_baseline_octave_glitches() {
        let mut s = Singer::new();
        s.sing(48.0, 0.2, false);
        s.frame(Some(60.0), 0.99, 0.1, false, false);
        s.sing(48.0, 0.2, false);
        assert_eq!(s.range.target, None);
        s.sing(48.0, 0.3, false);
        s.sing(48.0, 0.3, true);
        s.time += 1.0;
        s.sing(48.0, 0.1, true);
        assert_eq!(s.range.trials, 0);
        assert!(s.range.hold < 0.4);
    }
    #[test]
    fn short_imperfect_matches_accept_brief_pitch_and_tracking_wobbles() {
        let mut s = Singer::new();
        s.discover(48);
        s.sing(48.7, 0.2, true);
        s.sing(50.0, 0.05, true);
        s.sing(47.3, 0.85, true);
        assert_eq!(s.range.matched, vec![48]);
        assert_eq!(s.range.phase, RangePhase::Higher);
        s.quiet();
        s.sing(50.5, 0.2, true);
        for _ in 0..5 {
            s.frame(None, 0.2, 0.1, false, true);
        }
        s.sing(49.5, 0.85, true);
        assert_eq!(s.range.matched, vec![48, 50]);
        assert_eq!(s.range.target, Some(52));
    }
    #[test]
    fn distinct_wrong_notes_long_gaps_and_separate_bursts_do_not_match() {
        let mut s = Singer::new();
        s.discover(48);
        s.sing(49.0, 0.5, true);
        s.quiet();
        assert_eq!(s.range.successes, 0);
        s.sing(48.0, 0.2, true);
        s.sing(50.0, 0.15, true);
        s.sing(48.0, 0.2, true);
        assert_eq!(s.range.successes, 0);
        assert!(s.range.hold < 0.6);
        s.quiet();
        s.sing(48.0, 0.2, true);
        for _ in 0..15 {
            s.frame(None, 0.0, 0.0, false, true);
        }
        s.sing(48.0, 0.2, true);
        assert_eq!(s.range.successes, 0);
    }
    #[test]
    fn natural_discovery_accepts_a_short_wobbly_note() {
        let mut s = Singer::new();
        for i in 0..50 {
            let pitch = 55.0 + 0.5 * (i as f32 * 0.2).sin();
            s.frame(Some(pitch), 0.99, 0.1, false, false);
        }
        assert_eq!(s.range.target, Some(55));
        assert_eq!(s.range.phase, RangePhase::ConfirmStart);
    }
    #[test]
    fn supported_extremes_complete_without_repeating_a_bound_forever() {
        let mut s = Singer::new();
        s.discover(84);
        s.confirm();
        assert_eq!(
            (s.range.phase, s.range.target),
            (RangePhase::Lower, Some(82))
        );
        while s.range.target.is_some() {
            let target = s.range.target.unwrap();
            assert!((36..=84).contains(&target));
            s.confirm();
        }
        assert_eq!(s.range.recommendation().map(|r| (r.0, r.1)), Some((36, 84)));
    }
    #[test]
    fn clear_but_difficult_long_hold_allows_continuous_retry() {
        let mut s = Singer::new();
        s.discover(48);
        s.confirm();
        s.sing(48.0, 8.0, true);
        assert_eq!(s.range.trials, 1);
        assert_eq!(s.range.successes, 0);
        assert_eq!(s.range.target, Some(50));
    }
}

#[cfg(test)]
mod pcm_tests {
    use super::*;
    #[test]
    fn generated_pcm_pitch_frames_discover_and_confirm_a_natural_start() {
        let mut r = Range::new();
        let mut detector = crate::dsp::Detector::new();
        let mut time = 0.0;
        for (singing, ready, count) in [
            (true, false, 50),
            (false, true, 35),
            (true, true, 120),
            (false, true, 35),
            (true, true, 120),
        ] {
            let pcm: Vec<f32> = (0..600 + count * 120)
                .map(|i| {
                    if singing {
                        0.2 * (std::f32::consts::TAU * dsp::hz(55.0) * i as f32 / 12000.0).sin()
                    } else {
                        0.0
                    }
                })
                .collect();
            for start in (0..count * 120).step_by(120) {
                let f = detector.analyze(&pcm[start..start + 600], 12000.0, time, false);
                r.observe(&f, ready);
                time += 0.01;
            }
        }
        assert_eq!(r.matched, vec![55]);
        assert_eq!(r.target, Some(57));
        assert_eq!(r.phase, RangePhase::Higher);
    }
}
