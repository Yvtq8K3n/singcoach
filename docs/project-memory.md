# Current rebuild — 2026-10-16

Clear practice-note recommendation after range — 2026-10-16: user requested a most comfortable note after the check and setting it as target. Existing completion already assigns the actually matched note nearest the matched-span midpoint to config.target; made that behavior visible with Suggested practice note: X in the persistent header and Try X · target set in the completed cue. Tooltip explains the matched-midpoint basis and leaves comfort to user judgment, preserving the no-comfort-inference rule. Matched endpoints and highest sustain remain visible in the header. README updated. Extended the existing completion regression to verify C3 recommendation/target assignment and visible recommendation, cue and sustain at default/minimum sizes. All 74 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Real desktop/microphone/Windows acceptance remains for user verification.

Guide visibility adjustment — 2026-10-16: user found the 35%/1px continuous purple guide too faint. Increased chart and thumbnail guide lines to 65% purple opacity and 1.5px width; adapted the existing rendering assertion. Presentation only. All 74 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Actual desktop visibility remains for user confirmation.

Faint continuous range guide — 2026-10-16: inspected updated image.png; user requested a faint purple line instead of the dotted stroke. Changed each saved guide indicator to one continuous 1px line at 35% purple opacity, including thumbnail guide lines. Updated captions, README and AGENTS; adapted the existing visual indicator assertion. Presentation only; data, timelines, range pacing and scoring unchanged. All 74 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Desktop visual/hardware acceptance remains for user verification.

Guide indicator scope correction — 2026-10-16: user said the combined-chart change seemed broken and clarified the piano should be merely indicative. Removed the expanded paired-frame overlay pipeline and combined Spectrogram layout from the prior turn. Restored prior live microphone and saved-clip timing/layout, prior two-panel Spectrogram behavior and spectrum-stripping pitch overview copies. Saved range pitch review now draws only sparse, faint dotted purple guide-note segments via an explicit PitchGuide presentation type; no synthetic reference observations are generated to follow microphone samples. Existing green live target band, trimmed matched holds, hidden empty prompts, actual sung thumbnail labels and rolling live Compact remain. Thumbnail target indication remains; source-switch controls stay removed. Updated README/AGENTS. All 74 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Removed the obsolete combined-source pipeline regression and extended matched-hold review regression to verify the indicative dotted markers alongside unchanged source/clip behavior. Actual user-reported runtime issue was unspecified; real desktop/microphone/Windows acceptance remains unverified.

Combined range singing/piano chart — 2026-10-16: user requested both sources on one chart instead of switching Your singing/Piano guides. Removed the source-switch controls from history. Range Compact/Full scale now overlay blue microphone traces and purple expected piano target guides, with identical saved-clip time mapping; saved thumbnails retain actual median sung-note labels and include a purple target line. Live Compact stays continuously rolling and only adds the current target line after current reference readiness. Range Spectrogram now uses one microphone-spectrum chart with both pitch lines, rather than separate saved/live panes; mapped saved observations retain spectra for that view. Purple denotes expected pitch, not evidence of continued audio during a silent hold. Empty prompts remain hidden and microphone gaps remain gaps. Ordinary drill scoring/audio and range matching/pacing/three-run cutoff are unchanged. Updated README/AGENTS. All 75 locked/offline tests, Clippy with warnings denied, formatting, launcher syntax and release build pass. Added regression for shared source timestamps, C3 microphone versus C4 target, retained voice gaps/spectra, current-target readiness bounds, both trace colors and no source-switch controls across Compact/Full/Spectrogram at default/minimum sizes. Real desktop/microphone/Windows playback acceptance remains for user verification.

Continuous live range chart — 2026-10-16: user clarified freezing the chart during the pause felt wrong and requested continuous scrolling. Removed the last-reliable-frame viewport freeze; active Compact follows the newest microphone timestamp through silence, breathing pauses and upcoming piano cues. Saved matched holds still begin at confirmation, trim trailing silence and use fitted review layout; empty prompts remain hidden. Range pacing and scoring are unchanged. Updated README/AGENTS and the release regression to confirm live silent history continues while the saved clip stays trimmed. All 74 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Actual desktop/microphone/Windows hardware confirmation remains outstanding.

