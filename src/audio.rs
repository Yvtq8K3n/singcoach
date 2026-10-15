use crate::dsp::{self, Detector, Frame};
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
        "Native audio · CPAL sine oscillator"
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
        self.epoch += 1;
        let path = std::env::temp_dir().join(format!(
            "singcoach-{}-{}.wav",
            std::process::id(),
            self.epoch
        ));
        write_wave(
            &path,
            dsp::hz(note as f32),
            second.map(|n| dsp::hz(n as f32)),
            once,
        )?;
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
    let cycles = (freq * 2.0).round();
    let len = if once {
        if second.is_some() {
            76800
        } else {
            48000
        }
    } else {
        (rate as f32 * cycles / freq).round() as u32
    };
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
    if once {
        for sample in render_example(freq, second, rate) {
            data.extend(((sample * 32767.0) as i16).to_le_bytes());
        }
    } else {
        for i in 0..len {
            let x = (std::f32::consts::TAU * cycles * i as f32 / len as f32).sin() * 0.15;
            data.extend(((x * 32767.0) as i16).to_le_bytes());
        }
    }
    file.write_all(&data).map_err(|e| e.to_string())
}
/// Prepared example PCM, shared by Windows playback and reference analysis.
pub fn render_example(first: f32, second: Option<f32>, rate: u32) -> Vec<f32> {
    let len = (rate as f32 * if second.is_some() { 1.6 } else { 1.0 }) as usize;
    let (mut phase, mut current) = (0.0f32, first);
    (0..len)
        .map(|i| {
            let target = if i as f32 >= rate as f32 * 0.8 {
                second.unwrap_or(first)
            } else {
                first
            };
            current += (target - current) / (rate as f32 * 0.01);
            phase = (phase + std::f32::consts::TAU * current / rate as f32) % std::f32::consts::TAU;
            let envelope = (i as f32 / (rate as f32 * 0.005))
                .min((len - 1 - i) as f32 / (rate as f32 * 0.005))
                .min(1.0);
            phase.sin() * 0.15 * envelope
        })
        .collect()
}
struct Native {
    _stream: cpal::Stream,
    frequency: Arc<AtomicU32>,
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
        let rate = config.sample_rate.0 as f32;
        let channels = config.channels as usize;
        let frequency = Arc::new(AtomicU32::new(130.81f32.to_bits()));
        let freq = frequency.clone();
        let enabled = Arc::new(AtomicBool::new(false));
        let active = enabled.clone();
        let failed = Arc::new(AtomicBool::new(false));
        let error = failed.clone();
        let observed = Arc::new(AtomicBool::new(false));
        let ready = observed.clone();
        let (mut phase, mut amp, mut current) = (0.0f32, 0.0f32, 130.81f32);
        macro_rules! stream {
            ($t:ty,$convert:expr) => {
                device
                    .build_output_stream(
                        &config,
                        move |output: &mut [$t], _| {
                            if active.load(Ordering::Relaxed) {
                                ready.store(true, Ordering::Relaxed);
                            }
                            for frame in output.chunks_exact_mut(channels) {
                                let goal = if active.load(Ordering::Relaxed) {
                                    0.15
                                } else {
                                    0.0
                                };
                                amp += (goal - amp) * (1.0 / (rate * 0.005));
                                current += (f32::from_bits(freq.load(Ordering::Relaxed)) - current)
                                    * (1.0 / (rate * 0.01));
                                phase = (phase + std::f32::consts::TAU * current / rate)
                                    % std::f32::consts::TAU;
                                let sample = phase.sin() * amp;
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
            _stream: stream,
            frequency,
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
                n.frequency
                    .store(dsp::hz(note as f32).to_bits(), Ordering::Relaxed);
                n.observed.store(false, Ordering::Relaxed);
                n.enabled.store(true, Ordering::Relaxed);
            }
            Backend::Windows(w) => w.play(note, second, once)?,
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
        assert_eq!(pcm.len(), 19200);
        assert_eq!(pcm[0], 0.0);
        assert_eq!(*pcm.last().unwrap(), 0.0);
        assert!(pcm.windows(2).all(|w| (w[1] - w[0]).abs() < 0.02));
        let mut detector = Detector::new();
        for (start, note) in [(2400, 48), (14400, 55)] {
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
        assert!((samples[0] - samples.last().unwrap()).abs() < 0.004);
    }
}
