use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::Duration;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::{
    config::Config,
    dispatcher::EventDispatcher,
    sys::{self, SystemProperties},
};

/// Computes a deterministic session ID from machine ID, app key, and date string.
pub(crate) fn build_session_id(machine_id: &str, app_key: &str, date: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(machine_id.as_bytes());
    hasher.update(app_key.as_bytes());
    hasher.update(date.as_bytes());

    let hash = format!("{:x}", hasher.finalize());
    // Aptabase server validates SessionId <= 36 chars, SHA-256 hex is 64
    hash[..36].to_string()
}

/// Creates a deterministic session ID from machine ID, app key, and current UTC date.
/// Same device + same app + same calendar day = same session ID.
pub(crate) fn create_session_id(app_key: &str) -> String {
    let machine_id = machine_uid::get().unwrap_or_else(|_| "unknown".to_string());
    let today = OffsetDateTime::now_utc().date().to_string(); // e.g. "2026-02-13"
    build_session_id(&machine_id, app_key, &today)
}

/// A tracking session.
#[derive(Debug, Clone)]
pub struct TrackingSession {
    pub id: String,
    pub date: String,
}

impl TrackingSession {
    fn new(app_key: &str) -> Self {
        Self {
            id: create_session_id(app_key),
            date: OffsetDateTime::now_utc().date().to_string(),
        }
    }
}

/// The Aptabase client used to track events.
pub struct AptabaseClient {
    is_enabled: bool,
    app_key: String,
    session: SyncMutex<TrackingSession>,
    dispatcher: Arc<EventDispatcher>,
    app_version: String,
    sys_info: SystemProperties,
}

impl AptabaseClient {
    /// Creates a new Aptabase client.
    pub fn new(config: &Config, app_version: String) -> Self {
        let sys_info = sys::get_info();

        let is_enabled = !config.app_key.is_empty();
        let dispatcher = Arc::new(EventDispatcher::new(config, &sys_info));

        Self {
            is_enabled,
            app_key: config.app_key.clone(),
            dispatcher,
            session: SyncMutex::new(TrackingSession::new(&config.app_key)),
            app_version,
            sys_info,
        }
    }

    /// Starts the event dispatcher loop.
    pub(crate) fn start_polling(&self, interval: Duration) {
        let dispatcher = self.dispatcher.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                dispatcher.flush().await;
            }
        });
    }

    /// Returns the current session ID, rotating to a new one if the UTC date has changed.
    pub(crate) fn eval_session_id(&self) -> String {
        let mut session = self.session.lock().expect("could not lock events");

        let today = OffsetDateTime::now_utc().date().to_string();
        if session.date != today {
            *session = TrackingSession::new(&self.app_key);
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

    /// Flushes the event queue.
    pub async fn flush(&self) {
        self.dispatcher.flush().await;
    }

    /// Flushes the event queue, blocking the current thread.
    pub fn flush_blocking(&self) {
        futures::executor::block_on(async {
            self.flush().await;
        });
    }
}

#[cfg(test)]
mod tests;