Range saved holds and pause behavior — 2026-10-16: user requested stopping pane movement after singing, removing spurious numbered No voice entries and excluding the approach slide. Compact active range now freezes its right edge at the last reliable microphone frame during pauses. Singing review uses the fitted overview layout even for one Compact clip instead of the piano-centered pin layout, so saved holds span the review width. Range microphone clips begin only when holding_match becomes true (confirmation frame at time zero); approach/search frames remain live history only. Release, replay or finish trims trailing unreliable/silent frames. Singing thumbnails/overview filter empty prompts and use consecutive visible numbering, while explicit Piano guides retain reference history. The 1,600-observation per-clip and 24-guide bounds, matching thresholds, sustain metrics and audio routing remain unchanged. Updated README and AGENTS. All 74 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Regressions cover excluded wrong-note approach, exact confirmation epoch, bounded post-match capture, separate sources, hidden empty prompts in all views, release trimming, frozen pane and saved hold filling >2.5s of the three-second review viewport. Real microphone/Windows playback and desktop behavior remain for user verification.

Range display polish from image.png — 2026-10-16: user called the view clunky; screenshot showed a large empty upper plot, overlapping old blue clips and thumbnail octave spikes. Compact now fills its grid with notes around the target instead of reserving eleven slots but drawing fewer rows. Active range Compact shows the actual live three-second microphone trace with its current marker until an explicit thumbnail selection; automatic target changes clear selection as before. Saved singing Compact shows only selected/latest clip, while Full scale retains the combined overview. Captions identify live versus saved singing. Thumbnail traces no longer draw connecting lines across large pitch jumps; raw evidence and scoring are untouched. README updated. All 73 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass, including compact-grid target centering/no empty upper area and active-live/selected-review isolation. Actual desktop visual and hardware audio acceptance remain for user verification.

Range lower-end stop and actual singing review — 2026-10-16: user reported endless lower prompts after being unable to match the lowest note and pointed out image.png showed the robot piano staircase rather than their singing. Corrected the prior broad retry rule: unanswered descent prompts can step down while returning at/above the initial discovered baseline, but below it three unanswered ready repetitions finish with matched notes only. Successful low matches still continue; C2/manual finish remain supported. This is a prompt outcome, never a claimed physiological limit. Range guides now retain separate bounded microphone pitch/spectrum clips (1,600 observations per target, within the existing 24-guide bound). Your singing is the default range review source: measured reliable median-note thumbnail labels and blue actual voice traces, with no voice/gaps for missing/clipped/unreliable data. Piano guides remain a separately labeled purple source; generated references are never used as microphone evidence. Both sources work in Compact, Full scale and Spectrogram review. Full scale fits clips across its overview; longer sustain metrics remain independent of the retained clip bound. Updated README and AGENTS. All 71 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. New regressions cover returning from high, stopping the first unanswered below-baseline prompt, continued matched low notes, matched-only endpoints, C3 actual voice against a C4 reference, bounded retention and separate-source rendering in all views. Real microphone/Windows audio and desktop review acceptance remain for user verification.

Range descent retries instead of premature completion — 2026-10-16: user reported the lower search ending after a tiring climb and requested three tries then a lower guide. Three unanswered ready repetitions during Lower/LowerEdge now step down from the current target by the current step, offering another prompt instead of completing. Only manual finish or the supported C2 bound ends descent. Unmatched skipped targets never enter results or become the anchor; smaller-step refinement cannot jump upward toward an old distant anchor after skipped targets. Matched holds and the three-run piano cutoff remain unchanged. Updated UI help, README and AGENTS. All 69 locked/offline tests, Clippy with warnings denied, formatting, launcher syntax and release build pass. Regression covers readiness/early timeout rejection, skipped-target exclusion, recovery by matching a lower note, repeated downward retries and C2 completion with matched notes only. Actual microphone/Windows hardware acceptance remains outstanding.

Range piano cutoff with silent voluntary holds — 2026-10-16: user requested stopping the piano after three runs and clarified that singing should continue until breath/release without piano. Each range guide now stops audio after six current ready seconds; an explicit completed-playback flag preserves only the already matched hold microphone path, sustain timer and release/breathing progression. New confirmations still require current active ready playback. New targets/replays reset the cutoff; unmatched ascent/descent decisions and above-baseline grace remain as before, with no extra piano beyond three runs. Starting-guide retry uses Replay note. Added regression checking the 5.99/6s cutoff, >9s silent matched sustain, no premature progression, release/rest and preserved sustain metric. Updated README and AGENTS. All 69 locked/offline tests, Clippy with warnings denied, formatting, launcher syntax and release build pass. Actual native/Windows audible cutoff and real microphone hold tracking remain for hardware verification.

