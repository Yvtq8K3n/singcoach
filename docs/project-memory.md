# Current rebuild — 2026-10-15

GitHub preparation: user requested committing the app. The sandbox exposes the workspace `.git` as an empty read-only directory, so an in-place commit is unavailable. Created a separate repository export with the configured Git author and a portable `singcoach.bundle` containing the initial app commit on `main`; the workspace source and generated build artifacts remain separate. No remote destination was supplied, so nothing was pushed to GitHub.

Layout replication follow-up: user confirmed the permission fix made this app work, then requested the layout from `/home/marquez/projects/singcoach-codex` (the supplied singular `~/project/` path was a typo). Inspected the reference workspace's UI, audio, DSP, training, helper/launcher, tests and package configuration. Adapted presentation code into `src/monitor.rs` and this app's UI: 960×760 default window, compact header/range summary, large underlined monitor tabs, keyboard tab navigation, adaptive semitone lanes with distant-note gaps and full-scale labels, pinned prepared-reference traces, selectable saved-guide thumbnails with cached spectral previews, combined reference sequences, original spectral palette/axes/legend, collapsible zoom controls, right-side play button and compact exercise/measurement panels. Chart height uses measured remaining space, and very short windows retain vertical scrolling. Changing guide review preserves target, attempts and original voice/reference times. Audio, DSP, training evaluation, Windows helper and executable-permission launcher fix were preserved.

Verification after layout work: 22 tests pass, including layout checks at 720×720, 960×720, 960×760 and 1040×820, minimum-window header/monitor checks at 720×640, completed-summary placement, guide-review isolation, pinned clips with silence boundaries, distant voice/target visibility, and all prior PCM/audio/training tests. Clippy with `-D warnings`, formatting, launcher syntax and release build pass. Inspected a software-rendered 960×760 headless preview; this is presentation validation, not a new hardware playback test. User-confirmed working audio precedes these UI-only changes. The other project was read only and was not modified.


Playback startup follow-up: user reported `Windows audio helper could not start: Permission denied (os error 13)`. The compiled helper was present with mode 0644. Fixed `run.sh` to run `chmod u+x` after the optional compilation, including cached helpers, and repaired the current artifact. Launcher syntax, Cargo tests, Clippy, formatting and release build pass. Actual Windows playback still needs user listening verification; sandbox WSL interop remains unavailable.

The user requested creation of the application from the reference docs. This workspace initially contained only `docs/`; the former Rust workspace described below was unavailable. Implemented a new native `singcoach` Cargo package using cached eframe 0.31.1, CPAL 0.15.3 and ringbuf 0.4.8. Current source is `src/{main,audio,dsp,training,ui}.rs`, with `platform/windows_player.cs`, `run.sh`, Cargo.lock and README. Historical module paths/counts below do not describe the new code.

Implemented microphone/device control, custom YIN pitch detection and logarithmic spectra on a worker, bounded callback queues, native sine output, direct Windows SoundPlayer output under WSL, attack/transition drills, measured/unscored results, five-attempt reliability, guided range matching, compact/full-scale charts, spectrogram comparison, bounded history and asynchronous prepared-reference analysis. Connected Windows examples use one WAV; held range tones use loop replacement. No synthetic voice, accounts, network services or voice recordings. Range progression requires current playback readiness. Cancelling range exploration preserves drill results; completion applies the matched midpoint and resets the drill.

Automated verification: 16 tests pass, including PCM-to-score and headless rendering, with clean Clippy using `-D warnings`. Formatting, launcher syntax and the release build also pass. Hardware acceptance is outstanding: `./run.sh --playback-only` built Rust but Windows compiler startup failed with `WSL ... UtilBindVsockAnyPort ... socket failed 1`. Direct desktop startup failed with `XOpenDisplayFailed`. These are sandbox runtime limits, not evidence that playback is audible or that real voice timing is validated. Windows held-tone retune/stop smoothness especially requires listening. README records launch steps, architecture, thresholds and practical limits. Do not treat the historical passing-test totals below as current.

