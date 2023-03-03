mod error;
mod config;
mod state;
mod commands;
mod device;

use config::Config;

use state::AptabaseState;
use tauri::{
  plugin::{Builder, TauriPlugin},
    Runtime, Manager, 
};


/// Initializes the plugin.
pub fn init<R: Runtime>(app_key: String) -> TauriPlugin<R> {
  let cfg = Config::with_app_key(app_key);
  let state = AptabaseState::with_config(cfg);

  Builder::new("aptabase")
    .invoke_handler(tauri::generate_handler![commands::track_event])
    .setup(|app| {
      app.manage(state);
      Ok(())
    })
    .build()
}
