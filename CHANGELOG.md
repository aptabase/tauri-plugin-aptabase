# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.3.0] - 2026-10-16

JavaScript bindings: `@aptabase/tauri` 0.7.0.

### Added

- Error reporting (beta), to the error endpoint (`/api/v0/error`); reports appear on the Errors page of the dashboard
- `ErrorTracker` trait on `App`, `AppHandle` and `Window`: `track_error(&err)` reports any `std::error::Error` with its type name and causes, and `track_error_report(ErrorReport)` reports one described field by field (`ErrorReport::with_fatal` for severity `fatal`)
- `trackError(error, { fatal })` in the JavaScript bindings, with the `aptabase:allow-track-error` permission
- `Builder::with_crash_reporting()`: a panic on the main thread is reported as a fatal `crash` (the app waits up to three seconds for the report before it exits), one in an async task as a `taskException` and one on another thread as `unhandled`; the webview's uncaught errors and unhandled promise rejections are reported too
- Reports are sent right away, one per request, with the same retry rules as the other SDKs: network errors, 408, 429 and 5xx are retried, a 403 (monthly error quota exhausted) and other 4xx are dropped; at most 25 wait for delivery
- Tests for event delivery (batches of 25, what's kept for the next flush and what's dropped, the event format, polling) and for App Key validation, run in CI
- README sections on delivery and on the TLS backend features

### Fixed

- Use the system proxy settings on macOS and Windows again. Turning off reqwest's default features to add the TLS feature flags (#34) also turned off its `system-proxy`, `http2` and `charset` features; they're back on, and the TLS backend is still chosen with `default-tls`, `rustls-tls` or `native-tls`
- `rust-version` is 1.85.0: reqwest 0.13 and rand 0.10 don't build on 1.81
- `flush_events_blocking()`, and the flush after a custom panic hook, panicked with "there is no reactor running" when events were queued and the calling thread was outside Tauri's async runtime (the main thread handling `RunEvent::Exit`, as the README shows); the flush now runs on the runtime

## [1.2.0] - 2026-08-11

### Changed

- Bump all Rust dependencies to latest versions (tauri 2.11.5, tokio 1.53.1, reqwest 0.13.4, rand 0.10.0, serde 1.0.229, serde_json 1.0.151, time 0.3.41, os_info 3.12.0, log 0.4.27, tauri-plugin 2.6.3)
- Bump JavaScript dependencies (@tauri-apps/api ^2.10.1, @rollup/plugin-node-resolve ^16.0.3, @rollup/plugin-typescript ^12.3.0, @rollup/plugin-terser ^1.0.0, rollup ^4.57.1, typescript ^5.9.3)
- Update `rust-version` to 1.81.0 (required for `PanicHookInfo`)
- Update `rustls-tls` feature to use `reqwest/rustls` (reqwest 0.13 renamed the feature)
- Clean up tsconfig.json: remove dead baseUrl/paths, switch to moduleResolution "bundler", add outDir
- Remove hardcoded `moduleResolution: 'node'` from rollup config

### Fixed

- Fix `rand` 0.10 API change: `rand::Rng` → `rand::RngExt` in client.rs
- Fix rollup build error: align tsconfig outDir with rollup output dir
- Resolve issue #27: ENGINE_NAME not found on iOS — already fixed on main, this release publishes the fix to crates.io

## [1.1.0] - 2026-08-11

### Fixed

- Fix runtime panic on startup for apps without `#[tokio::main]` by replacing `tokio::spawn` with `tauri::async_runtime::spawn` (#30, fixes #22, fixes #25)
- Replace deprecated `std::panic::PanicInfo` with `std::panic::PanicHookInfo` (stable since Rust 1.81.0) (#30)
- Bump `@tauri-apps/api` to `^2.7.0` to resolve `window.__TAURI_IPC__` error (#30)

### Added

- TLS backend feature flags for reqwest: `default-tls`, `native-tls`, `rustls-tls` (#34, closes #28)
- Consumers can now opt into rustls for Android and cross-compile targets via `default-features = false, features = ["rustls-tls"]`

### Changed

- `reqwest` is now declared with `default-features = false`; `default = ["default-tls"]` preserves previous behavior (#34)

## [1.0.0] - 2025-03-27

### Changed

- Update to Tauri v2

## [0.5.1] - 2024-02-04

### Fixed

- Fixed an issue where `with_panic_hook` would swallow the panic after logging it

## [0.5.0] - 2023-12-20

### Fixed

- Fixed an issue where `with_panic_hook` would panic

### Added

- Added `msg: String` to `with_panic_hook`

## [0.4.2] - 2023-11-03

### Changed

- Use new session id format

## [0.4.1] - 2023-08-07

### Added

- Automatic flush of events on app exit

### Fixed

- Fix User-Agent header

## [0.4.0] - 2023-08-06

### Changed

- Events are now sent in batches to reduce network overhead
- While offline, events will be enqueue and sent when the app is back online
- Tauri 1.4 required

## [0.3.2] - 2023-07-23

### Fixed

- (macOS) Fixed an issue where sessions could span multiple days if the app was left open overnight

## [0.3.1] - 2023-07-11

### Fixed

- Wait for event to be flushed on panic

## [0.3.0] - 2023-07-10

### Added

- Add ability for panic hook to log panics to aptabase

## [0.2.1] - 2023-05-20

### Added

- Added support for automatic segregation of Debug/Release data source

## [0.2.0] - 2023-05-17

### Changed

- BREAKING CHANGE: replaced the `init` function with a `Builder` struct, see README for example usage

### Added

- Ability to set custom hosts for self hosted servers