---

# SingCoach project memory

Last inspected: 2026-10-15. This is a repository handoff, not an automatic
cross-session memory service. Future sessions should read it through AGENTS.md.
Update verified state and the handoff when doing meaningful work; do not infer
unfinished user requests from the backlog below.

## Current handoff

Latest user task: prepare a complete overview to recreate the app from scratch.
Completed `docs/rebuild-overview.md`, covering product behavior, architecture,
measurement definitions, layout/wireframe, 12 use cases, rebuild boundaries and
staged acceptance gates. Linked it from README. Documentation only for this task;
no application changes or new runtime checks. The user has not confirmed the
new Windows helper is audible. Do not resume playback implementation unless
requested; the active direction is now the rebuild brief.

Previous implementation history:

The deep inspection is complete. User reported Find my range is silent when
launched through ./run.sh, and explicitly requested deleting all synthesized
‘ah’ sound logic. The first restoration changed the waveform but did not fix
that silence; do not call audible playback verified.

Removed vocal_tone.rs, formant/harmonic generation, wavetable interpolation,
table crossfades/retune queue and breath-like amplitude changes. Native reference
now uses a scalar phase-continuous sine oscillator; Windows loops render sine PCM
directly. Keep click-free envelopes, smooth pitch changes and Windows-direct
routing. Exercise instructions still ask the user to sing ‘ah’; that is separate
from synthesizing a voice.

Latest verification: 47 workspace tests pass (two obsolete wavetable tests removed,
one sine-purity regression added); release desktop binary rebuilt successfully.
Audio Clippy passes with the same three pre-existing audio warnings.

Silence follow-up: user confirmed ./run.sh → Find my range is silent. Non-silent
WAVs and logs without player startup narrowed this to Windows player startup.
User's PowerShell Write-Output probe timed out (124), even from System32; CMD echo
succeeded (0). PowerShell startup is failing independently of SingCoach.

Replaced PowerShell script/base64 launch with windows_player.cs: a small compiled
.NET Framework SoundPlayer helper using the existing WAV/READY/DONE protocol.
./run.sh builds it once with Windows csc.exe (30-second timeout), caches it beside
the release binary, and reuses it. Windows interop is blocked in this coding
sandbox, so actual helper compilation/playback awaits user launch. A broader .NET
startup fault is still possible; do not promise the helper fixes that. WSL output
selection now always chooses Windows; missing helper errors rather than using WSLg.
Rust workspace tests pass (46) and release build passes. bash -n run.sh passes.
Other inspection findings remain follow-ups, not approved unrelated scope.

## Read first

- `docs/rebuild-overview.md`: self-contained brief for the planned fresh rebuild.
- `AGENTS.md`: product priorities and real-time constraints.
- `docs/audio-troubleshooting.md`: existing detailed, user-confirmed playback
  investigation and routing requirements. Preserve this history.
- This file: implementation map, current checks, discrepancies and open issues.
- `README.md`: extensive usage instructions, but with stale sections listed below.
- `SPEC.md`, `ARCHITECTURE.md`, `TASKS.md`: original design/milestones; they are
  not reliable statements of everything currently implemented or validated.

## Product constraints to preserve

Personal native Rust singing coach: prioritize clean attacks, transitions and
repeatability. One correction per attempt; deterministic measurements may abstain.
Technique and performance modes have different goals. Do not infer anatomy,
comfort, voice type or register from pitch. Keep UI, analysis and training separate.
No accounts, network services, songs/catalog expansion or premature curriculum.
Callback code must avoid allocation, locks, I/O, logging and heavyweight analysis.

## Implemented behavior and code map

Workspace uses Rust 2021, CPAL 0.15.3, ringbuf 0.4.8 and eframe 0.31.1, with
Cargo.lock present. About 4,400 lines in the main Rust source modules, plus tests,
desktop entry and example. No custom unsafe code found in project Rust sources.

