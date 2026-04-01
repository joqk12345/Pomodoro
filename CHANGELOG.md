# Changelog

All notable changes to this project will be documented in this file.

## 0.5.0 - 2026-04-01

Initial runnable desktop baseline generated from `dev-spec.md`.

### Added

- Bootstrapped a Tauri 2 + React 19 + TypeScript + Vite desktop application
- Added local SQLite persistence with `tasks`, `sessions`, `interruptions`, and `app_state`
- Implemented Tauri commands for:
  - app snapshot querying
  - JSON export
  - task create/update/move/delete/toggle completion
  - timer start/pause/resume/complete/abort
  - interruption recording
- Implemented persistent `active_timer` recovery and `cycle_focus_count`
- Implemented `focus / short_break / long_break` phase flow with fixed `25 / 5 / 15` rhythm
- Implemented `overlearning` behavior for tasks completed before focus time ends
- Implemented interruption handling with `postpone`, `pause`, and `abort`
- Added four-panel desktop UI:
  - `Today`
  - `Focus`
  - `Records`
  - `Analytics`
- Added today overview, recent history, 14-day trend, 7-day completed vs aborted, day/week comparisons, and estimate audit
- Added export download flow on the frontend
- Added default Tauri capability and icon asset
- Added project documentation baseline in `README.md`

### Verified

- Frontend build passed via `npm run build`
- Rust/Tauri backend check passed via `cargo check`
