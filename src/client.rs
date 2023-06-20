use log::debug;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{json, Value};
use std::{sync::Mutex, time::Duration, time::Instant};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::{
    config::Config,
    sys::{self, SystemProperties},
};

static SESSION_TIMEOUT: Duration = Duration::from_secs(4 * 60 * 60);
static HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct TrackingSession {
    pub id: String,
    pub last_touch_ts: Instant,
}

impl TrackingSession {
    fn new() -> Self {
        TrackingSession {
            id: Uuid::new_v4().to_string(),
            last_touch_ts: Instant::now(),
        }
    }
}

pub struct AptabaseClient {
    session: Mutex<TrackingSession>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) config: Config,
    pub app_version: String,
    pub sys_info: SystemProperties,
}

impl AptabaseClient {
    pub fn with_config(config: Config, app_version: String) -> Self {
        let mut headers = HeaderMap::new();
        let app_key_header = HeaderValue::from_str(config.app_key.as_str())
            .expect("failed to define App Key header value");
        headers.insert("App-Key", app_key_header);
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let user_agent = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
        let http_client = reqwest::Client::builder()
            .timeout(HTTP_REQUEST_TIMEOUT)
            .default_headers(headers)
            .user_agent(user_agent)
            .build()
            .expect("could not build http client");

        let sys_info = sys::get_info();

        AptabaseClient {
            config,
            http_client,
            session: Mutex::new(TrackingSession::new()),
            app_version,
            sys_info,
        }
    }

    pub(crate) fn eval_session_id(&self) -> String {
        let mut session = self.session.lock().expect("could not lock session");

        // session timeout since last touched, start a new one!
        if session.last_touch_ts.elapsed() > SESSION_TIMEOUT {
            *session = TrackingSession::new()
        } else {
            session.last_touch_ts = Instant::now()
        }
        return session.id.clone();
    }

    pub fn track_event(&self, name: &str, props: Option<Value>) {
        if self.config.app_key.is_empty() {
            return;
        }
        let body = json!({
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

        let url = self.config.ingest_api_url.clone();
        let http_client = self.http_client.clone();

        tauri::async_runtime::spawn(async move {
            let response = http_client.post(url).json(&body).send().await;
            match response {
                Ok(response) => {
                    if !response.status().is_success() {
                        debug!(
                            "failed to track_event with status code {}",
                            response.status()
                        );
                    }
                }
                Err(err) => {
                    debug!("failed to track_event: {}", err.to_string());
                }
            }
        });
    }
}