Melody score and sequence variety — 2026-10-16: user requested a visible score and a new sequence on Play for continued practice. Feedback now prominently labels the existing full-melody in-tune-time measurement as Score: N / 100, with a plain definition and explicit insufficient-data abstention; scoring thresholds remain unchanged. Added four deterministic five-note variants per existing climb/arpeggio family, preserving durations, initial note, transposition bounds and maximum pitch span. First Play uses the displayed sequence; later Play presses advance cyclically, with matching chart/audio/evaluation notes. Family/transposition changes reset the variant; attempt reset preserves rotation. Compact instruction text shows sequence number without adding a layout row. README updated. All 68 locked/offline tests, Clippy with warnings denied, formatting, launcher syntax and release build pass, including variant uniqueness/cycling, C2–C6 bounds, perfect synthetic scoring and silence abstention for each variant, plus minimum-window lifecycle rendering. Hardware microphone/Windows listening acceptance remains outstanding.

Melody waits for singing after piano — 2026-10-16: user reported excessive delay after reference and requested recording begin when singing starts. Removed Melody countdown state and its deadline mapping. Completed Melody playback now arms Listening directly instead of the shared one-second cooldown/quiet gate; Ready waits indefinitely for a reliable microphone frame, which begins recording at time zero just like a voice-only Idle start. Playback frames are drained before this transition and active playback still blocks recording. Manual stop/reset and other exercise cooldown behavior remain unchanged. Updated cue, status, README and lifecycle/clock-offset regressions. All 67 locked/offline tests, Clippy with warnings denied, formatting, launcher syntax and release build pass. Real microphone/Windows transition timing remains unverified; headphones remain recommended to separate reference from microphone evidence.

Melody timeline correction — 2026-10-16: updated image.png showed voice evidence reaching ~4.4s while the cursor/timer read ~2.8s. Melody used sample-clock timestamps for evidence but wall time for countdown recording epochs, cursor, target cues and completion. Map countdown deadline into capture time once; recording timer/target/completion now follow latest capture time, and the monitor cursor follows captured attempt evidence. Playback/countdown remain on wall time. Added regression with a 1.6s clock offset checking first-frame alignment, elapsed time and completion gating. All 67 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Actual microphone clock behavior and hardware acceptance still require a user run; no detector/audio backend changes.

Melody voice-start correction — 2026-10-16: inspected user image.png showing C3 detected in the header while Melody stayed Idle/Ready with no piano. Idle Melody now starts timed recording on the first reliable microphone frame when shared workflow is Listening and playback is stopped, retaining that frame at time zero. Existing piano/countdown flow remains available; cooldown, quiet-wait, clipping, silence and unreliable input cannot trigger a voice start. Results remain visible until New drill or play; New drill arms another voice-only attempt. Updated permanent cue, exercise instructions and README. Added regression for first C3 retention, relative timestamps, playback/workflow/reliability gates and result preservation. All 66 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Real microphone/Windows playback acceptance remains outstanding.

Live melody pitch visibility — 2026-10-16: user reported singing C3 without seeing it (image not received in this session). Found the melody renderer only displayed frames from timed recording. Added a separate blue Live note marker from fresh reliable microphone pitch in all melody monitor views; Compact/Spectrogram bounds include that live note even when below/above the transposed melody. Stale (>250 ms) or unreliable frames hide the marker. Live monitoring never enters attempt evidence/scoring. Added headless C3-outside-transposed-melody regression across all views and stale-frame rejection. All 65 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Hardware/screenshot confirmation remains outstanding.


Melody play-button flow — 2026-10-16: user requested the same play-button interaction as Clean attacks. Replaced separate Hear/Sing buttons with the existing monitor play/stop component beside the instructions. Playback completion enters Ready, waits for current capture and shared Listening (one-second cooldown plus 250 ms observed quiet), then starts the three-second countdown automatically. Stop attempt cancels waiting/countdown or finishes recording; stopping reference returns Idle and cannot trigger a countdown. Pattern/transposition stay locked while waiting. Header cue and README updated. All 64 locked/offline tests, Clippy with warnings denied, formatting, release build and launcher syntax pass. Hardware microphone/Windows playback acceptance remains outstanding.