| Area | Entry points and responsibility |
| --- | --- |
| Desktop | `apps/desktop/src/main.rs`: 960×760 window, minimum 720×640; `--playback-only` skips input enumeration/startup capture. |
| Shared values | `crates/singcoach-core/src/lib.rs`: MIDI/Hz/cents/note names; `PitchFrame` has optional Hz, timestamp, periodicity, RMS and clipping. `SpectrumFrame` holds 192 bins. |
| Capture | `crates/singcoach-audio/src/lib.rs`: input enumeration, default selection, PCM normalization/channel averaging, SPSC queue, analysis worker and shutdown. |
| Pitch | `crates/singcoach-dsp/src/lib.rs`: offline-callable `PitchDetector` trait and custom YIN implementation; no external pitch detector dependency. |
| Spectrum | `crates/singcoach-dsp/src/spectrum.rs`: mean removal, Hann window and Goertzel-style measurements on 192 logarithmic frequencies, 65–4000 Hz, floor −120 dBFS. |
| Training | `crates/singcoach-training/src/lib.rs`: thresholds, acoustic activity segmentation, shared result and five-attempt summary. |
| Exercises | `crates/singcoach-training/src/exercises/`: catalog, instructions/labels, attack analyzer and two-note transition analyzer. |
| Range | `crates/singcoach-training/src/calibration.rs`: baseline following, matched holds, up/down search, suggested interior starting note. |
| Reference | `crates/singcoach-audio/src/reference.rs`: backend selection, single/connected examples, held tone, retuning, visualizer and CPAL output. |
| Windows output | `crates/singcoach-audio/src/windows_reference.rs` and `windows_player.cs`: compiled Windows SoundPlayer helper, stdin protocol, READY/DONE messages, WAV preparation and preview worker. |
| Tone | `HeldTone` in `reference.rs`: simple sine oscillator; Windows loops generate sine PCM directly. No vocal synthesis module remains. |
| WAV diagnostics | `crates/singcoach-audio/src/output_wav.rs`: callback tap → SPSC → file worker; records reference output, not microphone input. |
| UI | `crates/singcoach-ui/src/lib.rs`: `CoachApp`; controls, history/review, playback coordination, calibration workflow, exercise scoring dispatch and egui charts. Largest module (~1,680 lines), including headless tests. |

Capture starts automatically; Linux automatic input selection prefers pipewire,
then pulse, then default, then other devices, excluding null. Only opening errors
trigger fallback, never silence. Input callback averages normalized channels and
pushes into a preallocated half-second SPSC buffer. Errors/overflow set atomics.
Worker uses 50 ms windows with ~10 ms hop; pitch channel capacity 256, spectrum
128. A full/disconnected pitch channel marks capture failed; spectra are best
effort. UI stops capture and discards the pending segment on reported failure.

YIN supports approximately 65–1100 Hz; RMS below 0.008 is unvoiced and any
sample magnitude ≥0.995 marks the analysis window clipped. Confidence is
periodicity, not calibrated probability. Window-start sample time is mapped to
session time by UI offsets. No separate sophisticated pitch tracker is present.

Segmentation ends after 200 ms silence and retains shorter gaps so settling cannot
bridge interruptions. More than 1,500 frames discards an attempt and waits for
silence. Attack scoring requires ≥350 ms, ≥80% valid observations, clear early
frames, no clipping and strictly ordered timestamps without gaps >25 ms.
Initial error is the median through the first 30 ms. Settlement requires 100 ms
within ±15 cents. A clean technique attack starts within ±20 cents, settles
within 80 ms and has settled median within ±15 cents. Sustain uses confident
observations from settlement onward, not just the qualifying hold.

Technique score: clamp(100 − 0.6×absolute initial cents − 0.15×settling ms
− 0.4×absolute settled cents, 0, 100). Unsettled attempts use 1,000 ms penalty.
Performance attack score is binary confirmed arrival (100) or no arrival (0).
Exactly five scored attempts produce mean, population SD, best/worst, clean ratio
and reliability clamp(mean − SD, 0, 100); abstentions do not enter the five.

