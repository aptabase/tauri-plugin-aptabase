use rand::RngExt;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    sync::{mpsc, Arc, Mutex as SyncMutex},
    time::Duration,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    config::Config,
    dispatcher::{self, EventDispatcher},
    errors::{self, ErrorDispatcher, ErrorKind, ErrorReport},
    sys::{self, SystemProperties},
};

static SESSION_TIMEOUT: Duration = Duration::from_secs(4 * 60 * 60);

fn new_session_id() -> String {
    let epoch_in_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs();

    let mut rng = rand::rng();
    let random: u64 = rng.random_range(0..=99999999);

    let id = epoch_in_seconds * 100_000_000 + random;

    id.to_string()
}

/// A tracking session.
#[derive(Debug, Clone)]
pub struct TrackingSession {
    pub id: String,
    pub last_touch_ts: OffsetDateTime,
}

impl TrackingSession {
    fn new() -> Self {
        Self {
            id: new_session_id(),
            last_touch_ts: OffsetDateTime::now_utc(),
        }
    }
}

/// The Aptabase client used to track events.
pub struct AptabaseClient {
    is_enabled: bool,
    session: SyncMutex<TrackingSession>,
    dispatcher: Arc<EventDispatcher>,
    errors: Arc<ErrorDispatcher>,
    app_version: String,
    sys_info: SystemProperties,
}

impl AptabaseClient {
    /// Creates a new Aptabase client.
    pub fn new(config: &Config, app_version: String) -> Self {
        let sys_info = sys::get_info();

        let is_enabled = !config.app_key.is_empty();
        let build = |b: reqwest::ClientBuilder| b.build().expect("could not build http client");
        let http_client = build(dispatcher::http_client(config, &sys_info));
        let crash_client =
            build(dispatcher::http_client(config, &sys_info).pool_max_idle_per_host(0));
        let errors = Arc::new(ErrorDispatcher::new(
            config.error_api_url.clone(),
            http_client.clone(),
            crash_client,
        ));
        let dispatcher = Arc::new(EventDispatcher::new(config, http_client));

        Self {
            is_enabled,
            dispatcher,
            errors,
            session: SyncMutex::new(TrackingSession::new()),
            app_version,
            sys_info,
        }
    }

