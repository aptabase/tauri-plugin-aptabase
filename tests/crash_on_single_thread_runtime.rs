//! A crash on the only thread of the runtime Tauri uses: an app can hand Tauri a single-thread runtime
//! (`tauri::async_runtime::set`), and a panic on that runtime's thread stops the one thread that could run anything
//! spawned on it, a send already in flight included. The crash report must still be delivered before the hook
//! returns. (A test binary of its own: the runtime Tauri uses is set once per process.)

#[path = "../src/test_server.rs"]
mod test_server;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use serde_json::Value;
use tauri_plugin_aptabase::{Builder, InitOptions};

#[test]
fn a_crash_on_the_runtimes_only_thread_is_still_delivered() {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap(),
    );
    tauri::async_runtime::set(runtime.handle().clone());

    // The first report is never answered: its send is still in flight when the app crashes.
    let server = test_server::server(&[0, 202]);
    let _app = tauri::test::mock_builder()
        .plugin(
            Builder::new("A-SH-0000000000")
                .with_options(InitOptions {
                    host: Some(server.base.clone()),
                    ..Default::default()
                })
                .with_crash_reporting()
                .build(),
        )
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();

    // The main thread (Rust names it "main") running the runtime, as `#[tokio::main(flavor = "current_thread")]` does.
    let on_runtime = runtime.clone();
    let started = Instant::now();
    let crashed = std::thread::Builder::new()
        .name("main".into())
        .spawn(move || {
            on_runtime.block_on(async {
                // A task's panic runs on this thread too, but only the task dies; its report starts on its way.
                let _ = tokio::spawn(async { panic!("sync failed") }).await;
                tokio::time::sleep(Duration::from_millis(300)).await;
                panic!("settings are corrupt");
            })
        })
        .unwrap();
    assert!(crashed.join().is_err());
    let elapsed = started.elapsed();

    let sent: Vec<Value> = server
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|(_, b)| b.clone())
        .collect();
    assert_eq!(sent.len(), 2, "{sent:?}");
    let (task, crash) = (&sent[0], &sent[1]);
    assert_eq!(
        (task["severity"].as_str(), task["kind"].as_str()),
        (Some("error"), Some("taskException"))
    );
    assert_eq!(
        (crash["severity"].as_str(), crash["kind"].as_str()),
        (Some("fatal"), Some("crash"))
    );
    assert!(crash["errorMessage"]
        .as_str()
        .unwrap()
        .starts_with("settings are corrupt"));
    // 300 ms of sleep, then a crash delivered without waiting out the 3 s timeout for the send in flight.
    assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
}
