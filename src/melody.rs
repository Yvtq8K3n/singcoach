//! Fixed-time melody practice. Only microphone frames enter assessment.
use crate::{
    dsp::{self, Frame},
    training::median,
};

#[derive(Clone, Copy, Debug)]
pub struct Note {
    pub midi: i32,
    pub start: f64,
    pub duration: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Good,
    Improve,
    Unscored,
}
#[derive(Debug)]
pub struct Assessment {
    pub bias: Option<f32>,
    pub absolute: Option<f32>,
    pub coverage: f32,
    pub in_tune: Option<f32>,
    pub status: Status,
}
#[derive(Debug)]
pub struct Result {
    pub notes: Vec<Assessment>,
    pub score: Option<f32>,
    pub coverage: f32,
    pub hardest: Option<usize>,
    pub feedback: String,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Idle,
    Playback,
    Ready,
    Recording { start: f64 },
    Analyzing,
    Results,
    Error,
}
pub struct Coach {
    pub state: State,
    pub pattern: usize,
    pub transpose: i32,
    pub variation: usize,
    reference_used: bool,
    pub tolerance: f32,
    pub minimum_coverage: f32,
    pub margin: f64,
    pub frames: Vec<Frame>,
    pub result: Option<Result>,
    pub error: Option<String>,
    pub playback_start: Option<f64>,
}
impl Default for Coach {
    fn default() -> Self {
        Self {
            state: State::Idle,
            pattern: 0,
            transpose: 0,
            variation: 0,
            reference_used: false,
            tolerance: 35.0,
            minimum_coverage: 0.5,
            margin: 0.08,
            frames: Vec::with_capacity(1400),
            result: None,
            error: None,
            playback_start: None,
        }
    }
}
impl Coach {
    pub fn notes(&self) -> [Note; 5] {
        let variants = if self.pattern == 0 {
            [
                [0, 2, 4, 5, 4],
                [0, 2, 4, 2, 0],
                [0, 4, 2, 5, 4],
                [0, 2, 5, 4, 2],
            ]
        } else {
            [
                [0, 4, 7, 4, 0],
                [0, 7, 4, 7, 0],
                [0, 4, 0, 7, 4],
                [0, 7, 4, 0, 4],
            ]
        };
        let pitches = variants[self.variation % variants.len()].map(|offset| 48 + offset);
        let durations = if self.pattern == 0 {
            [1.0, 1.0, 1.0, 1.0, 2.0]
        } else {
            [1.5, 1.0, 1.5, 1.0, 2.0]
        };
        let mut start = 0.0;
        std::array::from_fn(|i| {
            let note = Note {
                midi: pitches[i] + self.transpose,
                start,
                duration: durations[i],
            };
            start += durations[i];
            note
        })
    }
    pub fn duration(&self) -> f64 {
        let n = self.notes()[4];
        n.start + n.duration
    }
    pub fn sequence(&self) -> [(i32, u32); 5] {
        self.notes().map(|n| (n.midi, (n.duration * 1000.0) as u32))
    }
    pub fn prepare_reference(&mut self) {
        if self.reference_used || !self.frames.is_empty() {
            self.variation = (self.variation + 1) % 4;
        }
        self.reset();
        self.reference_used = true;
    }
    pub fn reset_variation(&mut self) {
        self.variation = 0;
        self.reference_used = false;
        self.reset();
    }
    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.frames.clear();
        self.result = None;
        self.error = None;
        self.playback_start = None;
    }
    pub fn fail(&mut self, message: &str) {
        self.state = State::Error;
        self.result = None;
        self.error = Some(message.into());
    }
    pub fn push(&mut self, f: &Frame) {
        if let State::Recording { start } = self.state {
            let time = f.time - start;
            if time >= 0.0 && time <= self.duration() + 0.3 && self.frames.len() < 1600 {
                let mut sample = f.clone();
                sample.time = time;
                self.frames.push(sample);
            }
        }
    }
    pub fn finish(&mut self) {
        self.state = State::Analyzing;
        self.result = Some(evaluate(
            &self.notes(),
            &self.frames,
            self.tolerance,
            self.minimum_coverage,
            self.margin,
        ));
        self.state = State::Results;
    }
}
pub fn reliable(f: &Frame) -> bool {
    !f.clipped
        && f.rms.is_finite()
        && f.rms >= 0.008
        && f.confidence >= 0.85
        && f.hz.is_some_and(|h| h.is_finite() && h > 0.0)
}
/// A segment may span at most 25 ms and must have reliable endpoints.
pub(crate) fn connected(a: &Frame, b: &Frame) -> bool {
    reliable(a)
        && reliable(b)
        && b.time > a.time
        && b.time - a.time <= 0.025
        && (dsp::midi(a.hz.unwrap()) - dsp::midi(b.hz.unwrap())).abs() < 9.0
}
pub fn evaluate(
    notes: &[Note],
    frames: &[Frame],
    tolerance: f32,
    minimum: f32,
    margin: f64,
) -> Result {
    let mut assessments = Vec::new();
    let (mut tuned_time, mut useful_time, mut voiced_time) = (0.0f64, 0.0f64, 0.0f64);
    for note in notes {
        let (lo, hi) = (note.start + margin, note.start + note.duration - margin);
        let useful = (hi - lo).max(0.001);
        let mut errors = Vec::new();
        let (mut voiced, mut tuned) = (0.0, 0.0);
        for pair in frames.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if !connected(a, b) {
                continue;
            }
            let dt = (b.time.min(hi) - a.time.max(lo)).max(0.0);
            if dt == 0.0 {
                continue;
            }
            let error = dsp::cents(a.hz.unwrap(), note.midi);
            errors.push(error);
            voiced += dt;
            if error.abs() <= tolerance {
                tuned += dt;
            }
        }
        let coverage = (voiced / useful).min(1.0) as f32;
        let damaged = frames
            .iter()
            .any(|f| f.time >= lo && f.time < hi && f.clipped);
        let reached_end = frames.last().is_some_and(|f| f.time >= hi);
        let enough = coverage >= minimum && errors.len() >= 10 && !damaged && reached_end;
        let ratio = if enough {
            Some((tuned / voiced) as f32)
        } else {
            None
        };
        let bias = if enough { Some(median(&errors)) } else { None };
        let absolute = if enough {
            Some(median(&errors.iter().map(|e| e.abs()).collect::<Vec<_>>()))
        } else {
            None
        };
        assessments.push(Assessment {
            bias,
            absolute,
            coverage,
            in_tune: ratio,
            status: match ratio {
                Some(r) if r >= 0.7 => Status::Good,
                Some(_) => Status::Improve,
                None => Status::Unscored,
            },
        });
        tuned_time += tuned;
        useful_time += useful;
        voiced_time += voiced;
    }
    // A partial melody never receives a misleading whole-melody score.
    let complete = assessments.iter().all(|a| a.status != Status::Unscored);
    let hardest = assessments
        .iter()
        .enumerate()
        .filter_map(|(i, a)| a.in_tune.map(|r| (i, r)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i);
    let feedback = if !complete {
        "Dados insuficientes: repete a melodia inteira com voz clara e auscultadores.".into()
    } else if let Some(i) = hardest.filter(|&i| assessments[i].status == Status::Improve) {
        let direction = if assessments[i].bias.unwrap_or(0.0) < 0.0 {
            "mais alta"
        } else {
            "mais baixa"
        };
        format!(
            "Repete a nota {} ({}) {} e mantém a altura estável.",
            i + 1,
            dsp::name(notes[i].midi),
            direction
        )
    } else {
        "Boa afinação. Repete para ganhar consistência.".into()
    };
    Result {
        notes: assessments,
        coverage: (voiced_time / useful_time.max(0.001)).min(1.0) as f32,
        score: complete.then_some((100.0 * tuned_time / useful_time.max(0.001)) as f32),
        hardest,
        feedback,
    }
}
pub fn metric(value: Option<f32>, unit: &str) -> String {
    value.map_or_else(|| "—".into(), |v| format!("{v:+.0}{unit}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frames(c: &Coach, bad: Option<usize>) -> Vec<Frame> {
        let notes = c.notes();
        (0..=(c.duration() * 100.0) as usize)
            .map(|i| {
                let time = i as f64 * 0.01;
                let index = notes
                    .iter()
                    .position(|n| time >= n.start && time < n.start + n.duration)
                    .unwrap_or(4);
                Frame {
                    time,
                    hz: Some(dsp::hz(
                        notes[index].midi as f32 + if bad == Some(index) { 0.8 } else { 0.0 },
                    )),
                    confidence: 0.98,
                    rms: 0.1,
                    clipped: false,
                    spectrum: None,
                }
            })
            .collect()
    }
    fn result(c: &Coach, f: &[Frame]) -> Result {
        evaluate(&c.notes(), f, c.tolerance, c.minimum_coverage, c.margin)
    }
    #[test]
    fn reference_rotates_bounded_sequences_with_score_and_abstention() {
        for pattern in 0..2 {
            for transpose in [-12, if pattern == 0 { 31 } else { 29 }] {
                let mut c = Coach {
                    pattern,
                    transpose,
                    ..Default::default()
                };
                let mut seen = Vec::new();
                for index in 0..8 {
                    c.prepare_reference();
                    assert_eq!(c.variation, index % 4);
                    let pitches = c.notes().map(|n| n.midi);
                    assert!(pitches.iter().all(|n| (36..=84).contains(n)));
                    if index < 4 {
                        assert!(!seen.contains(&pitches));
                        seen.push(pitches);
                    }
                    let r = result(&c, &frames(&c, None));
                    assert!(r.score.unwrap() > 98.0);
                    assert!(result(&c, &[]).score.is_none());
                }
                c.reset_variation();
                c.prepare_reference();
                assert_eq!(c.variation, 0);
            }
        }
    }
    #[test]
    fn fixed_alignment_and_only_wrong_note_changes() {
        for pattern in 0..2 {
            let c = Coach {
                pattern,
                ..Default::default()
            };
            let r = result(&c, &frames(&c, None));
            assert!(r.notes.iter().all(|a| a.status == Status::Good));
            assert!(r.score.unwrap() > 98.0);
            let r = result(&c, &frames(&c, Some(2)));
            assert_eq!(r.hardest, Some(2));
            for (i, a) in r.notes.iter().enumerate() {
                assert_eq!(
                    a.status,
                    if i == 2 {
                        Status::Improve
                    } else {
                        Status::Good
                    }
                );
            }
            assert!((r.notes[2].bias.unwrap() - 80.0).abs() < 0.1);
            assert!((r.notes[2].absolute.unwrap() - 80.0).abs() < 0.1);
        }
    }
    #[test]
    fn silence_unreliable_clipping_gaps_and_partial_attempt_abstain() {
        let c = Coach::default();
        for mode in 0..5 {
            let mut f = frames(&c, None);
            for a in &mut f {
                match mode {
                    0 => a.hz = None,
                    1 => a.confidence = 0.2,
                    2 => a.clipped = true,
                    3 => a.rms = 0.0,
                    _ => {}
                }
            }
            if mode == 4 {
                f.retain(|f| f.time < 0.5);
            }
            assert!(result(&c, &f).score.is_none());
        }
        let mut f = frames(&c, None);
        f.retain(|f| f.time < 2.1 || f.time > 2.9);
        let r = result(&c, &f);
        assert_eq!(r.notes[2].status, Status::Unscored);
        assert!(r.score.is_none());
        let mut partial = frames(&c, None);
        partial.retain(|f| f.time < c.duration() - 0.4);
        let r = result(&c, &partial);
        assert!(r.score.is_none());
        assert_eq!(r.notes[4].status, Status::Unscored);
        let mut f = frames(&c, None);
        f[250].clipped = true;
        assert_eq!(result(&c, &f).notes[2].status, Status::Unscored);
    }
    #[test]
    fn trace_breaks_at_silence_gaps_and_octave_glitches() {
        let c = Coach::default();
        let f = frames(&c, None);
        let a = &f[10];
        let mut b = f[11].clone();
        assert!(connected(a, &b));
        b.hz = None;
        assert!(!connected(a, &b));
        b = f[11].clone();
        b.time += 0.1;
        assert!(!connected(a, &b));
        b = f[11].clone();
        b.hz = Some(a.hz.unwrap() * 2.0);
        assert!(!connected(a, &b));
        b = f[11].clone();
        b.clipped = true;
        assert!(!connected(a, &b));
    }
    #[test]
    fn transposition_math_and_capture_use_attempt_epoch_and_reset() {
        for pattern in 0..2 {
            for transpose in -12..=if pattern == 0 { 31 } else { 29 } {
                let c = Coach {
                    pattern,
                    transpose,
                    ..Default::default()
                };
                for n in c.notes() {
                    assert!((36..=84).contains(&n.midi));
                    assert!(dsp::cents(dsp::hz(n.midi as f32), n.midi).abs() < 0.01);
                }
            }
        }
        let mut c = Coach {
            state: State::Recording { start: 100.0 },
            ..Default::default()
        };
        let mut f = frames(&c, None)[0].clone();
        f.time = 99.0;
        c.push(&f);
        assert!(c.frames.is_empty());
        f.time = 100.25;
        c.push(&f);
        assert_eq!(c.frames[0].time, 0.25);
        c.finish();
        assert!(c.result.as_ref().unwrap().score.is_none());
        c.reset();
        assert!(c.frames.is_empty() && c.result.is_none());
        assert_eq!(c.state, State::Idle);
    }
    #[test]
    fn prepared_piano_sequence_has_correct_duration_and_pitches() {
        let c = Coach {
            pattern: 1,
            ..Default::default()
        };
        let pcm = crate::piano::sequence(&c.sequence(), 12000);
        assert_eq!(pcm.len(), (c.duration() * 12000.0) as usize + 600);
        let mut detector = crate::dsp::Detector::new();
        for n in c.notes() {
            let start = ((n.start + 0.2) * 12000.0) as usize;
            let frame = detector.analyze(&pcm[start..start + 600], 12000.0, 0.0, false);
            assert!(dsp::cents(frame.hz.unwrap(), n.midi).abs() < 8.0);
        }
    }
}
