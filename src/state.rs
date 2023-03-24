use std::{sync::Mutex, time::Instant, time::Duration};
use reqwest::header::{HeaderMap, HeaderValue};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{config::Config, sys::{SystemProperties, self}};

static SESSION_TIMEOUT: Duration = Duration::from_secs(4 * 60 * 60);
static HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct TrackingSession {
    pub id: String,
    pub started_at: OffsetDateTime,
    pub last_touch_ts: Instant
}

impl TrackingSession {
    fn new() -> Self { 
        TrackingSession {
            id: Uuid::new_v4().to_string(),
            started_at: OffsetDateTime::now_utc(),
            last_touch_ts: Instant::now(),
        }
     }
}

pub struct AptabaseState {
    session: Mutex<TrackingSession>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) config: Config,
    pub app_version: String,
    pub user_id: String,
    pub sys_info: SystemProperties
}


impl AptabaseState {
    pub fn with_config(config: Config, app_version: String) -> Self {
        let mut headers = HeaderMap::new();
        let app_key_header = HeaderValue::from_str(config.app_key.as_str()).expect("failed to define App Key header value");
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
        let user_id = compute_user_id(&sys_info.identifier, &config.app_key);

        AptabaseState {
            config,
            http_client,
            session: Mutex::new(TrackingSession::new()),
            app_version,
            user_id,
            sys_info,
        }
    }

    pub(crate) fn eval_session(&self) -> TrackingSession {
        let mut session = self.session.lock().expect("could not lock session");

        // session timeout since last touched, start a new one!
        if session.last_touch_ts.elapsed() > SESSION_TIMEOUT {
            *session = TrackingSession::new()
        } else {
            session.last_touch_ts = Instant::now()
        }
        return session.clone();
    }
}

fn compute_user_id(device_id: &str, app_key: &str) -> String {
    let hash = Sha256::new()
        .chain_update(device_id.trim().to_lowercase())
        .chain_update(app_key.trim().to_lowercase())
        .finalize();
    
    return format!("{:x}", hash);
}