Transitions are already implemented. UI offers ±2/5/7 semitones, examples hold
each note ~0.8 seconds, and the target guide changes at 0.8 seconds. Analysis
detects actual departure/arrival rather than enforcing that cue time. Requires
≥800 ms total, ≥90% valid frames and a clear first 250 ms within ±35 cents.
Departure leaves ±40 cents; arrival holds ±15 cents for 100 ms. Clean transition
requires ≤80 ms movement, arrival median within ±15 cents and spread ≤20 cents.
Score penalizes movement time, arrival error and spread above 20 cents. Broken
voicing during movement abstains. Transition UI hides the mode control; its
evaluator ignores Mode and applies technique criteria.

Range check is independent of exercise scoring. Starts at C3 or this session's
accepted starting note. Before first match, ~1.5 seconds of clear stable voice
can move the baseline. One continuous one-second match within ±75 cents advances
the target; held data is then validated. Search rises by two semitones, then the
user's strain/limit button starts descent below the first match. Bounds C2–C6.
No silence timeout infers a boundary. Recommendation is a matched note nearest
the checked span's midpoint, with accuracy and lower-note tie breaks. Completion
automatically applies the result in `CoachApp::update`. State is session-only.

Compact/full-scale pitch and spectrogram tabs share retained session history.
Reference and microphone histories are separate. New drill/target/module changes
reset scoring but preserve chart data. Review windows copy/filter history rather
than deleting it. Playback suppresses normal scoring, followed by a 500 ms
cooldown and 250 ms observed continuous quiet. Exercise prompt steps describe
listening/repeating but do not strictly require example playback before scoring.

## Audio routing: preserve the workaround

Existing user-confirmed evidence: WSLg output can sound squishy even when generated
and Linux monitor WAVs are clean. Playback-only and standalone paplay reproduced
it; simultaneous microphone capture is not required. Windows-direct playback was
clean. Exact downstream fault remains unknown; do not claim a proven cause.

Backend selection uses Microsoft kernel detection. WSL always selects the
Windows helper beside the running executable, built by ./run.sh; missing helper
or launch/runtime failure propagates without a WSLg retry. One process stays
alive for held-note changes. WAV paths are translated with wslpath and sent as
data; no PowerShell or shell code is involved. Previews wait for the current
READY epoch and then use elapsed time; they do not measure speaker latency.
CPAL uses callback sample counts for previews. Held references preview only a
short prepared window.
Native held retuning updates scalar oscillator frequency through atomics; callback
performs no waveform allocation or file writing.

Diagnostic reference WAVs (and Windows logs) remain in the OS temporary directory.
They are not microphone recordings. No automatic artifact cleanup was found.

## Verification on 2026-10-15

Environment: rustc/cargo 1.99.0. Cached dependencies and native development
libraries were sufficient for an offline workspace build.

- `cargo test --workspace --locked --offline`: PASS, 48 tests (audio 17,
  core 1, DSP 5, training 12, training pipeline 2, UI 11). Desktop/doc tests: 0.
- `cargo clippy --workspace --all-targets --locked --offline`: exits successfully
  with 7 distinct warnings: manual is_multiple_of, clone_on_copy, test
  chunks_exact_to_as_chunks, two unnecessary_unwrap warnings and two
  too_many_arguments warnings. No warnings-as-errors setting in current CI.
- `cargo fmt --all -- --check`: FAIL; extensive existing formatting differences.
  No formatting rewrite was performed.
- No microphone, actual desktop window, audible playback or Windows interop
  acceptance tests were performed in this inspection. Synthetic/headless success
  does not validate real voice timing, pitch confidence or user comfort.
- `.git` is exposed as an empty/read-only directory here; git status/log report
  “not a git repository”. No commit history or change baseline was available.

