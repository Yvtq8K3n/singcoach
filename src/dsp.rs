#[derive(Clone, Debug)]
pub struct Frame {
    pub time: f64,
    pub hz: Option<f32>,
    pub confidence: f32,
    pub rms: f32,
    pub clipped: bool,
    pub spectrum: Option<Vec<f32>>,
}
pub fn hz(note: f32) -> f32 {
    440.0 * 2.0_f32.powf((note - 69.0) / 12.0)
}
pub fn midi(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}
pub fn cents(hz: f32, target: i32) -> f32 {
    (midi(hz) - target as f32) * 100.0
}
pub fn name(note: i32) -> String {
    let names = [
        "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
    ];
    format!(
        "{}{}",
        names[note.rem_euclid(12) as usize],
        note.div_euclid(12) - 1
    )
}
pub struct Detector {
    diff: Vec<f32>,
    norm: Vec<f32>,
}
impl Detector {
    pub fn new() -> Self {
        Self {
            diff: Vec::new(),
            norm: Vec::new(),
        }
    }
    pub fn analyze(&mut self, x: &[f32], rate: f32, time: f64, spectral: bool) -> Frame {
        let mean = x.iter().sum::<f32>() / x.len() as f32;
        let rms = (x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / x.len() as f32).sqrt();
        let mut f = Frame {
            time,
            hz: None,
            confidence: 0.0,
            rms,
            clipped: x.iter().any(|v| v.abs() >= 0.995),
            spectrum: None,
        };
        if spectral {
            f.spectrum = Some(
                (0..64)
                    .map(|i| {
                        let freq = 65.0 * (4000.0_f32 / 65.0).powf(i as f32 / 63.0);
                        let step = std::f32::consts::TAU * freq / rate;
                        let coeff = 2.0 * step.cos();
                        let (mut a, mut b) = (0.0, 0.0);
                        for (j, v) in x.iter().enumerate() {
                            let w = 0.5
                                - 0.5
                                    * (std::f32::consts::TAU * j as f32 / (x.len() - 1) as f32)
                                        .cos();
                            let c = (v - mean) * w + coeff * a - b;
                            b = a;
                            a = c;
                        }
                        let amp =
                            (a * a + b * b - coeff * a * b).max(0.0).sqrt() * 4.0 / x.len() as f32;
                        (20.0 * amp.max(0.000001).log10()).max(-120.0)
                    })
                    .collect(),
            );
        }
        if rms < 0.008 {
            return f;
        }
        let max = ((rate / 65.0) as usize).min(x.len() / 2 - 1);
        let min = (rate / 1100.0) as usize;
        self.diff.resize(max + 1, 0.0);
        self.norm.resize(max + 1, 1.0);
        let mut sum = 0.0;
        for lag in 1..=max {
            let d = (0..x.len() - max)
                .map(|j| (x[j] - x[j + lag]).powi(2))
                .sum::<f32>();
            self.diff[lag] = d;
            sum += d;
            self.norm[lag] = if sum > 0.0 { d * lag as f32 / sum } else { 1.0 };
        }
        let mut lag = min.max(2);
        while lag < max {
            if self.norm[lag] < 0.15 {
                while lag + 1 < max && self.norm[lag + 1] < self.norm[lag] {
                    lag += 1;
                }
                break;
            }
            lag += 1;
        }
        if lag >= max {
            return f;
        }
        f.confidence = (1.0 - self.norm[lag]).clamp(0.0, 1.0);
        let (a, b, c) = (self.norm[lag - 1], self.norm[lag], self.norm[lag + 1]);
        let denom = a - 2.0 * b + c;
        let shift = if denom.abs() > 1e-9 {
            (0.5 * (a - c) / denom).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        f.hz = Some(rate / (lag as f32 + shift));
        f
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notes_round_trip() {
        for n in 36..=84 {
            assert!((midi(hz(n as f32)) - n as f32).abs() < 0.001);
        }
        assert_eq!(name(48), "C3");
    }
    #[test]
    fn noise_abstains_and_clipping_is_flagged() {
        let mut seed = 12345u32;
        let noise: Vec<f32> = (0..600)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                ((seed >> 8) as f32 / 16777216.0 - 0.5) * 0.3
            })
            .collect();
        let mut detector = Detector::new();
        assert!(detector.analyze(&noise, 12000.0, 0.0, false).hz.is_none());
        let clipped: Vec<f32> = (0..600)
            .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / 12000.0).sin() * 1.1)
            .collect();
        assert!(detector.analyze(&clipped, 12000.0, 0.0, false).clipped);
    }
    #[test]
    fn synthetic_pitch_and_silence() {
        let mut d = Detector::new();
        for freq in [65.41, 130.81, 220.0, 440.0, 1046.5] {
            let x: Vec<_> = (0..2400)
                .map(|i| 0.2 * (std::f32::consts::TAU * freq * i as f32 / 48000.0).sin())
                .collect();
            let f = d.analyze(&x, 48000.0, 0.0, false);
            assert!(
                (1200.0 * (f.hz.unwrap() / freq).log2()).abs() < 8.0,
                "{freq}: {:?}",
                f.hz
            );
        }
        assert!(d
            .analyze(&vec![0.0; 2400], 48000.0, 0.0, false)
            .hz
            .is_none());
    }
}
