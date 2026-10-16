//! The webview's side of error reporting: `trackError` and the crash reporting script call the plugin's
//! `track_error` command with these arguments, as JavaScript names them.

#[path = "../src/test_server.rs"]
mod test_server;

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
    webview::InvokeRequest,
    WebviewWindowBuilder,
};
use tauri_plugin_aptabase::{Builder, InitOptions};

fn invoke(
    webview: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    args: Value,
) -> Result<(), Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: "plugin:aptabase|track_error".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|_| ())
}

#[test]
fn the_webview_reports_errors_through_track_error() {
    let server = test_server::server(&[202, 202]);
    // An app's build script tells it the plugins' permissions, as `aptabase:allow-track-error` grants this command;
    // a mock app has no build script, so the command is allowed directly.
    let mut context = mock_context(noop_assets());
    context.runtime_authority_mut().__allow_command(
        "plugin:aptabase|track_error".into(),
        tauri::utils::acl::ExecutionContext::Local,
    );
    let app = mock_builder()
        .plugin(
            Builder::new("A-SH-0000000000")
                .with_options(InitOptions {
                    host: Some(server.base.clone()),
                    ..Default::default()
                })
                .build(),
        )
        .build(context)
        .unwrap();
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    // trackError(error, { fatal: true })
    invoke(
        &webview,
        json!({ "errorType": "TypeError", "message": "TypeError: x is undefined", "stackTrace": "at save (app.js:3:9)", "fatal": true }),
    )
    .unwrap();
    // The crash reporting script, for an unhandled rejection without a stack.
    invoke(
        &webview,
        json!({ "errorType": "Error", "message": "Error: offline", "stackTrace": null, "kind": "taskException" }),
    )
    .unwrap();

    let started = Instant::now();
    while server.requests.lock().unwrap().len() < 2 && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut sent: Vec<Value> = server
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|(_, body)| body.clone())
        .collect();
    sent.sort_by_key(|r| r["errorType"].as_str().unwrap().to_string());
    assert_eq!(sent.len(), 2);
    let (rejection, fatal) = (&sent[0], &sent[1]);
    assert_eq!(fatal["errorType"], "TypeError");
    assert_eq!(fatal["stackTrace"], "at save (app.js:3:9)");
    assert_eq!(
        (fatal["severity"].as_str(), fatal["kind"].as_str()),
        (Some("fatal"), Some("crash"))
    );
    assert_eq!(rejection["errorMessage"], "Error: offline");
    assert_eq!(rejection["stackTrace"], Value::Null);
    assert_eq!(
        (rejection["severity"].as_str(), rejection["kind"].as_str()),
        (Some("error"), Some("taskException"))
    );
}
