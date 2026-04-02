# Changelog

All notable changes to this project will be documented in this file.

## 0.6.1 - 2026-04-02

Import and desktop dialog fix release.

### Added

- Added full JSON import on the frontend and backend
- Added compatibility parsing for legacy exports with top-level `activeTimer`
- Added import validation for ids, enums, timestamps, references, and non-negative counters
- Added import regression tests, including an external fixture path test
- Added Tauri dialog plugin integration for native file selection

### Changed

- Import now replaces local SQLite data atomically after validation
- Sidebar import flow now uses the native Tauri file picker instead of a hidden browser file input
- Default desktop capability now explicitly grants `dialog:allow-open`

### Verified

- Frontend build passed via `npm run build`
- Rust/Tauri backend check passed via `cargo check`
- Legacy import test passed via `cargo test imports_legacy_payload_shape`
- External fixture import test passed via `env POMODORO_IMPORT_FILE=/Users/mac/Downloads/test-podomo.json cargo test imports_external_fixture_when_requested`

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
