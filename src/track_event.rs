use log::debug;
use serde_json::{Value, json};
use tauri::State;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::state::AptabaseState;

pub fn internal_track_event(
  state: State<'_, AptabaseState>,
  name: &str,
  props: Option<Value>,
) {
  if state.config.app_key.is_empty() {
    return ;
  }
  let body = json!({
      "timestamp": OffsetDateTime::now_utc().format(&Rfc3339).unwrap(),
      "sessionId": state.eval_session_id(),
      "eventName": name,
      "systemProps": {
          "isDebug": state.sys_info.is_debug,
          "osName": state.sys_info.os_name,
          "osVersion": state.sys_info.os_version,
          "locale": state.sys_info.locale,
          "engineName": state.sys_info.engine_name,
          "engineVersion": state.sys_info.engine_version,
          "appVersion": state.app_version,
          "sdkVersion": concat!(env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION"))
      },
      "props": props
  });

  let url = state.config.ingest_api_url.clone();
  let http_client = state.http_client.clone();

  tauri::async_runtime::spawn(async move {
    let response = http_client.post(url).json(&body).send().await;
    match response {
        Ok(response) => {
            if !response.status().is_success() {
              debug!("failed to track_event with status code {}", response.status());
            }
        }
        Err(err) => {
          debug!("failed to track_event: {}", err.to_string());
        }
    }
  });
}