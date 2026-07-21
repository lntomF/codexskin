# CodeSkin Roadmap

> 中文版：[查看路线图](../../ROADMAP.md)

This roadmap describes current priorities, not guaranteed delivery dates.
Priorities may change based on user feedback, target-application updates, and
security or compatibility needs.

## Guiding principles

CodeSkin should remain local-first, reversible, respectful of user privacy,
independent from official application installation files, transparent about
compatibility limits, safe for community contribution, and visually expressive
without compromising readability.

## Current foundation

- Windows desktop application
- Local target-app detection
- Local CDP connection and runtime visual injection
- Background import and local storage
- Theme persistence
- Theme-application verification
- Restore-official-appearance workflow
- Windows release packaging

## Near-term priorities

### Compatibility and reliability

- Improve target-app detection and diagnostics.
- Document tested application versions.
- Make restore behavior more visible and reliable.
- Add regression coverage for target detection and restoration.

### Better first-run experience

- Explain the local-only connection model clearly.
- Explain what CodeSkin changes and does not change.
- Improve empty, error, and reconnect states.
- Make safe restoration easy to find.

### Theme quality

- Add original or clearly licensed starter themes.
- Improve contrast and readability controls.
- Support clean, low-distraction, and accessibility-focused presets.
- Establish a safe community theme-submission process.

### Community health

- Add contribution guidance and issue templates.
- Build a compatibility-report library.
- Label beginner-friendly issues.
- Publish concise release notes and maintenance updates.
- Recognize documentation, testing, and theme contributors.

## Later exploration

These items will be considered only after user validation:

- per-project visual profiles;
- theme import and export;
- optional settings backup;
- curated community theme gallery; and
- visual modes for recording or presentations.

## Explicit non-goals

CodeSkin does not aim to replace Codex, ChatGPT, or official agent workflows;
modify official application installation files; manage model-provider API keys;
distribute unlicensed images; execute arbitrary scripts bundled with community
themes; or promise permanent compatibility with every future target-app version.
