//! Crash reporting end to end: the plugin, built into a (mock) Tauri app, installs its panic hook, and each panic
//! reaches the error endpoint reported as what it ended.

#[path = "../src/test_server.rs"]
mod test_server;

use std::time::{Duration, Instant};

use serde_json::Value;
use tauri_plugin_aptabase::{Builder, InitOptions};

fn wait_for(server: &test_server::Server, count: usize) -> Vec<Value> {
    let started = Instant::now();
    while server.requests.lock().unwrap().len() < count
        && started.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    server
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|(_, body)| body.clone())
        .collect()
}

#[test]
fn panics_are_reported_as_what_they_ended() {
    let server = test_server::server(&[202, 202]);
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

    // The main thread's panic ends the app (Rust names that thread "main"; one of the same name stands in for it).
    // The hook waits for the report before it returns, so the report has arrived by the time the thread is joined.
    let crashed = std::thread::Builder::new()
        .name("main".into())
        .spawn(|| panic!("settings are corrupt"))
        .unwrap()
        .join();
    assert!(crashed.is_err());
    let sent: Vec<Value> = server
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|(_, body)| body.clone())
        .collect();
    assert_eq!(sent.len(), 1, "delivered before the hook returned");
    let crash = &sent[0];
    assert_eq!(crash["errorType"], "panic");
    assert_eq!(
        (crash["severity"].as_str(), crash["kind"].as_str()),
        (Some("fatal"), Some("crash"))
    );
    let message = crash["errorMessage"].as_str().unwrap();
    assert!(
        message.starts_with("settings are corrupt (at tests/crash_reporting.rs:"),
        "{message}"
    );
    let stack = crash["stackTrace"].as_str().unwrap();
    assert!(
        stack.starts_with("thread 'main' panicked at tests/crash_reporting.rs:"),
        "{stack}"
    );
    assert_eq!(crash["platform"], "Tauri");

    // A panic in a task on Tauri's async runtime ends only that task.
    let task = tauri::async_runtime::spawn(async { panic!("sync failed") });
    assert!(tauri::async_runtime::block_on(task).is_err());
    let sent = wait_for(&server, 2);
    assert_eq!(sent.len(), 2);
    let task = &sent[1];
    assert!(task["errorMessage"]
        .as_str()
        .unwrap()
        .starts_with("sync failed (at "));
    assert_eq!(
        (task["severity"].as_str(), task["kind"].as_str()),
        (Some("error"), Some("taskException"))
    );
}
