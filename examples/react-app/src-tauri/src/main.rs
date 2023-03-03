#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_aptabase::init("A-EU-14982741824".into()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
