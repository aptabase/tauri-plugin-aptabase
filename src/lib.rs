mod config;
mod client;
mod commands;
mod sys;

use std::{sync::Arc, panic::PanicInfo, time::Duration, thread};

use config::Config;
use serde_json::Value;
use client::AptabaseClient;
use tauri::{
  plugin::{TauriPlugin, self},
    Runtime, Manager, App, AppHandle, Window, 
};

#[derive(Default, Debug, Clone)]
pub struct InitOptions {
  pub host: Option<String>
}


pub struct Builder {
  app_key: String,
  panic_hook: Option<PanicHook>,
  options: InitOptions
}

pub type PanicHook =
  Box<dyn Fn(&AptabaseClient, &PanicInfo<'_>) + 'static + Sync + Send>;

impl Builder {
    pub fn new(app_key: &str) -> Self {
      Builder {
        app_key: app_key.into(),
        panic_hook: None,
        options: Default::default()
      }
    }

    pub fn with_options(mut self, opts: InitOptions) -> Self {
      self.options = opts;
      self
    }

    pub fn with_panic_hook(mut self, hook: PanicHook) -> Self {
      self.panic_hook = Some(hook);
      self
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
      plugin::Builder::new("aptabase")
        .invoke_handler(tauri::generate_handler![commands::track_event])
        .setup(|app| {
          let cfg = Config::new(self.app_key, self.options.host);
          let app_version = app.package_info().version.to_string();
          let client = Arc::new(AptabaseClient::with_config(cfg, app_version));
          
          if let Some(hook) = self.panic_hook {
            let hook_client = client.clone();
            std::panic::set_hook(Box::new(move |info| {
              hook(&hook_client, info);

              // Wait 2sec to give time for the thread to send the event
              // This can be removed when we move to Background Queue + Flush
              thread::sleep(Duration::from_millis(2000));
            }));
          }

          app.manage(client);
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
      let client = self.state::<Arc<AptabaseClient>>();
      client.track_event(name, props);
    }
}

impl EventTracker for AppHandle {
    fn track_event(&self, name: &str, props: Option<Value>) {
      let client = self.state::<Arc<AptabaseClient>>();
      client.track_event(name, props);
    }
}

impl EventTracker for Window {
    fn track_event(&self, name: &str, props: Option<Value>) {
      let client = self.state::<Arc<AptabaseClient>>();
      client.track_event(name, props);
    }
}