#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri_plugin_log::LogTarget;
use tauri_plugin_aptabase::EventTracker;
use serde_json::json;

#[tauri::command]
fn this_will_panic() {
  panic!("I told you so!");
}

fn main() {
    tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![this_will_panic])
        .plugin(tauri_plugin_aptabase::Builder::new("A-DEV-0000000000").with_panic_hook(Box::new(|client, info| {
            client.track_event("panic", Some(json!({
                "info": format!("{:?}", info),
            })));
        })).build())
        .plugin(tauri_plugin_log::Builder::default().targets([
            LogTarget::LogDir,
            LogTarget::Stdout,
            LogTarget::Webview,
        ]).build())
        .setup(|app| {
            app.track_event("app_started", None);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
