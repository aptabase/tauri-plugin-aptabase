mod config;
mod client;
mod commands;
mod sys;
mod dispatcher;

use std::{sync::Arc, panic::PanicInfo, time::Duration};

use config::Config;
use serde_json::Value;
use client::AptabaseClient;
use tauri::{
  plugin::{TauriPlugin, self},
    Runtime, Manager, App, AppHandle, Window, RunEvent,
};

#[derive(Default, Debug, Clone)]
pub struct InitOptions {
  pub host: Option<String>,
  pub flush_interval: Option<Duration>,
}


/// The Aptabase Plugin builder
pub struct Builder {
  app_key: String,
  panic_hook: Option<PanicHook>,
  options: InitOptions
}

pub type PanicHook =
  Box<dyn Fn(&AptabaseClient, &PanicInfo<'_>) + 'static + Sync + Send>;

impl Builder {
    /// Creates a new builder.
    pub fn new(app_key: &str) -> Self {
      Builder {
        app_key: app_key.into(),
        panic_hook: None,
        options: Default::default()
      }
    }

    /// Sets custom topions to use for the Aptabase client.
    pub fn with_options(mut self, opts: InitOptions) -> Self {
      self.options = opts;
      self
    }

    /// Sets a custom panic hook.
    pub fn with_panic_hook(mut self, hook: PanicHook) -> Self {
      self.panic_hook = Some(hook);
      self
    }

    /// Builds and initializes the plugin
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
      plugin::Builder::new("aptabase")
        .invoke_handler(tauri::generate_handler![
          commands::track_event
        ])
        .setup(|app| {
          let cfg = Config::new(self.app_key, self.options);
          let app_version = app.package_info().version.to_string();
          let client = Arc::new(AptabaseClient::new(&cfg, app_version));

          client.start_polling(cfg.flush_interval);
          
          if let Some(hook) = self.panic_hook {
            let hook_client = client.clone();
            std::panic::set_hook(Box::new(move |info| {
              hook(&hook_client, info);
              
              hook_client.flush_blocking();
            }));
          }

          app.manage(client);
          Ok(())
        })
        .on_event(move |app, event| {
            if let RunEvent::Exit = event {
              let client = app.state::<Arc<AptabaseClient>>();
              client.flush_blocking();
            }
        })
        .build()
    }
}

/// Trait implemented by Tauri handlers
pub trait EventTracker {
  /// Enqueues an event to be sent to the server.
  fn track_event(&self, name: &str, props: Option<Value>);

  /// Flushes the event queue, blocking the current thread.
  fn flush_events_blocking(&self);
}

impl EventTracker for App {
    fn track_event(&self, name: &str, props: Option<Value>) {
      let client = self.state::<Arc<AptabaseClient>>();
      client.track_event(name, props);
    }

    fn flush_events_blocking(&self) {
      let client = self.state::<Arc<AptabaseClient>>();
      client.flush_blocking();
    }
}

impl EventTracker for AppHandle {
    fn track_event(&self, name: &str, props: Option<Value>) {
      let client = self.state::<Arc<AptabaseClient>>();
      client.track_event(name, props);
    }

    fn flush_events_blocking(&self) {
      let client = self.state::<Arc<AptabaseClient>>();
      client.flush_blocking();
    }
}

impl EventTracker for Window {
    fn track_event(&self, name: &str, props: Option<Value>) {
      let client = self.state::<Arc<AptabaseClient>>();
      client.track_event(name, props);
    }

    fn flush_events_blocking(&self) {
      let client = self.state::<Arc<AptabaseClient>>();
      client.flush_blocking();
    }
}