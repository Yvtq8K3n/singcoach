use crate::{
    dsp::{self, Detector, Frame},
    piano::{self, Piano},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{traits::*, HeapRb};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

pub fn inputs() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|ds| ds.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}
pub struct Capture {
    _stream: cpal::Stream,
    stop: Arc<AtomicBool>,
    pub failed: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    pub frames: Receiver<Frame>,
}
impl Capture {
    pub fn start(selected: Option<&str>, epoch: Instant) -> std::result::Result<Self, String> {
        let host = cpal::default_host();
        if let Some(name) = selected {
            let device = host
                .input_devices()
                .map_err(|e| e.to_string())?
                .find(|d| d.name().ok().as_deref() == Some(name))
                .ok_or("Selected microphone is unavailable. Refresh Settings.")?;
            return Self::open(device, epoch);
        }
        let mut devices: Vec<_> = host.input_devices().map_err(|e| e.to_string())?.collect();
        let default = host.default_input_device().and_then(|d| d.name().ok());
        devices.retain(|d| d.name().is_ok_and(|n| n != "null"));
        devices.sort_by_key(|d| match d.name().ok().as_deref() {
            Some("pipewire") => 0,
            Some("pulse") => 1,
            n if n == default.as_deref() => 2,
            _ => 3,
        });
        let mut errors = Vec::new();
        for device in devices {
            match Self::open(device, epoch) {
                Ok(c) => return Ok(c),
                Err(e) => errors.push(e),
            }
        }
        Err(format!(
            "Could not open a microphone. Connect a device and refresh Settings. {}",
            errors.join("; ")
        ))
    }
    fn open(device: cpal::Device, epoch: Instant) -> std::result::Result<Self, String> {
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let rate = config.sample_rate.0 as usize;
        let channels = config.channels as usize;
        let rb = HeapRb::<(f32, bool)>::new(rate / 2);
        let (mut producer, mut consumer) = rb.split();
        let failed = Arc::new(AtomicBool::new(false));
        let error = failed.clone();
        let overflow = failed.clone();
        let fail = move |_| {
            error.store(true, Ordering::Relaxed);
        };
        macro_rules! stream {
            ($t:ty,$convert:expr) => {
                device
                    .build_input_stream(
                        &config,
                        move |data: &[$t], _| {
                            for frame in data.chunks_exact(channels) {
                                let value =
                                    frame.iter().map($convert).sum::<f32>() / channels as f32;
                                let clipped = frame.iter().map($convert).any(|v| v.abs() >= 0.995);
                                if producer.try_push((value, clipped)).is_err() {
                                    overflow.store(true, Ordering::Relaxed);
                                }
                            }
                        },
                        fail,
                        None,
                    )
                    .map_err(|e| e.to_string())?
            };
        }
        let stream = match format {
            cpal::SampleFormat::F32 => stream!(f32, |v: &f32| *v),
            cpal::SampleFormat::I16 => stream!(i16, |v: &i16| *v as f32 / 32768.0),
            cpal::SampleFormat::U16 => stream!(u16, |v: &u16| (*v as f32 - 32768.0) / 32768.0),
            _ => return Err(format!("Unsupported microphone format: {format:?}")),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let error = failed.clone();
        let (sender, frames) = mpsc::sync_channel(256);
        stream.play().map_err(|e| e.to_string())?;
        let worker = thread::spawn(move || {
            // Reduce analysis rate to ~12 kHz, outside the audio callback.
            let decimation = (rate / 12000).max(1);
            let analysis_rate = rate as f32 / decimation as f32;
            let size = (analysis_rate * 0.05) as usize;
            let hop = (analysis_rate * 0.01) as usize;
            let mut window = Vec::with_capacity(size);
            let mut clipping = Vec::with_capacity(size);
            let mut block_clip = false;
            let mut detector = Detector::new();
            let mut total = 0usize;
            let mut block_sum = 0.0;
            let mut block_n = 0;
            let mut count = 0;
            let origin = epoch.elapsed().as_secs_f64();
            while !done.load(Ordering::Relaxed) && !error.load(Ordering::Relaxed) {
                let mut any = false;
                while let Some((v, clip)) = consumer.try_pop() {
                    block_clip |= clip;
                    any = true;
                    block_sum += v;
                    block_n += 1;
                    total += 1;
                    if block_n < decimation {
                        continue;
                    }
                    window.push(block_sum / decimation as f32);
                    clipping.push(block_clip);
                    block_clip = false;
                    block_sum = 0.0;
                    block_n = 0;
                    if window.len() == size {
                        let time = origin + (total as f64 / rate as f64)
                            - size as f64 / analysis_rate as f64;
                        let mut frame =
                            detector.analyze(&window, analysis_rate, time, count % 10 == 0);
                        frame.clipped |= clipping.iter().any(|x| *x);
                        if sender.try_send(frame).is_err() {
                            error.store(true, Ordering::Relaxed);
                            break;
                        }
                        window.drain(..hop);
                        clipping.drain(..hop);
                        count += 1;
                    }
                }
                if !any {
                    thread::sleep(Duration::from_millis(2));
                }
            }
        });
        Ok(Self {
            _stream: stream,
            stop,
            failed,
            worker: Some(worker),
            frames,
        })
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

pub fn is_wsl() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .is_ok_and(|s| s.to_lowercase().contains("microsoft"))
}
pub fn route() -> &'static str {
    if is_wsl() {
        "Windows direct · SoundPlayer helper"
    } else {
        "Native audio · CPAL piano"
    }
}
struct Windows {
    child: Child,
    stdin: ChildStdin,
    events: Receiver<String>,
    file: Option<std::path::PathBuf>,
    epoch: u64,
}
impl Windows {
    fn new() -> std::result::Result<Self, String> {
        let exe = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("singcoach-player.exe");
        if !exe.exists() {
            return Err("Windows audio helper is missing. Launch with ./run.sh to build it. WSLg fallback is disabled.".into());
        }
        let mut child = Command::new(exe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Windows audio helper could not start: {e}"))?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, events) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(s) => {
                        if tx.send(s).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            events,
            file: None,
            epoch: 0,
        })
    }
    fn play(
        &mut self,
        note: i32,
        second: Option<i32>,
        once: bool,
    ) -> std::result::Result<(), String> {
        self.play_pcm(note, second, once, None)
    }
    fn play_pcm(
        &mut self,
        note: i32,
        second: Option<i32>,
        once: bool,
        pcm: Option<&[f32]>,
    ) -> std::result::Result<(), String> {
        self.epoch += 1;
        let path = std::env::temp_dir().join(format!(
            "singcoach-{}-{}.wav",
            std::process::id(),
            self.epoch
        ));
        if let Some(samples) = pcm {
            write_pcm(&path, samples, 48000)?;
        } else {
            write_wave(
                &path,
                dsp::hz(note as f32),
                second.map(|n| dsp::hz(n as f32)),
                once,
            )?;
        }
        let translated = Command::new("wslpath")
            .arg("-w")
            .arg(&path)
            .output()
            .map_err(|e| e.to_string())?;
        if !translated.status.success() {
            return Err("Unable to translate the reference WAV path.".into());
        }
        let winpath = String::from_utf8_lossy(&translated.stdout);
        writeln!(
            self.stdin,
            "{}\t{}\t{}",
            if once { "ONCE" } else { "PLAY" },
            self.epoch,
            winpath.trim()
        )
        .and_then(|_| self.stdin.flush())
        .map_err(|e| e.to_string())?;
        if let Some(old) = self.file.replace(path) {
            let _ = std::fs::remove_file(old);
        }
        Ok(())
    }
    fn stop(&mut self) {
        let _ = writeln!(self.stdin, "STOP");
        let _ = self.stdin.flush();
    }
}
impl Drop for Windows {
    fn drop(&mut self) {
        self.stop();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(path) = self.file.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}
fn write_wave(
    path: &std::path::Path,
    freq: f32,
    second: Option<f32>,
    once: bool,
) -> std::result::Result<(), String> {
    let rate = 48000u32;
    let samples = if once {
        render_example(freq, second, rate)
    } else {
        piano::range_loop(freq, rate)
    };
    write_pcm(path, &samples, rate)
}
fn write_pcm(
    path: &std::path::Path,
    samples: &[f32],
    rate: u32,
) -> std::result::Result<(), String> {
    let len = samples.len() as u32;
    let bytes = len * 2;
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(bytes as usize + 44);
    data.extend(b"RIFF");
    data.extend((36 + bytes).to_le_bytes());
    data.extend(b"WAVEfmt ");
    data.extend(16u32.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(rate.to_le_bytes());
    data.extend((rate * 2).to_le_bytes());
    data.extend(2u16.to_le_bytes());
    data.extend(16u16.to_le_bytes());
    data.extend(b"data");
    data.extend(bytes.to_le_bytes());
    for sample in samples {
        data.extend(((sample * 32767.0) as i16).to_le_bytes());
    }
    file.write_all(&data).map_err(|e| e.to_string())
}
/// Prepared example PCM, shared by Windows playback and reference analysis.
pub fn render_example(first: f32, second: Option<f32>, rate: u32) -> Vec<f32> {
    piano::example(first, second, rate)
}

struct Native {
    sequence: Arc<[(AtomicU32, AtomicU32); 5]>,
    sequence_mode: Arc<AtomicBool>,
    _stream: cpal::Stream,
    frequency: Arc<AtomicU32>,
    strike: Arc<AtomicU32>,
    enabled: Arc<AtomicBool>,
    observed: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
}
impl Native {
    fn new() -> std::result::Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No audio output device available.")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = config.channels as usize;
        let frequency = Arc::new(AtomicU32::new(130.81f32.to_bits()));
        let freq = frequency.clone();
        let enabled = Arc::new(AtomicBool::new(false));
        let active = enabled.clone();
        let failed = Arc::new(AtomicBool::new(false));
        let error = failed.clone();
        let observed = Arc::new(AtomicBool::new(false));
        let ready = observed.clone();
        let strike = Arc::new(AtomicU32::new(0));
        let strikes = strike.clone();
        let sequence = Arc::new(std::array::from_fn(|_| {
            (AtomicU32::new(0), AtomicU32::new(0))
        }));
        let schedule = sequence.clone();
        let sequence_mode = Arc::new(AtomicBool::new(false));
        let sequence_active = sequence_mode.clone();
        let rate = config.sample_rate.0;
        let mut notes = [(0.0f32, 0u64); 5];
        let (mut sequence_index, mut remaining, mut sequenced) = (0usize, 0u64, false);
        let mut piano = Piano::new(rate);
        let (mut last_strike, mut was_active) = (0u32, false);
        macro_rules! stream {
            ($t:ty,$convert:expr) => {
                device
                    .build_output_stream(
                        &config,
                        move |output: &mut [$t], _| {
                            let enabled = active.load(Ordering::Relaxed);
                            let epoch = strikes.load(Ordering::Acquire);
                            if enabled && (!was_active || epoch != last_strike) {
                                sequenced = sequence_active.load(Ordering::Relaxed);
                                if sequenced {
                                    for (dst, src) in notes.iter_mut().zip(schedule.iter()) {
                                        *dst = (
                                            f32::from_bits(src.0.load(Ordering::Relaxed)),
                                            src.1.load(Ordering::Relaxed) as u64 * rate as u64
                                                / 1000,
                                        );
                                    }
                                    sequence_index = 0;
                                    remaining = notes[0].1;
                                }
                                piano.strike(if sequenced {
                                    notes[0].0
                                } else {
                                    f32::from_bits(freq.load(Ordering::Relaxed))
                                });
                                last_strike = epoch;
                                ready.store(true, Ordering::Relaxed);
                            } else if !enabled && was_active {
                                piano.stop();
                            }
                            was_active = enabled;
                            for frame in output.chunks_exact_mut(channels) {
                                if enabled && sequenced {
                                    if remaining == 0 {
                                        sequence_index += 1;
                                        if sequence_index < 5 {
                                            piano.strike(notes[sequence_index].0);
                                            remaining = notes[sequence_index].1;
                                        } else {
                                            piano.stop();
                                            sequenced = false;
                                        }
                                    }
                                    remaining = remaining.saturating_sub(1);
                                }
                                let sample = piano.sample();
                                for v in frame {
                                    *v = ($convert)(sample);
                                }
                            }
                        },
                        move |_| {
                            error.store(true, Ordering::Relaxed);
                        },
                        None,
                    )
                    .map_err(|e| e.to_string())?
            };
        }
        let stream = match format {
            cpal::SampleFormat::F32 => stream!(f32, |x: f32| x),
            cpal::SampleFormat::I16 => stream!(i16, |x: f32| (x * 32767.0) as i16),
            cpal::SampleFormat::U16 => stream!(u16, |x: f32| ((x + 1.0) * 32767.5) as u16),
            _ => return Err("Unsupported output format.".into()),
        };
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            sequence,
            sequence_mode,
            _stream: stream,
            frequency,
            strike,
            enabled,
            observed,
            failed,
        })
    }
}
enum Backend {
    Native(Native),
    Windows(Windows),
}
pub struct Player {
    backend: Option<Backend>,
    pub ready: bool,
    pub playing: bool,
    pub error: Option<String>,
    requested: Instant,
}
impl Player {
    pub fn new() -> Self {
        Self {
            backend: None,
            ready: false,
            playing: false,
            error: None,
            requested: Instant::now(),
        }
    }
    pub fn play(&mut self, note: i32) -> std::result::Result<(), String> {
        self.play_notes(note, None, false)
    }
    pub fn example(&mut self, note: i32, second: Option<i32>) -> std::result::Result<(), String> {
        self.play_notes(note, second, true)
    }
    fn play_notes(
        &mut self,
        note: i32,
        second: Option<i32>,
        once: bool,
    ) -> std::result::Result<(), String> {
        self.ready = false;
        self.playing = false;
        self.error = None;
        if self.backend.is_none() {
            self.backend = Some(if is_wsl() {
                Backend::Windows(Windows::new()?)
            } else {
                Backend::Native(Native::new()?)
            });
        }
        match self.backend.as_mut().unwrap() {
            Backend::Native(n) => {
                n.sequence_mode.store(false, Ordering::Relaxed);
                n.frequency
                    .store(dsp::hz(note as f32).to_bits(), Ordering::Relaxed);
                n.observed.store(false, Ordering::Relaxed);
                n.strike.fetch_add(1, Ordering::Release);
                n.enabled.store(true, Ordering::Relaxed);
            }
            Backend::Windows(w) => w.play(note, second, once)?,
        }
        self.playing = true;
        self.requested = Instant::now();
        Ok(())
    }
    /// One fixed five-note sequence; scheduling is sample-based in the native callback.
    pub fn melody(&mut self, notes: &[(i32, u32); 5]) -> std::result::Result<(), String> {
        self.stop();
        self.error = None;
        if self.backend.is_none() {
            self.backend = Some(if is_wsl() {
                Backend::Windows(Windows::new()?)
            } else {
                Backend::Native(Native::new()?)
            });
        }
        match self.backend.as_mut().unwrap() {
            Backend::Windows(w) => {
                w.play_pcm(notes[0].0, None, true, Some(&piano::sequence(notes, 48000)))?
            }
            Backend::Native(n) => {
                for (dst, src) in n.sequence.iter().zip(notes) {
                    dst.0
                        .store(dsp::hz(src.0 as f32).to_bits(), Ordering::Relaxed);
                    dst.1.store(src.1, Ordering::Relaxed);
                }
                n.sequence_mode.store(true, Ordering::Relaxed);
                n.observed.store(false, Ordering::Relaxed);
                n.strike.fetch_add(1, Ordering::Release);
                n.enabled.store(true, Ordering::Relaxed);
            }
        }
        self.playing = true;
        self.requested = Instant::now();
        Ok(())
    }
    pub fn stop(&mut self) {
        if let Some(b) = &mut self.backend {
            match b {
                Backend::Native(n) => n.enabled.store(false, Ordering::Relaxed),
                Backend::Windows(w) => w.stop(),
            }
        }
        self.playing = false;
        self.ready = false;
    }
    pub fn poll(&mut self) {
        let mut error = None;
        if let Some(b) = &mut self.backend {
            match b {
                Backend::Native(n) => {
                    if self.playing && n.observed.load(Ordering::Relaxed) {
                        self.ready = true;
                    }
                    if n.failed.load(Ordering::Relaxed) {
                        error = Some("Audio output disconnected. Retry playback.".into());
                    }
                }
                Backend::Windows(w) => {
                    while let Ok(line) = w.events.try_recv() {
                        if line == format!("READY\t{}", w.epoch) {
                            self.ready = true;
                        } else if let Some(message) = line.strip_prefix("ERROR\t") {
                            if let Some((epoch, message)) = message.split_once('\t') {
                                if epoch == w.epoch.to_string() {
                                    error = Some(message.into());
                                }
                            }
                        }
                    }
                    if w.child.try_wait().ok().flatten().is_some() {
                        error = Some("Windows audio helper exited. Retry playback.".into());
                    }
                }
            }
        }
        if self.playing && !self.ready && self.requested.elapsed() > Duration::from_secs(5) {
            error = Some("Reference playback did not become ready. Retry playback.".into());
        }
        if let Some(e) = error {
            self.stop();
            self.backend = None;
            self.error = Some(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn examples_have_envelopes_and_connected_correct_pitches() {
        let pcm = render_example(dsp::hz(48.0), Some(dsp::hz(55.0)), 12000);
        assert_eq!(pcm.len(), 48000);
        assert_eq!(pcm[0], 0.0);
        assert_eq!(*pcm.last().unwrap(), 0.0);
        assert!(pcm.windows(2).all(|w| (w[1] - w[0]).abs() < 0.08));
        let mut detector = Detector::new();
        for (start, note) in [(2400, 48), (28800, 55)] {
            let f = detector.analyze(&pcm[start..start + 600], 12000.0, 0.0, false);
            assert!(dsp::cents(f.hz.unwrap(), note).abs() < 5.0);
        }
    }
    #[test]
    fn windows_loop_wav_is_non_silent_and_well_formed() {
        let path = std::env::temp_dir().join(format!("singcoach-test-{}.wav", std::process::id()));
        write_wave(&path, 130.81, None, false).unwrap();
        let data = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(path);
        assert_eq!(&data[..4], b"RIFF");
        assert_eq!(&data[8..12], b"WAVE");
        assert_eq!(
            u32::from_le_bytes(data[40..44].try_into().unwrap()) as usize,
            data.len() - 44
        );
        let samples: Vec<_> = data[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();
        assert!(samples.iter().any(|v| v.abs() > 0.14));
        assert!(samples[0].abs() < 0.001);
        assert!((samples[0] - samples.last().unwrap()).abs() < 0.02);
    }
}
