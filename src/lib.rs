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

  Builder::new("aptabase")
    .invoke_handler(tauri::generate_handler![commands::track_event])
    .setup(|app| {
      let cfg = Config::with_app_key(app_key);
      let app_version = app.package_info().version.to_string();
      let state = AptabaseState::with_config(cfg, app_version);
      app.manage(state);
      Ok(())
    })
    .build()
}