Tests cover generated PCM, pitch math, noise/silence/clipping, direct versus
scooped attacks, continuity/abstention, range search, waveform/loop properties,
Windows command protocol/readiness, layout and chart history. Transition analyzer
tests use supplied pitch frames; no end-to-end connected transition PCM scoring
test was found. No recorded real-singing fixtures were found.

## Open findings and recommended follow-ups

These are inspection findings, not fixes made in this session. Recheck code
before acting; locations are named by function to survive line-number changes.

1. **Range check can advance without working reference playback.** In UI `poll`,
   readiness blocks only when `example` is Some and not ready. `play_example`
   clears `pending_calibration_example` before attempting playback; failure sets
   an error and leaves no example. Asynchronous errors also remove the example
   via `stop_example`. With calibration still active, later voice frames can
   pass hold validation with no output playing. Confirmed by control-flow
   inspection, not a hardware reproduction. Add failure-state coverage before
   fixing; accepting a hold should depend on a current ready reference.
2. **Resolved selection gap:** WSL detection no longer depends on PowerShell
   availability. A missing compiled Windows player is an explicit error; native
   output is used only outside WSL. Audible validation remains outstanding.
3. **Reference timestamps omit Windows startup delay.** `play_example` sets the
   session offset when requesting playback, while the preview worker waits for
   READY and then emits relative timestamps starting at zero. Thus data waits
   correctly but appears earlier than actual playback on the shared session
   timeline by startup/load delay. Relative pinned guides can hide this. Code
   inspection finding; measure alignment before changing visual synchronization.
4. **Measurement validity still needs real singing/hardware validation.**
   Overlapping 50 ms windows blur onset and transition timing relative to the
   80 ms clean threshold. Fixed RMS gate, octave errors, background noise and
   speaker leakage remain limitations. Profile DSP and UI before optimization;
   YIN and spectral analysis share one worker. No measured latency budget exists.
5. **Long-session memory/work grows.** Voice pitch/spectrum, reference histories
   and guide textures are retained without a cap. Spectrum payload alone is
   ~77 KB/s (~276 MB/hour) at 100 frames/s, excluding container overhead and
   reference data. UI window extraction scans retained data. This is documented
   behavior, not a measured performance failure. Preserve review/history intent
   if introducing storage bounds or indexing.
6. **UI and shared result coupling.** UI owns substantial workflow state;
   transitions reuse attack-named fields and carry distinct labels. The catalog
   provides a useful seam, but any further exercises should avoid expanding the
   central UI state machine unnecessarily. The exercise heading still says
   “Start directly on one note” even for transitions (`render`).

## Documentation discrepancies

- README final implementation-environment paragraph and TASKS introduction say
  Rust is unavailable and the code has never compiled. Superseded by checks above.
- Cargo.lock exists; README's instruction to generate it after the first build
  is historical. CI currently runs tests/Clippy without --locked and no fmt check.
- README measurement section says transitions are deferred, while both code and
  its earlier usage section include them.
- README range paragraph describes repeated-note ranking by mean error/spread;
  current `StartingNoteCheck::finish` uses midpoint proximity, accuracy and note
  tie breaks, without that repeat-ranking algorithm.
- “Saved” range results are held in `personal_start` for this session, not saved
  to disk. README does mention session visibility; avoid implying persistence.
- ARCHITECTURE begins with a suggested module layout and hypothetical event
  structs; use actual code/current map above for implementation details.
- TASKS checkboxes conflate implementation, automated verification and manual
  acceptance. Do not check all of them solely because automated tests pass.

## Next-session workflow

Read AGENTS.md and this handoff; read audio troubleshooting before any output
routing work. Confirm the current user's task. Recheck affected code and relevant
tests, then implement only the authorized scope. Use offline/locked Cargo checks
when appropriate. Record changes, verification and remaining hardware checks here.
Use `./run.sh` for the normal release launch; it disables Wayland under WSL to
prefer X11. The audio troubleshooting document also records an alternative WSLg
runtime-directory launch setting; neither was exercised in this inspection.