Shared melody monitor — 2026-10-16: user requested combining the melody path with existing monitor/spectrogram visuals. Moved melody drawing out of melody.rs into monitor.rs. All three existing views now retain the same timed purple piano path, per-note assessments and blue microphone trace; Compact fits the melody, Full scale uses C2–C6, Spectrogram overlays retained microphone spectra using the existing spectral palette. Playback and recording cursors follow their own timelines. Reference path is planned target data, never microphone evidence. Melody capture retains bounded spectra already produced by the worker; scoring and audio callbacks/backends are unchanged. README updated. Verification: all 64 locked/offline tests pass, including shared melody path rendering in all three views at default/minimum sizes. Clippy with warnings denied, formatting, release build and launcher syntax pass. Real desktop/microphone/Windows listening acceptance remains outstanding.


Melody exercise integration — 2026-10-16: user requested Melody be an ordinary exercise reusing the app layout. Added Exercise::Melody to the existing selector, removed the separate header navigation/boolean and standalone page, and split melody controls/results into the existing exercise and feedback panels. The piano roll now occupies the Compact monitor's existing adaptive chart region; Full scale/Spectrogram remain accessible and do not cancel the attempt. Shared begin_example dispatches the piano sequence. Melody now uses the existing Workflow::Reference/Cooldown/AwaitQuiet/Listening gates; removed its duplicate input-after/quiet-since fields. The small melody state retains only sequence-specific countdown, timed recording and results. Common reset/New drill/exercise changes cancel pending melody state and clear the five-attempt set, matching existing exercise-switch behavior; playback-related resets preserve a one-second cooldown. Range takes priority without changing the selected exercise. Ordinary evaluator explicitly abstains for Melody so its per-note timeline cannot be scored as an attack/transition. No audio backend, synthesis or detector changes in this follow-up. README usage updated.

Verification: 64 locked/offline tests pass, including shared-layout rendering at 960×760/720×640, cancellation at playback/countdown/recording/results, playback readiness gating, range priority, and all prior exercise/audio tests. Clippy with warnings denied, formatting, release build and launcher syntax pass. Hardware/audio acceptance remains outstanding; this follow-up did not run real microphone/Windows playback checks.

Melody Coach — 2026-10-16: implemented the attached SINGCOACH_MELODY_COACH_AGENT.md incrementally with no new dependencies. Header navigation selects Exercises / Monitor or Melody Coach without shrinking existing controls. New src/melody.rs owns two five-note presets (C3 climb and arpeggio with unequal explicit durations), bounded current-attempt frames, scoring and an isolated piano roll. Reuses Frame, note math, median, capture/YIN worker, theme/egui, Piano and Player; training/range evaluators and monitor renderer remain unchanged. Player adds a fixed five-note atomic schedule consumed sample-by-sample in the existing native callback (no allocations/locks/I/O/analysis); Windows uses one prepared sequence WAV through the unchanged helper protocol and route. Melody recording uses a three-second visual countdown, capture timestamps relative to its deadline, no playback, one-second output cooldown and 250 ms observed quiet before starting. Playback readiness/errors use the existing Player. Views/configuration changes cancel safely; microphone errors invalidate melody evidence; existing drill results survive view changes. The roll retains targets/voice after completion with dashed blue segments, reliable-data gaps and green/amber/grey plus symbols/tooltips. Default ±35 cents, >=50% reliable coverage, >=70% in-tune voiced time for green, adjustable boundary margins (80 ms default); signed bias, median absolute error, coverage and in-tune ratios are displayed. Clipped or incomplete notes abstain; global in-tune time percentage requires all five assessable complete notes. One correction, no voice-type inference. Current melody frames capped at 1,600, session-only. README documents usage, timing limitations and manual checklist.

Verification: 63 tests pass including all 57 prior tests plus six melody scoring/PCM/lifecycle/headless regressions; locked/offline Clippy with warnings denied, formatting, release build and launcher syntax pass. Real desktop/microphone and Windows sequence audibility/timing/stop remain unverified in this sandbox; automated rendering and synthetic PCM checks are separate from hardware acceptance.

