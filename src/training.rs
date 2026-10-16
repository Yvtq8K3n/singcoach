use crate::dsp::{cents, Frame};
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Exercise {
    Attack,
    Transition,
    Melody,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Technique,
    Performance,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Config {
    pub target: i32,
    pub interval: i32,
    pub exercise: Exercise,
    pub mode: Mode,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            target: 48,
            interval: 2,
            exercise: Exercise::Attack,
            mode: Mode::Technique,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Measured {
    pub score: f32,
    pub clean: bool,
    pub initial: f32,
    pub settling: Option<f32>,
    pub settled: Option<f32>,
    pub spread: Option<f32>,
    pub confidence: f32,
    pub feedback: String,
}
#[derive(Clone, Debug)]
pub enum Result {
    Measured(Measured),
    Unscored(String),
}
pub fn median(values: &[f32]) -> f32 {
    let mut v = values.to_vec();
    v.sort_by(f32::total_cmp);
    if v.len().is_multiple_of(2) {
        (v[v.len() / 2 - 1] + v[v.len() / 2]) * 0.5
    } else {
        v[v.len() / 2]
    }
}
pub fn sd(v: &[f32]) -> f32 {
    let mean = v.iter().sum::<f32>() / v.len() as f32;
    (v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / v.len() as f32).sqrt()
}
fn arrival(f: &[Frame], start: usize, target: i32) -> Option<usize> {
    for i in start..f.len() {
        let mut j = i;
        while j < f.len()
            && f[j].hz.is_some_and(|h| cents(h, target).abs() <= 15.0)
            && f[j].confidence >= 0.85
            && (j == i || f[j].time - f[j - 1].time <= 0.025)
        {
            if f[j].time - f[i].time >= 0.099 {
                return Some(i);
            }
            j += 1;
        }
    }
    None
}
pub fn evaluate(f: &[Frame], c: Config) -> Result {
    let no = |s: &str| Result::Unscored(s.into());
    if c.exercise == Exercise::Melody {
        return no("Melody requires its expected note timeline.");
    }
    if f.len() < 2 {
        return no("Sing for about one second, then pause.");
    }
    let duration = f.last().unwrap().time - f[0].time;
    if duration
        < if c.exercise == Exercise::Attack {
            0.35
        } else {
            0.8
        }
    {
        return no("Hold the exercise longer, then pause.");
    }
    if f.iter().any(|f| f.clipped) {
        return no("Input clipped. Lower microphone gain and retry.");
    }
    if f.windows(2)
        .any(|w| w[1].time <= w[0].time || w[1].time - w[0].time > 0.025)
    {
        return no("Audio was interrupted. Start a fresh attempt.");
    }
    let valid = |f: &&Frame| f.hz.is_some() && f.confidence >= 0.85;
    let fraction = f.iter().filter(valid).count() as f32 / f.len() as f32;
    if fraction
        < if c.exercise == Exercise::Attack {
            0.8
        } else {
            0.9
        }
    {
        return no("Pitch was unclear. Use a steady sound in a quieter room.");
    }
    let early: Vec<_> = f
        .iter()
        .take_while(|x| x.time - f[0].time <= 0.03)
        .collect();
    if early.iter().any(|x| x.hz.is_none() || x.confidence < 0.85) {
        return no("The start was unclear. Try another relaxed entry.");
    }
    let initial = median(
        &early
            .iter()
            .map(|x| cents(x.hz.unwrap(), c.target))
            .collect::<Vec<_>>(),
    );
    let confidence = f.iter().map(|x| x.confidence).sum::<f32>() / f.len() as f32;
    let mut departure = 0;
    let target = if c.exercise == Exercise::Transition {
        let first: Vec<_> = f.iter().take_while(|x| x.time - f[0].time < 0.25).collect();
        if first
            .iter()
            .any(|x| x.hz.is_none_or(|h| cents(h, c.target).abs() > 35.0) || x.confidence < 0.85)
        {
            return no("Establish the first note for a quarter second before moving.");
        }
        departure = f
            .iter()
            .position(|x| x.hz.is_some_and(|h| cents(h, c.target).abs() > 40.0))
            .unwrap_or(f.len());
        c.target + c.interval
    } else {
        c.target
    };
    let reached = arrival(f, departure, target);
    if c.exercise == Exercise::Transition
        && f[departure.min(f.len())..reached.unwrap_or(f.len())]
            .iter()
            .any(|x| x.hz.is_none() || x.confidence < 0.85)
    {
        return no("Keep the sound connected through the note change.");
    }
    let settling = reached.map(|i| ((f[i].time - f[departure].time) * 1000.0) as f32);
    let held: Vec<f32> = reached
        .map(|i| {
            f[i..]
                .iter()
                .filter(valid)
                .map(|x| cents(x.hz.unwrap(), target))
                .collect()
        })
        .unwrap_or_default();
    let settled = if held.is_empty() {
        None
    } else {
        Some(median(&held))
    };
    let spread = if held.is_empty() {
        None
    } else {
        Some(sd(&held))
    };
    let clean = initial.abs()
        <= if c.exercise == Exercise::Attack {
            20.0
        } else {
            35.0
        }
        && settling.is_some_and(|v| v <= 80.0)
        && settled.is_some_and(|v| v.abs() <= 15.0)
        && (c.exercise == Exercise::Attack || spread.is_some_and(|v| v <= 20.0));
    let score = if c.exercise == Exercise::Attack && c.mode == Mode::Performance {
        if reached.is_some() {
            100.0
        } else {
            0.0
        }
    } else if c.exercise == Exercise::Attack {
        (100.0
            - 0.6 * initial.abs()
            - 0.15 * settling.unwrap_or(1000.0)
            - 0.4 * settled.unwrap_or(100.0).abs())
        .clamp(0.0, 100.0)
    } else {
        (100.0
            - 0.15 * settling.unwrap_or(1000.0)
            - 0.4 * settled.unwrap_or(100.0).abs()
            - 0.5 * (spread.unwrap_or(100.0) - 20.0).max(0.0))
        .clamp(0.0, 100.0)
    };
    let feedback =
        if c.mode == Mode::Performance && c.exercise == Exercise::Attack && reached.is_some() {
            "Target arrival confirmed. Repeat it reliably.".into()
        } else if clean {
            "Clean entry. Repeat it reliably.".into()
        } else if c.exercise == Exercise::Attack && initial.abs() > 20.0 {
            format!(
                "You started {:.0} cents {}. Enter directly on the note.",
                initial.abs(),
                if initial < 0.0 { "low" } else { "high" }
            )
        } else if reached.is_none() {
            "Listen again, then hold the target steadily.".into()
        } else if c.exercise == Exercise::Transition {
            "Move directly to the second note while keeping the sound connected.".into()
        } else {
            "Aim to establish the target sooner, without searching.".into()
        };
    Result::Measured(Measured {
        score,
        clean,
        initial,
        settling,
        settled,
        spread,
        confidence,
        feedback,
    })
}
#[derive(Default)]
pub struct Segmenter {
    frames: Vec<Frame>,
    quiet_since: Option<f64>,
    blocked: bool,
}
impl Segmenter {
    pub fn started_at(&self) -> Option<f64> {
        self.frames.first().map(|f| f.time)
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn push(&mut self, f: Frame) -> Option<Vec<Frame>> {
        let active = f.rms >= 0.008;
        if active {
            self.quiet_since = None;
            if !self.blocked {
                self.frames.push(f);
            }
        } else if !self.frames.is_empty() || self.blocked {
            let since = *self.quiet_since.get_or_insert(f.time);
            if f.time - since >= 0.2 {
                self.blocked = false;
                self.quiet_since = None;
                let mut out = std::mem::take(&mut self.frames);
                while out.last().is_some_and(|f| f.rms < 0.008) {
                    out.pop();
                }
                return if out.is_empty() { None } else { Some(out) };
            }
            if !self.blocked {
                self.frames.push(f);
            }
        }
        if self.frames.len() > 1500 {
            self.frames.clear();
            self.blocked = true;
        }
        None
    }
}
#[derive(Debug)]
pub struct Summary {
    pub mean: f32,
    pub spread: f32,
    pub best: f32,
    pub worst: f32,
    pub clean: usize,
    pub reliability: f32,
}
pub fn summarize(v: &[Measured]) -> Summary {
    let scores: Vec<_> = v.iter().map(|v| v.score).collect();
    let mean = scores.iter().sum::<f32>() / v.len() as f32;
    let spread = sd(&scores);
    Summary {
        mean,
        spread,
        best: scores.iter().copied().fold(0.0, f32::max),
        worst: scores.iter().copied().fold(100.0, f32::min),
        clean: v.iter().filter(|v| v.clean).count(),
        reliability: (mean - spread).clamp(0.0, 100.0),
    }
}
pub use crate::range::{Range, RangeEvent, RangePhase};
#[cfg(test)]
mod tests {
    use super::*;
    fn frames(errors: impl Fn(usize) -> Option<f32>, n: usize) -> Vec<Frame> {
        (0..n)
            .map(|i| Frame {
                time: i as f64 * 0.01,
                hz: errors(i).map(|e| crate::dsp::hz(48.0 + e / 100.0)),
                confidence: 0.99,
                rms: 0.1,
                clipped: false,
                spectrum: None,
            })
            .collect()
    }
    fn measured(f: &[Frame], c: Config) -> Measured {
        match evaluate(f, c) {
            Result::Measured(v) => v,
            r => panic!("{r:?}"),
        }
    }
    #[test]
    fn direct_and_scoop() {
        let c = Config::default();
        assert!(measured(&frames(|_| Some(0.0), 100), c).clean);
        let s = measured(
            &frames(
                |i| Some(if i < 20 { -100.0 + i as f32 * 5.0 } else { 0.0 }),
                100,
            ),
            c,
        );
        assert!(!s.clean);
        assert!(s.score < 90.0);
        assert_eq!(
            measured(
                &frames(
                    |i| Some(if i < 20 { -100.0 + i as f32 * 5.0 } else { 0.0 }),
                    100
                ),
                Config {
                    mode: Mode::Performance,
                    ..c
                }
            )
            .score,
            100.0
        );
    }
    #[test]
    fn unreliable_audio_abstains() {
        let mut f = frames(|_| Some(0.0), 100);
        f[10].clipped = true;
        assert!(matches!(
            evaluate(&f, Config::default()),
            Result::Unscored(_)
        ));
        f[10].clipped = false;
        f[10].time += 0.1;
        assert!(matches!(
            evaluate(&f, Config::default()),
            Result::Unscored(_)
        ));
    }
    #[test]
    fn reliability_penalizes_spread() {
        let mut v = vec![measured(&frames(|_| Some(0.0), 100), Config::default()); 5];
        v[4].score = 0.0;
        let s = summarize(&v);
        assert_eq!(s.mean, 80.0);
        assert_eq!(s.reliability, 40.0);
    }
    #[test]
    fn transition_and_break() {
        let c = Config {
            exercise: Exercise::Transition,
            ..Default::default()
        };
        assert!(measured(&frames(|i| Some(if i < 80 { 0.0 } else { 200.0 }), 160), c).clean);
        assert!(matches!(
            evaluate(
                &frames(
                    |i| if (82..88).contains(&i) {
                        None
                    } else {
                        Some(if i < 80 { 0.0 } else { 200.0 })
                    },
                    160
                ),
                c
            ),
            Result::Unscored(_)
        ));
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    fn pcm_attempt(scoop: bool) -> Measured {
        let rate = 12000.0;
        let mut phase = 0.0f32;
        let pcm: Vec<_> = (0..16800)
            .map(|i| {
                let t = i as f32 / rate;
                let error = if scoop {
                    (-100.0 + t * 500.0).min(0.0)
                } else {
                    0.0
                };
                phase += std::f32::consts::TAU * crate::dsp::hz(48.0 + error / 100.0) / rate;
                if t < 1.0 {
                    0.2 * phase.sin()
                } else {
                    0.0
                }
            })
            .collect();
        let mut detector = crate::dsp::Detector::new();
        let mut segmenter = Segmenter::default();
        for start in (0..pcm.len() - 600).step_by(120) {
            let f = detector.analyze(
                &pcm[start..start + 600],
                rate,
                start as f64 / rate as f64,
                false,
            );
            if let Some(attempt) = segmenter.push(f) {
                match evaluate(&attempt, Config::default()) {
                    Result::Measured(m) => return m,
                    r => panic!("{r:?}"),
                }
            }
        }
        panic!("No completed attempt")
    }
    #[test]
    fn pcm_detector_segmentation_and_scoring() {
        let direct = pcm_attempt(false);
        let scoop = pcm_attempt(true);
        assert!(direct.clean);
        assert!(!scoop.clean);
        assert!(direct.score > scoop.score + 30.0);
    }
    #[test]
    fn short_silence_is_retained_as_interruption() {
        let mut s = Segmenter::default();
        let mut output = None;
        for i in 0..150 {
            let quiet = (50..60).contains(&i) || i >= 100;
            let f = Frame {
                time: i as f64 * 0.01,
                hz: if quiet {
                    None
                } else {
                    Some(crate::dsp::hz(48.0))
                },
                confidence: if quiet { 0.0 } else { 1.0 },
                rms: if quiet { 0.0 } else { 0.1 },
                clipped: false,
                spectrum: None,
            };
            if let Some(segment) = s.push(f) {
                output = Some(segment);
            }
        }
        let frames = output.unwrap();
        assert!(frames.iter().any(|f| f.hz.is_none()));
    }
}
