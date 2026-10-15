//! Small, allocation-free piano-style instrument shared by both output routes.
//! A struck string has a short attack and upper partials that decay faster than
//! its fundamental. Range guides gently restrike every two seconds.
pub const EXAMPLE_NOTE_SECONDS: f64 = 2.0;
const WEIGHTS: [f32; 8] = [1.0, 0.42, 0.22, 0.12, 0.065, 0.035, 0.018, 0.009];
const DECAY_SECONDS: [f32; 8] = [1.7, 0.9, 0.58, 0.4, 0.3, 0.23, 0.18, 0.14];

struct Voice {
    rate: f32,
    phase: f64,
    step: f64,
    partials: [f32; 8],
    decay: [f32; 8],
    attack: f32,
    release: f32,
    releasing: bool,
}
impl Voice {
    fn new(rate: u32) -> Self {
        let rate = rate as f32;
        Self {
            rate,
            phase: 0.0,
            step: 0.0,
            partials: [0.0; 8],
            decay: DECAY_SECONDS.map(|seconds| (-1.0 / (rate * seconds)).exp()),
            attack: 0.0,
            release: 0.0,
            releasing: false,
        }
    }
    fn strike(&mut self, hz: f32) {
        self.step = std::f64::consts::TAU * hz as f64 / self.rate as f64;
        // End each two-second ringing period at a zero crossing, so the
        // prewarmed Windows loop also starts quietly when first loaded.
        self.phase = (-self.step * self.rate as f64 * 2.0).rem_euclid(std::f64::consts::TAU);
        self.partials = std::array::from_fn(|i| {
            if hz * ((i + 1) as f32) < self.rate * 0.45 {
                WEIGHTS[i]
            } else {
                0.0
            }
        });
        self.attack = 0.0;
        self.release = 1.0;
        self.releasing = false;
    }
    fn damp(&mut self) {
        self.releasing = true;
    }
    fn sample(&mut self) -> f32 {
        if self.release <= 0.0 {
            return 0.0;
        }
        // Generate harmonic sines by recurrence: only one sin/cos pair per voice.
        let (sin, cos) = self.phase.sin_cos();
        let (mut previous, mut harmonic) = (0.0, sin);
        let mut sum = 0.0;
        for (amplitude, decay) in self.partials.iter_mut().zip(self.decay) {
            sum += *amplitude as f64 * harmonic;
            *amplitude *= decay;
            let next = 2.0 * cos * harmonic - previous;
            previous = harmonic;
            harmonic = next;
        }
        self.phase = (self.phase + self.step) % std::f64::consts::TAU;
        let attack = self.attack * self.attack * (3.0 - 2.0 * self.attack);
        let sample = sum as f32 * 0.13 * attack * self.release;
        self.attack = (self.attack + 1.0 / (self.rate * 0.005)).min(1.0);
        if self.releasing {
            self.release = (self.release - 1.0 / (self.rate * 0.04)).max(0.0);
        }
        sample
    }
}

pub struct Piano {
    voices: [Voice; 2],
    current: usize,
    hz: f32,
    period: u32,
    remaining: u32,
    playing: bool,
}
impl Piano {
    pub fn new(rate: u32) -> Self {
        Self {
            voices: [Voice::new(rate), Voice::new(rate)],
            current: 0,
            hz: 130.81,
            period: rate * 2,
            remaining: 0,
            playing: false,
        }
    }
    pub fn strike(&mut self, hz: f32) {
        self.voices[self.current].damp();
        self.current = 1 - self.current;
        self.voices[self.current].strike(hz);
        self.hz = hz;
        self.remaining = self.period;
        self.playing = true;
    }
    pub fn stop(&mut self) {
        self.playing = false;
        for voice in &mut self.voices {
            voice.damp();
        }
    }
    pub fn sample(&mut self) -> f32 {
        if self.playing {
            if self.remaining == 0 {
                self.strike(self.hz);
            }
            self.remaining -= 1;
        }
        self.voices.iter_mut().map(Voice::sample).sum()
    }
}