Two-line permanent cue: user refined the compact cue to twice its current height so supporting information remains visible. Set fixed cue height to 60 px, preserving 280 px width/permanent top-right placement. Added 18 px action heading and visible wrapped detail text underneath; icon enlarged proportionally. Hold shows note/time plus release instruction, direction cues show target/high note, and completion retains matched endpoints/highest sustain. Detailed hover text remains. Removed duplicate note/time from the heading now that details are visible again. Geometry regression expects 60 px consistently across active/idle/completed states at all supported tested widths. All 57 tests, Clippy with warnings denied, formatting and release build pass. UI-only; hardware visual acceptance remains for user run.

Compact permanent instruction cue: user clarified that the callout must be about the Find my range button height and remain permanently visible. Replaced the tall card with a fixed 30 px high, 280 px wide cue using smaller vector icons and 16 px one-line instruction text. Target notes appear with higher/lower cues, Hold includes note/live sustain time, and detailed guidance remains on hover. Cue is always reserved at the top right, including idle (Ready), exercise workflow and completed states. Highest sustain is now also in the visible left result summary. UI-only; exercise/range timing unchanged. All 57 tests pass, including cue geometry across active/idle/completed states and 720/800/960/1200 px widths; Clippy with warnings denied, formatting and release build pass. Desktop visual acceptance remains for user run.

Stable header/card layout: user approved interaction/exercise behavior and annotated image.png asking that the callout share the header rows rather than adding vertical blank space, and stop changing between full-width/shrunken cards. Refactored header into controls/live readout/range details on the left and a top-aligned 280 px coaching column on the right at every supported width; removed the 900 px stacking breakpoint. Card reserves its available inner width and 92 px content height (consistent outer dimensions); icon/text top-align; completion headline shortened to “Complete” to fit. Left controls wrap within their column. No timing, range/audio/score logic changed. All 57 tests, Clippy with warnings denied, formatting and release build pass. New rendering regression checks equal card geometry/location across baseline, higher, lower, Hold, direction pause and completion at 720/800/960/1200 px widths, and ensures live readout sits beside rather than beneath the card. Existing minimum/default controls/layout checks pass. Desktop visual acceptance remains for user run.

Faster Hold → Breathe: user reported release transition too slow. Removed the minimum singing-slot wait after a matched release and shortened release detection from 250 ms to 120 ms. After confirmation, quiet or unvoiced/low-confidence background input can release the hold, so room noise above the RMS silence gate no longer keeps Hold open. Clipped/nonfinite input and discontinuities cannot supply old release time; sub-120 ms gaps still do not release. The subsequent breathing pause remains one second, voluntary pitched holds and sustain metrics are preserved. Added regression for prompt release before the old slot end despite background RMS above the silence gate, updated sustain/release checks and docs. All 56 tests pass; Clippy with warnings denied, formatting and release build pass after a minor style fix. Real microphone transition timing remains for user verification.

Voluntary sustain / fun metric / bounded ascent grace: user requested keeping Hold as long as they sustain, a highest-note sustain duration metric, and a fix for delayed descent after singing a higher note in falsetto. Matched guided notes now remain pending until 250 ms of contiguous unclipped quiet; brief quiet gaps and clipped frames cannot release them. After release, wait until the minimum singing slot ends (if needed), then rest one second and advance. Callout shows Hold with live in-band segment seconds and “Release when ready.” Track longest reliable in-band segment at the highest matched note, including reliable initial qualification time; exclude silence, clipping, unreliable/out-of-band frames, reset the current segment on >100 ms gaps/discontinuities, and preserve best session duration. Completion persists and displays Highest sustain (note + seconds). Three-repeat unmatched-note behavior still applies; above-baseline grace gets one extra set of three repetitions, capped at 12 ready seconds per unmatched target, while deliberately matched holds are exempt. No falsetto/register detection or inference was added. All 55 tests, Clippy with warnings denied, formatting and release build pass, including extended voluntary hold, initial listen gating, quiet release/breathing, brief-gap/clipping rejection, sustain persistence/non-inflation, no evidence reuse, bounded ascent grace, and summary/header rendering. Actual mic/Windows playback acceptance remains for user run.

Visual instruction callout: user requested large up/down arrows and short action words, reusable later for exercise guidance. Extracted InstructionCard presentation with vector-drawn icons (up/down, hold, piano/listen, breath wave, completion check), 23px action headings and concise target details. Range headings now “Go higher” / “Go lower”; temporal cues become “Listen,” “Hold,” “Breathe,” and “Breathe · then lower.” Direction remains in details during temporal cues; descending details retain the highest matched note. Full last-matched stage information is available on hover. Component accepts icon/title/detail/color independently of the controller, ready for later drill use; exercise workflow/audio unchanged. All 54 tests, Clippy with warnings denied, formatting and release build pass, including updated direction-title bounds at default/minimum windows and direction-pause cue. Desktop visual acceptance remains for the user.

