# SingCoach development

Read `docs/rebuild-overview.md` and the current note at the top of `docs/project-memory.md`. Historical source paths in the handoff describe the former prototype; this rebuild uses `src/`.

Keep the app local, native, and small. Prioritize attacks, transitions, and repeatability. Show one correction per attempt, and abstain when evidence is unreliable. Pitch never identifies physiology, comfort, voice type, or register. Reference audio must remain a pure electronic tone; do not restore synthetic voice or formant logic.

Audio callbacks must not allocate, lock, log, do I/O, or run analysis. Preserve explicit Windows-direct audio under WSL; never silently fall back to WSLg. Keep reference and microphone evidence separate, and require current playback readiness before range progression. Bound session history.

Run locked/offline Cargo tests, Clippy with warnings denied, formatting checks, and a release build for implementation changes. Record hardware checks separately from automated success. Preserve historical reference documents and update the current handoff when making meaningful changes.
