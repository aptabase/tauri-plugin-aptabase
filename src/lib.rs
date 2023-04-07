mod config;
mod state;
mod commands;
mod sys;
mod track_event;

use config::Config;
use serde_json::Value;
use state::AptabaseState;
use tauri::{
  plugin::{Builder, TauriPlugin},
    Runtime, Manager, App, AppHandle, Window, 
};
use track_event::internal_track_event;

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

pub trait EventTracker {
  fn track_event(&self, name: &str, props: Option<Value>);
}

impl EventTracker for App {
    fn track_event(&self, name: &str, props: Option<Value>) {
        internal_track_event(self.state(), name, props)
    }
}

impl EventTracker for AppHandle {
    fn track_event(&self, name: &str, props: Option<Value>) {
        internal_track_event(self.state(), name, props)
    }
}

impl EventTracker for Window {
    fn track_event(&self, name: &str, props: Option<Value>) {
        internal_track_event(self.state(), name, props)
    }
}