Slower musical pacing: user approved one-second piano preview, two-second singing slot and one-second breathing pause, with extra space on direction changes. Added guided cadence around existing range recognition: observe_guided ignores preview evidence, latches a successful one-second ±90-cent match, keeps the current target through the remainder of the singing slot, then rests one second before advance. Late matches also receive the full rest; latched matched endpoints survive early Finish. Piano stops during the rest; queued microphone evidence cannot qualify the next note. A first descent transition stops playback and waits two seconds before starting the lower guide. Callouts/progress show listen, matched/finish gently and matched/take a breath cues. Six-second unanswered-prompt timeouts remain based on current ready playback, suppressed after a match; ascent above-baseline grace remains. Raw recognition tests exercise observe, while separate observe_guided tests cover cadence. Exercise PCM and native UI stage timing now share EXAMPLE_NOTE_SECONDS=2.0 (single note two seconds; transitions two seconds per pitch), with one-second post-reference cooldown and updated prompts/guide durations. Scoring thresholds are unchanged. All 54 tests, Clippy with warnings denied, formatting and release build pass, including guided listen/sing/rest ordering, no early advance/evidence reuse, direction-pause backlog gating, shared PCM duration/pitch, and existing minimum/default layout/audio/training checks. Actual Windows piano pacing and microphone acceptance still require a user run.

Range-only estimate field: user explicitly requested an “Estimate” despite the limitations of endpoint-only voice classification. Added completed-header Estimate with hover explanation and approximate endpoint comparison against six educational reference spans verified from Rochdale Music Service (URLs/algorithm documented in README). A2–F4 yields Baritone; close alternatives are shown together. Less than an octave yields insufficient range, more than three octaves yields Multiple ranges, and distant endpoints abstain. This is an unvalidated app heuristic, not confirmed voice type, comfort, register or gender; AGENTS records this user-authorized presentation. All 52 tests, Clippy with warnings denied, formatting and release build pass, including estimate examples, ambiguity, wide/narrow/invalid spans, and header fit at default/minimum sizes. Desktop/hardware acceptance remains for a user run.

Highest-first display: inspected annotated image.png requesting result order match the high-to-low descent and suggesting tenor/alto classification. Swapped header endpoints/placeholders and completion callout/controller summary to Highest → Lowest, preserving the internal (low, high, starting note) tuple and scoring. Voice type remains unclassified because range endpoints alone do not establish tessitura/timbre/registration; explained this limitation to the user. All 51 tests, Clippy with warnings denied, formatting and release build pass. No audio/hardware behavior changed.

Prominent coaching callout: user annotated image.png requesting a visible message in the empty upper-right header space instead of the tiny status line. Added a bordered/tinted coaching card with large stage heading and matched-note details: initial note, “Let’s go higher,” “Now let’s go lower,” and completed range plus encouragement. At widths >=900 px it sits at the right of the header controls; narrower windows place it below the live readout. Removed the duplicate tiny phase/result lines while retaining technical hints and controls. All 51 tests, Clippy with warnings denied, formatting and release build pass, including callout title location/fit for ascent, descent and completion at 960×760 and 720×640. Desktop visual acceptance remains for the user; audio/controller unchanged.

Descent completion / stage messages: inspected the user’s updated image.png (cropped saved guides ending with A2). The lower search previously had no timeout, so unanswered low notes repeated indefinitely. Lower/LowerEdge now finish after three current ready piano repetitions (six seconds). Above-initial-baseline grace remains ascent-only, preventing high off-target singing from holding a low prompt open. Completion includes only matched endpoints; unmatched low guide excluded. Added persistent stage messages: initial setup, “Let’s go up” plus last matched note, “Highest matched … Let’s go down” plus last matched note, and a completed low/high banner after the controller closes. User-requested comfort wording is represented as matched notes because pitch alone cannot measure comfort. Updated AGENTS/README. All 50 tests, Clippy with warnings denied, formatting and release build pass, including lower timeout readiness/early-boundary gating, high-pitch protection isolation, stage note names and unmatched-note exclusion. Hardware acceptance remains outstanding.

