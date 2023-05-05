#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use tauri_plugin_log::LogTarget;
use tauri_plugin_aptabase::EventTracker;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_aptabase::Builder::new("A-DEV-0000000000").build())
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
