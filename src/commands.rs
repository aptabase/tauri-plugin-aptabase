use serde_json::json;
use tauri::{
  command,
  AppHandle,  Runtime, Window, State,
};
use time::{
    OffsetDateTime, format_description::well_known::Rfc3339,
};

use crate::state::AptabaseState;
use crate::error::Error;

type Result<T> = std::result::Result<T, Error>;

#[command]
pub async fn track_event<R: Runtime>(
  name: &str,
  _app: AppHandle<R>,
  _window: Window<R>,
  state: State<'_, AptabaseState>,
) -> Result<()> {
  let event = json!({
      "timestamp": OffsetDateTime::now_utc().format(&Rfc3339).unwrap(),
      "session_id": state.eval_session_id(),
      "event_name": name,
      "system_props": {
          "os_family": state.device_info.os_family,
          "os_name": state.device_info.os_family,
          "os_version": state.device_info.os_version,
          "os_locale": state.device_info.os_locale,
          "app_version": state.app_version,
          "sdk_version": format!("{}@{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
      },
  });

  let body = json!({"events": vec![event]});
  let url = state.config.ingest_api_url.clone();

  state.http_client.post(url).json(&body).send().await.ok();

  Ok(())
}