Range tolerance follow-up: inspected updated image.png showing target C#4 and voice C4, −79 cents / 98% periodicity, just outside the former ±75-cent band. User reports matching still too hard. Widened range-only tolerance to ±90 cents, keeping one-second duration, 80% reliable/in-band requirements and no quiet lock. Chart green band and header automatically share MATCH_CENTS; median uses the same small numerical edge allowance as the in-band check. Ordinary drills remain unchanged. All 49 tests, Clippy with warnings denied, formatting and release build pass. Expanded recovery regression covers ±79, ±85 and ±90 cents; sustained ±100-cent neighboring notes remain rejected. Real microphone acceptance remains outstanding.

Green-band / one-second correction: inspected the new root image.png screenshot, which shows +64 cents / 98% periodicity inside green while the controller says “Pause, then repeat” after an unclear trial. Root cause was retry quiet-lock suppressing valid subsequent matching. Removed that state and UI cue entirely: matching continues after failed/unclear attempts without silence. Per user request restored guided duration to about one second while keeping initial voice discovery at 400 ms. Recent window requires 80% reliable frames and 80% of reliable pitch within ±75 cents (the actual green band), median inside the band, and a current in-band reliable frame. Brief outliers allowed; touching the purple center for a moment cannot advance. Above-baseline grace survives smaller-step guide changes so continuous retries do not lose descent protection. Updated UI, README, AGENTS and tests. All 48 tests, Clippy with warnings denied, formatting and release build pass, including ±64/±75-cent recovery after a five-second unclear onset without silence, no half-second advancement, one-second wobbly matching, PCM-derived confirmation, readiness and automatic descent checks. Real microphone acceptance remains for a user run.

Above-baseline timeout correction: user reported that the three-run descent should not apply while singing above baseline. Store the initial discovered note separately from the moving guide/anchor. A recent 250 ms window with at least 16 reliable frames, median more than 25 cents above the initial note and SD at most 50 cents postpones descent until six seconds after the last such evidence. This works during retry/quiet-wait states and never counts the off-target note as matched. Clipping, brief pitch glitches and unreliable input do not renew protection; readiness/target changes clear transient evidence. UI and docs describe the exception. All 47 tests, Clippy with warnings denied, formatting and release build pass, including prolonged off-target high singing, grace period expiration and brief glitch rejection. Hardware acceptance remains outstanding.

Three-repeat descent update: user expected the climb to reverse after three unanswered piano runs at a difficult F4. Higher/UpperEdge now automatically descend from the highest matched note after six seconds of current ready playback (three two-second repetitions). This is a prompt timeout, including unanswered/silent prompts, not a measured physiological limit; unmatched targets never enter results. Starting-note confirmation and lower-search completion remain manual/no timeout. Readiness loss and every reference change reset the UI timer; queued matches are processed before timeout. Manual struggle controls remain available. Updated UI help, README and AGENTS to supersede earlier manual-only ascent behavior. All 45 tests, Clippy with warnings denied, formatting and release build pass, including F4 timeout, readiness gating, early-boundary rejection and unmatched-note exclusion. Actual Windows playback timing still requires a hardware run.

Full-scale map update: Full scale now displays all retained saved guides together regardless of individual selection or current sequence. Added overview reference layout that fits every clip across the map with explicit breaks, avoiding the former three-second cropping of earlier guides. Reference time is scaled only for overview; the blue microphone trace remains live. Thumbnail clicks open Compact review; returning to Full restores all guides. Existing 24-guide session bound remains. All 44 tests, Clippy with warnings denied, formatting and release build pass, including a regression fitting 49 note clips without dropping notes or breaks. Desktop/hardware visual verification remains for a user run.

Range screenshot follow-up: the user clarified that nearby pitch should qualify without near-perfect tracking. Guided checks now judge a rolling ~400 ms window: median within ±75 cents, at least 80% reliable frames, and at least 80% of reliable pitch within ±100 cents. Brief excursions and tracking gaps no longer restart an otherwise good phrase; uncertain onsets age out. A single accepted window advances immediately with no required pause or second confirmation. Each target/playback change clears evidence. Silence and clipping clear the window; distinctly wrong sustained notes still fail. Updated range instructions and README/AGENTS. Normal drill scoring and audio routing remain unchanged. All 43 tests, Clippy with warnings denied, formatting and release build pass, including screenshot-like wobble, continuous progression, uncertain-onset recovery and playback reset regressions. Actual microphone behavior still needs the user's hardware run.