/// Exercise examples: one two-second piano note, or two connected notes.
pub fn example(first: f32, second: Option<f32>, rate: u32) -> Vec<f32> {
    let change = (rate as f64 * EXAMPLE_NOTE_SECONDS) as usize;
    let len = change * if second.is_some() { 2 } else { 1 };
    let release = (rate as usize / 25).max(1);
    let mut piano = Piano::new(rate);
    piano.strike(first);
    (0..len)
        .map(|i| {
            if i == change {
                if let Some(hz) = second {
                    piano.strike(hz);
                }
            }
            let envelope = ((len - 1 - i) as f32 / release as f32).min(1.0);
            let envelope = envelope * envelope * (3.0 - 2.0 * envelope);
            piano.sample() * envelope
        })
        .collect()
}

/// Warm up one restrike period so the Windows loop has the same ringing tail
/// at both ends. It can repeat indefinitely without resetting at a hard edge.
pub fn range_loop(hz: f32, rate: u32) -> Vec<f32> {
    let period = rate as usize * 2;
    let mut piano = Piano::new(rate);
    piano.strike(hz);
    for _ in 0..period {
        piano.sample();
    }
    (0..period).map(|_| piano.sample()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{self, Detector};
    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
    }
    #[test]
    fn piano_has_decaying_harmonics_and_correct_pitch_across_range() {
        for rate in [12000, 44100, 48000] {
            for note in [36, 48, 60, 72, 84] {
                let pcm = example(dsp::hz(note as f32), None, rate);
                let start = rate as usize / 5;
                let size = rate as usize / 20;
                let f = Detector::new().analyze(&pcm[start..start + size], rate as f32, 0.0, false);
                assert!(
                    dsp::cents(f.hz.unwrap(), note).abs() < 8.0,
                    "{rate}, {note}: {f:?}"
                );
                assert!(
                    rms(&pcm[start..start + size])
                        > rms(&pcm[rate as usize * 4 / 5..rate as usize * 4 / 5 + size]) * 1.3
                );
                assert_eq!(pcm[0], 0.0);
                assert_eq!(*pcm.last().unwrap(), 0.0);
                assert!(pcm.iter().all(|x| x.is_finite() && x.abs() < 0.3));
            }
        }
        let rate = 48000;
        let pcm = example(150.0, None, rate);
        let block = &pcm[4800..9600];
        let amplitude = |harmonic: f64| {
            let (mut s, mut c) = (0.0, 0.0);
            for (i, x) in block.iter().enumerate() {
                let p = std::f64::consts::TAU * 150.0 * harmonic * i as f64 / rate as f64;
                s += *x as f64 * p.sin();
                c += *x as f64 * p.cos();
            }
            2.0 * (s * s + c * c).sqrt() / block.len() as f64
        };
        assert!(
            amplitude(2.0) > amplitude(1.0) * 0.2,
            "Piano must have audible upper partials, not just a sine"
        );
    }
    #[test]
    fn repeating_range_notes_have_no_silent_or_discontinuous_loop_seam() {
        for note in [36, 48, 72, 84] {
            let rate = 48000;
            let pcm = range_loop(dsp::hz(note as f32), rate);
            assert_eq!(pcm.len(), 96000);
            assert!(pcm[0].abs() < 0.00001);
            assert!((pcm[0] - pcm[pcm.len() - 1]).abs() < 0.02);
            assert!(rms(&pcm[..2400]) > 0.02);
            assert!(rms(&pcm[pcm.len() - 2400..]) > 0.015);
            let f = Detector::new().analyze(&pcm[48000..50400], rate as f32, 0.0, false);
            assert!(dsp::cents(f.hz.unwrap(), note).abs() < 8.0);
        }
    }
    #[test]
    fn stop_releases_to_silence_and_same_note_can_be_restruck() {
        let mut piano = Piano::new(48000);
        piano.strike(130.81);
        for _ in 0..4800 {
            piano.sample();
        }
        piano.stop();
        for _ in 0..2400 {
            piano.sample();
        }
        assert_eq!(piano.sample(), 0.0);
        piano.strike(130.81);
        let fresh: Vec<_> = (0..4800).map(|_| piano.sample()).collect();
        assert!(rms(&fresh) > 0.04);
    }
}