    /// Starts the event dispatcher loop.
    pub(crate) fn start_polling(&self, interval: Duration) {
        let dispatcher = self.dispatcher.clone();
        let errors = self.errors.clone();

        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                dispatcher.flush().await;
                errors.flush().await;
            }
        });
    }

    /// Returns the current session ID, creating a new one if necessary.
    pub(crate) fn eval_session_id(&self) -> String {
        // Also called from panic hooks, where a lock poisoned by another panic must not panic again.
        let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());

        let now = OffsetDateTime::now_utc();
        if (now - session.last_touch_ts) > SESSION_TIMEOUT {
            *session = TrackingSession::new();
        } else {
            session.last_touch_ts = now;
        }

        session.id.clone()
    }

    /// Enqueues an event to be sent to the server.
    pub fn track_event(&self, name: &str, props: Option<Value>) -> Result<(), String> {
        if !self.is_enabled {
            return Ok(());
        }

        if let Some(props) = &props {
            if !matches!(props, Value::Object(_)) {
                return Err(
                    "props must be `None` or the `Object` variation of `serde_json::Value`"
                        .to_owned(),
                );
            }
        }

        let ev = json!({
            "timestamp": OffsetDateTime::now_utc().format(&Rfc3339).unwrap(),
            "sessionId": self.eval_session_id(),
            "eventName": name,
            "systemProps": {
                "isDebug": self.sys_info.is_debug,
                "osName": self.sys_info.os_name,
                "osVersion": self.sys_info.os_version,
                "locale": self.sys_info.locale,
                "engineName": self.sys_info.engine_name,
                "engineVersion": self.sys_info.engine_version,
                "appVersion": self.app_version,
                "sdkVersion": concat!(env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION"))
            },
            "props": props
        });

        self.dispatcher.enqueue(ev);

        Ok(())
    }

    /// Reports an error to Aptabase. It is sent right away, in the background.
    pub fn track_error(&self, report: ErrorReport) {
        let kind = if report.fatal {
            ErrorKind::Crash
        } else {
            ErrorKind::Handled
        };
        self.track_error_as(report, kind);
    }

    /// Reports an error captured as `kind`.
    pub(crate) fn track_error_as(&self, report: ErrorReport, kind: ErrorKind) {
        if !self.is_enabled {
            return;
        }

        // Errors are worth more than events: they are sent now rather than at the next flush.
        if self.errors.enqueue(self.error_body(&report, kind)) {
            let errors = self.errors.clone();
            tauri::async_runtime::spawn(async move { errors.flush().await });
        }
    }

    /// Reports a crash, straight away and not through the queue, then sends the reports queued before it, waiting at
    /// most `timeout`: what the app allows before it exits.
    pub(crate) fn report_crash(&self, report: ErrorReport, timeout: Duration) {
        if !self.is_enabled {
            return;
        }

        let body = self.error_body(&report, ErrorKind::Crash);
        errors::send_crash_blocking(&self.errors, body, timeout);
    }

    /// Flushes the events and the error reports, waiting at most `timeout`, from a thread and a runtime of their own:
    /// what a panic hook can afford.
    pub(crate) fn flush_within(&self, timeout: Duration) {
        let dispatcher = self.dispatcher.clone();
        let errors = self.errors.clone();
        errors::block_on_own_thread(
            async move {
                dispatcher.flush().await;
                errors.flush().await;
            },
            timeout,
        );
    }

    fn error_body(&self, report: &ErrorReport, kind: ErrorKind) -> Value {
        let timestamp = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
        errors::report_body(
            report,
            kind,
            &self.eval_session_id(),
            &timestamp,
            &self.sys_info,
            &self.app_version,
        )
    }

    /// Flushes the event queue and the error reports waiting to be sent again.
    pub async fn flush(&self) {
        self.dispatcher.flush().await;
        self.errors.flush().await;
    }

    /// Flushes the event queue, blocking the current thread.
    ///
    /// A request needs an async runtime's reactor. A thread outside the runtime (the main thread handling
    /// `RunEvent::Exit`, a panic hook there) has none, so the flush runs on Tauri's runtime while this thread waits;
    /// a thread inside a runtime flushes in place, as it always has.
    pub fn flush_blocking(&self) {
        if tokio::runtime::Handle::try_current().is_ok() {
            futures::executor::block_on(self.flush());
            return;
        }
        let dispatcher = self.dispatcher.clone();
        let errors = self.errors.clone();
        let (done, wait) = mpsc::channel();
        tauri::async_runtime::spawn(async move {
            dispatcher.flush().await;
            errors.flush().await;
            let _ = done.send(());
        });
        let _ = wait.recv();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_server::server;

    fn client(base: &str) -> AptabaseClient {
        let config = Config {
            app_key: "A-DEV-0000000000".into(),
            ingest_api_url: format!("{base}/api/v0/events").parse().unwrap(),
            error_api_url: format!("{base}/api/v0/error").parse().unwrap(),
            flush_interval: Duration::from_secs(60),
        };
        AptabaseClient::new(&config, "1.4.0".into())
    }

    // The main thread handling RunEvent::Exit is outside the async runtime: flushing there used to panic with
    // "there is no reactor running".
    #[test]
    fn a_blocking_flush_sends_the_events_from_a_thread_outside_the_runtime() {
        let server = server(&[200]);
        let client = Arc::new(client(&server.base));
        client.track_event("app_exited", None).unwrap();
        let flushing = client.clone();
        std::thread::spawn(move || flushing.flush_blocking())
            .join()
            .expect("the flush doesn't panic");
        let sent = server.requests.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1[0]["eventName"], "app_exited");
    }

    #[test]
    fn an_error_is_sent_right_away_stamped_with_the_session() {
        let server = server(&[202]);
        let client = client(&server.base);
        let session = client.eval_session_id();
        client.track_error(ErrorReport::new(
            "std::io::error::Error",
            "settings.json is missing",
        ));
        let started = std::time::Instant::now();
        while server.requests.lock().unwrap().is_empty()
            && started.elapsed() < Duration::from_secs(5)
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        let sent = server.requests.lock().unwrap();
        assert_eq!(sent.len(), 1, "sent without waiting for the next flush");
        let report = &sent[0].1;
        assert_eq!(report["errorType"], "std::io::error::Error");
        assert_eq!(report["kind"], "handled");
        assert_eq!(report["sessionId"], session.as_str());
        assert_eq!(report["appVersion"], "1.4.0");
    }
}