Forgiving range update: user reported that one-second near-perfect holds were too difficult. Range discovery and guided matching now take about 400 ms, guided tolerance is ±75 cents, and up to 100 ms of voiced pitch/tracking wobble pauses matching progress without erasing it. Longer gaps, silence, clipping and capture discontinuities still discard the partial hold. Discovery accepts a short moderately wobbly note and listens immediately when no reference tail needs suppressing. Two independent short matches confirm a note, with retries allowed instead of a three-trial cap. Repeated clear misses may reduce the guide step to one semitone, but no longer infer a vocal limit or automatically reverse/finish; the user selects “Struggling — go lower/finish,” with automatic behavior only at supported C6/C2 bounds. This supersedes the stricter range workflow below. Piano/routing and exercise scoring are unchanged.

UI now says “Match briefly · 0.4s” and “retries welcome”; the green band is ±75 cents during range checks, while normal drills keep ±15 cents. Range readout says “Close enough” within its accepted zone. Verification: all 42 tests pass, including short ±70-cent matches, brief wrong-pitch/unvoiced tracking wobbles, rejection of genuinely different notes, long gaps and separated bursts, short wobbling baseline discovery, retry isolation, smaller-step/user-directed search, chart-band size, short PCM-derived holds and all prior audio/training/UI checks. Clippy with warnings denied, formatting, launcher syntax and release build pass. Microphone/piano acceptance still needs a user run.


Voice-first guided range update: user approved repeated confirmation and smaller edge steps, and requested “sing a note → piano guides higher → struggling starts descent to the lowest note.” Added `src/range.rs` with explicit Baseline/ConfirmStart/Higher/UpperEdge/Lower/LowerEdge/Complete states. Initial discovery is voice-only (about 1.2 seconds of stable confident pitch, nearest supported C2–C6 note), with no fabricated target or initial piano. Each guided note requires two independent one-second matches out of up to three measured trials, within ±50 cents, separated by 250 ms quiet. Two clear misses trigger a one-semitone backoff/refinement, then automatic descent from the highest confirmed note; the low edge is refined similarly before completion. User “Struggling — go lower/finish” controls switch immediately. Silence, short/noisy/clipped input and missing playback do not count as reach failures. Replay/readiness failure cannot reuse one continuous hold as a second match. Completion applies a midpoint matched starting note; cancellation preserves the drill.

UI gating discards pre-start microphone backlog and frames captured before the current reference becomes ready. Natural-note discovery hides old pinned guides and has no green target. Audio routing/helper and piano synthesis are unchanged for this range update. Tests: 38 pass, including a complete synthetic climb/refine/descent, PCM-derived natural-note discovery/confirmation, silence/clipping/noise rejection, independent confirmations, replay/continuity gating, supported boundaries, minimum-size range UI and all prior audio/DSP/training/layout checks. Clippy with warnings denied, formatting, launcher syntax and release build pass. Real microphone/piano timing still needs the user's hardware run. The workspace now exposes an existing Git repository initialized by the user (`83c447c`, origin `git@github.com:Yvtq8K3n/singcoach.git`); this task did not commit or push changes.


Piano reference update: user explicitly requested piano for everything with no selector, superseding the historical pure-sine requirement. Added `src/piano.rs`, a generated piano-style instrument shared by prepared Windows WAVs, native playback, and reference guide analysis. Eight harmonic partials have progressively faster decay, with a 5 ms attack and 40 ms damping; old/new notes overlap during transitions. Range guides restrike every two seconds, with a prewarmed periodic WAV for seamless Windows loops. No sampled assets, network dependencies, voice/formant synthesis or UI sound settings. Kept the working Windows helper, routing, permission fix, and training evaluators. Labels and README/AGENTS now describe the single piano reference.

Piano verification: all 25 tests pass, including musical pitch accuracy C2–C6 at 12/44.1/48 kHz, harmonic presence and decay, no silent loop boundary, release/restrike, connected examples, WAV validation and prior UI/training tests. Clippy with warnings denied, formatting, launcher syntax and release build pass. Audible timbre and real hardware acceptance still require user listening; the sandbox cannot launch Windows audio or the desktop. Prior user-confirmed audibility referred to the old sound.


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
