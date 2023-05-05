mod config;
mod state;
mod commands;
mod sys;
mod track_event;

use config::Config;
use serde_json::Value;
use state::AptabaseState;
use tauri::{
  plugin::{TauriPlugin, self},
    Runtime, Manager, App, AppHandle, Window, 
};
use track_event::internal_track_event;

#[derive(Default, Debug, Clone)]
pub struct InitOptions {
  pub host: Option<String>
}


pub struct Builder {
  app_key: String,
  options: InitOptions
}

impl Builder {
    pub fn new(app_key: &str) -> Self {
      Builder {
        app_key: app_key.into(),
        options: Default::default()
      }
    }

    pub fn with_options(mut self, opts: InitOptions) -> Self {
      self.options = opts;
      self
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
      plugin::Builder::new("aptabase")
        .invoke_handler(tauri::generate_handler![commands::track_event])
        .setup(|app| {
          let cfg = Config::new(self.app_key, self.options.host);
          let app_version = app.package_info().version.to_string();
          let state = AptabaseState::with_config(cfg, app_version);
          app.manage(state);
          Ok(())
        })
        .build()
    }
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