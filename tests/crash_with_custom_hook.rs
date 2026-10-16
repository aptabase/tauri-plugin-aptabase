//! A crash with a custom panic hook: the crash's report and what the hook tracked share the one wait the app can afford,
//! so an endpoint that never answers holds the app up for that wait once, not once for each. (A test binary of its
//! own: the panic hook is set once per process.)

#[path = "../src/test_server.rs"]
mod test_server;

use std::time::{Duration, Instant};

use tauri_plugin_aptabase::{Builder, InitOptions};

#[test]
fn a_crash_waits_for_its_reports_once() {
    // Nothing is ever answered: the crash's report and the hook's event both stay in flight.
    let server = test_server::server(&[0, 0]);
    let _app = tauri::test::mock_builder()
        .plugin(
            Builder::new("A-SH-0000000000")
                .with_options(InitOptions {
                    host: Some(server.base.clone()),
                    // As in a release build: the hook's event is still queued when the app crashes.
                    flush_interval: Some(Duration::from_secs(60)),
                })
                .with_panic_hook(Box::new(|client, _info, msg| {
                    let _ =
                        client.track_event("panic", Some(serde_json::json!({ "message": msg })));
                }))
                .with_crash_reporting()
                .build(),
        )
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();

    let started = Instant::now();
    let crashed = std::thread::Builder::new()
        .name("main".into())
        .spawn(|| panic!("settings are corrupt"))
        .unwrap()
        .join();
    assert!(crashed.is_err());
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_millis(2900), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(3800), "{elapsed:?}");
}
