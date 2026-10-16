mod client;
mod commands;
mod config;
mod dispatcher;
mod errors;
mod sys;
#[cfg(test)]
mod test_server;

use std::{
    panic::PanicHookInfo,
    sync::Arc,
    time::{Duration, Instant},
};

use client::AptabaseClient;
use config::Config;
use errors::ErrorKind;
pub use errors::ErrorReport;
use serde_json::Value;
use tauri::{
    plugin::{self, TauriPlugin},
    App, AppHandle, Manager, RunEvent, Runtime, Window,
};

#[derive(Default, Debug, Clone)]
pub struct InitOptions {
    pub host: Option<String>,
    pub flush_interval: Option<Duration>,
}

/// How long a crash waits for its report to be sent before the app exits.
const CRASH_FLUSH_TIMEOUT: Duration = Duration::from_secs(3);

/// Reports the webview's uncaught errors and unhandled promise rejections, with crash reporting on.
const CRASH_REPORTING_SCRIPT: &str = include_str!("crash_reporting.js");

/// The Aptabase Plugin builder
pub struct Builder {
    app_key: String,
    panic_hook: Option<PanicHook>,
    crash_reporting: bool,
    options: InitOptions,
}

pub type PanicHook =
    Box<dyn Fn(&AptabaseClient, &PanicHookInfo<'_>, String) + 'static + Sync + Send>;

fn get_panic_message(info: &PanicHookInfo) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        return s.to_string();
    } else if let Some(s) = payload.downcast_ref::<String>() {
        return s.to_string();
    }

    format!("{:?}", payload)
}

impl Builder {
    /// Creates a new builder.
    pub fn new(app_key: &str) -> Self {
        Self {
            app_key: app_key.into(),
            panic_hook: None,
            crash_reporting: false,
            options: Default::default(),
        }
    }

    /// Sets custom options to use for the Aptabase client.
    pub fn with_options(mut self, opts: InitOptions) -> Self {
        self.options = opts;
        self
    }

    /// Sets a custom panic hook.
    pub fn with_panic_hook(mut self, hook: PanicHook) -> Self {
        self.panic_hook = Some(hook);
        self
    }

    /// Reports crashes and uncaught errors to Aptabase, on the Errors page of the dashboard.
    ///
    /// - A panic on the main thread ends the app: it is reported as a fatal crash, and the app waits up to three
    ///   seconds for the report to be sent before it exits.
    /// - A panic in a task on an async runtime (an async command, `tauri::async_runtime::spawn`) ends only that task,
    ///   and one on a thread of your own ends only that thread: both are reported as errors.
    /// - The webview's uncaught errors and unhandled promise rejections are reported as errors. This needs the
    ///   `aptabase:allow-track-error` permission in the window's capabilities.
    ///
    /// A hook set with [`Builder::with_panic_hook`] still runs, before the panic is reported.
    pub fn with_crash_reporting(mut self) -> Self {
        self.crash_reporting = true;
        self
    }

    /// Builds and initializes the plugin
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        let crash_reporting = self.crash_reporting;
        let mut builder =
            plugin::Builder::new("aptabase").invoke_handler(tauri::generate_handler![
                commands::track_event,
                commands::track_error
            ]);
        if crash_reporting {
            builder = builder.js_init_script(CRASH_REPORTING_SCRIPT.to_string());
        }
        builder
            .setup(move |app, _api| {
                let cfg = Config::new(self.app_key, self.options);
                let app_version = app.package_info().version.to_string();
                let client = Arc::new(AptabaseClient::new(&cfg, app_version));

                client.start_polling(cfg.flush_interval);

                if self.panic_hook.is_some() || crash_reporting {
                    let custom_hook = self.panic_hook;
                    let default_panic = std::panic::take_hook();
                    let hook_client = client.clone();
                    std::panic::set_hook(Box::new(move |info| {
                        let msg = get_panic_message(info);
                        if let Some(hook) = &custom_hook {
                            hook(&hook_client, info, msg.clone());
                        }
                        // One budget for everything the hook sends, so the app never waits longer than that in all.
                        let deadline = Instant::now() + CRASH_FLUSH_TIMEOUT;
                        if crash_reporting {
                            let (report, kind) = errors::panic_report(info, &msg);
                            if kind == ErrorKind::Crash {
                                hook_client.report_crash(report, CRASH_FLUSH_TIMEOUT);
                            } else {
                                hook_client.track_error_as(report, kind);
                            }
                        }
                        // What the custom hook tracked, after the crash's report so that nothing holds it up.
                        let left = deadline.saturating_duration_since(Instant::now());
                        if custom_hook.is_some() && !left.is_zero() {
                            hook_client.flush_within(left);
                        }
                        default_panic(info);
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
    fn track_event(&self, name: &str, props: Option<Value>) -> Result<(), String>;

    /// Flushes the event queue, blocking the current thread.
    fn flush_events_blocking(&self);
}

/// Trait implemented by Tauri handlers to report errors
pub trait ErrorTracker {
    /// Reports an error your code handled: its type name, its message followed by the errors that caused it, and a
    /// backtrace of where it was reported when `RUST_BACKTRACE` enables backtraces. It is sent right away, in the
    /// background.
    fn track_error<E: std::error::Error + ?Sized>(&self, error: &E) {
        self.track_error_report(ErrorReport::from_error(error));
    }

    /// Reports an error described field by field; [`ErrorReport::with_fatal`] marks one the app can't recover from.
    fn track_error_report(&self, report: ErrorReport);
}

impl EventTracker for App {
    fn track_event(&self, name: &str, props: Option<Value>) -> Result<(), String> {
        let client = self.state::<Arc<AptabaseClient>>();
        client.track_event(name, props)
    }

    fn flush_events_blocking(&self) {
        let client = self.state::<Arc<AptabaseClient>>();
        client.flush_blocking();
    }
}

impl EventTracker for AppHandle {
    fn track_event(&self, name: &str, props: Option<Value>) -> Result<(), String> {
        let client = self.state::<Arc<AptabaseClient>>();
        client.track_event(name, props)
    }

    fn flush_events_blocking(&self) {
        let client = self.state::<Arc<AptabaseClient>>();
        client.flush_blocking();
    }
}

impl EventTracker for Window {
    fn track_event(&self, name: &str, props: Option<Value>) -> Result<(), String> {
        let client = self.state::<Arc<AptabaseClient>>();
        client.track_event(name, props)
    }

    fn flush_events_blocking(&self) {
        let client = self.state::<Arc<AptabaseClient>>();
        client.flush_blocking();
    }
}

impl ErrorTracker for App {
    fn track_error_report(&self, report: ErrorReport) {
        let client = self.state::<Arc<AptabaseClient>>();
        client.track_error(report);
    }
}

impl ErrorTracker for AppHandle {
    fn track_error_report(&self, report: ErrorReport) {
        let client = self.state::<Arc<AptabaseClient>>();
        client.track_error(report);
    }
}

impl ErrorTracker for Window {
    fn track_error_report(&self, report: ErrorReport) {
        let client = self.state::<Arc<AptabaseClient>>();
        client.track_error(report);
    }
}
