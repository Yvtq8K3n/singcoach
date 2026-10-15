# SingCoach

A local Rust desktop singing coach rebuilt from [the reference brief](docs/rebuild-overview.md). Listen to a pure sine reference, sing a short exercise, pause, and get one correction. Complete five measured repetitions to assess consistency.

## Run

```bash
./run.sh
```

The launcher builds the release binary using the included lockfile and locally cached dependencies. For a first setup without cached crates, run `cargo fetch --locked` with network access first. Linux needs Rust, a C toolchain, ALSA development libraries, and an X11 or Wayland desktop. On Debian/Ubuntu the native build dependencies include `build-essential pkg-config libasound2-dev libxkbcommon-dev libwayland-dev libx11-dev libxi-dev libgl1-mesa-dev`.

On WSL, `run.sh` compiles `platform/windows_player.cs` with the Windows .NET Framework compiler and uses that helper for direct Windows output. The helper is cached next to the release binary. Windows interop and a working WSLg X display are required. No PowerShell is used, and playback never falls back to WSLg audio. Native Linux/Windows/macOS builds use CPAL output. On those platforms you can also use `cargo run --release --locked`.

To isolate output without opening or enumerating a microphone:

```bash
./run.sh --playback-only
```

## Practice

1. Choose a comfortable target and click **Play reference**.
2. For **Clean attack**, sing “ah” for about a second, then pause. Technique mode rewards direct entry; performance mode allows a slide and checks confirmed target arrival.
3. For **Two-note transition**, sing one continuous vowel through both pitches, then pause. Supported intervals are ±2, ±5, and ±7 semitones.
4. Follow the single correction. Only measured attempts count toward the five-attempt set. **New drill** or a configuration change clears that set.
5. The completed set shows mean, standard deviation, best/worst, clean ratio, and reliability (mean minus standard deviation).

**Find my range** is a separate matched-note exploration. Wear headphones so the reference is not mistaken for your voice. Match each guide for a continuous second, explore higher, then use **Feels strained / can't reach** to explore lower. The same control finishes the lower search. Baseline buttons adjust the first target; a stable voice can also establish it. Finishing requires at least two different matched notes and applies a middle matched starting note. Cancelling preserves the prior drill. Silence never establishes a limit, and this workflow does not identify voice type, register, comfort, or absolute vocal limits.

**Settings** selects/refreshes microphones and shows the output route. Automatic selection prefers PipeWire, PulseAudio, then the system default, with fallback only on opening errors. Silence does not switch devices. Space toggles the microphone when controls do not own keyboard input. Focused monitor tabs accept left/right arrows.

The layout follows the companion `singcoach-codex` project, with a 960×760 default window, compact range/microphone status, large monitor tabs, and exercise/feedback panels below the chart. The chart uses the remaining vertical space; short windows can scroll. The compact and full-scale charts distinguish voice, target, and reference. Compact lanes keep distant voice and target ranges visible with explicit gaps. Prepared reference traces remain pinned for comparison after playback. The spectrogram compares generated reference PCM with microphone spectra on the same frequency/gain scale. Saved guide thumbnails can be selected without changing the exercise or scores; **All notes** returns to the combined reference sequence. Spectral zoom/gain controls are under **Zoom and clarity**. Stop capture to freeze/review the chart; use **Latest** to leave history review. Up to 60,000 voice and reference observations and 24 guides are retained, approximately ten minutes of continuous capture. Data is session-only. No microphone recordings or network services are used. Temporary generated WAVs are cleaned on normal helper replacement/exit; crashes can leave them in the OS temp directory.

## Implementation

- `src/audio.rs`: bounded SPSC capture queue, analysis worker, device errors, native phase-continuous oscillator, Windows helper protocol, prepared reference PCM.
- `src/dsp.rs`: custom YIN pitch detection, note math, RMS/clipping and logarithmic spectral measurements.
- `src/training.rs`: segmentation, typed exercise configuration, measured/unscored results, attack/transition evaluation, reliability and range controller.
- `src/ui.rs`: desktop UI and explicit practice workflow, adaptive panel sizing, bounded history, asynchronous reference analysis and audio coordination.
- `src/monitor.rs`: presentation adapted from the companion singcoach-codex project: musical pitch lanes, pinned reference traces, monitor tabs, spectral palette, axes and legends.
- `platform/windows_player.cs`: persistent SoundPlayer process with epoch-tagged readiness/errors; connected examples play from one prepared WAV.

Capture callbacks only normalize/mix samples, flag raw clipping, and push into preallocated storage. Analysis runs on a worker at approximately 12 kHz after decimation. A full queue, stream error, or stopped frame flow discards the partial attempt and requires a capture restart. Ordinary scoring is suppressed during reference playback, for 500 ms afterward, and until 250 ms of observed quiet. Range progression requires the current reference to be ready. Native readiness means the callback has begun processing the request; Windows readiness means the helper loaded and requested playback. Neither measures speaker latency or proves audibility.

## Verification and remaining acceptance

```bash
cargo test --locked --offline
cargo clippy --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
cargo build --release --locked --offline
bash -n run.sh
```

All 22 automated tests pass. Coverage includes reference-layout control placement, minimum-window rendering, guide-review isolation, pinned-reference continuity, synthetic pitches, PCM-to-segmentation/scoring, direct versus scooped entries, connected versus interrupted transitions, abstentions, reliability, playback-gated range matching, WAV correctness, and headless rendering of all views at minimum/default window sizes.

The user confirmed working playback after the helper executable-permission fix. The UI port preserves that audio backend. New desktop launch and audio checks remain unavailable in the coding sandbox: X display access fails, and Windows C# compiler startup fails with a WSL interop socket error. Run `./run.sh` from your ordinary WSL desktop terminal to perform the hardware checks. Verify beep start/stop, held range tones, target changes, microphone selection, headphone range matching and real sung examples. The Windows held-tone route uses SoundPlayer loop replacement, so retune/stop smoothness needs listening validation; the native route smooths pitch and amplitude in its oscillator. The 50 ms pitch windows blur fine timing, periodicity is not a calibrated probability, and speaker leakage/octave errors remain possible. Automated success does not establish singing-measurement accuracy or audible playback.
# singcoach
