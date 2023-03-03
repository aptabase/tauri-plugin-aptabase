
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
use crate::device;

type Result<T> = std::result::Result<T, Error>;

#[command]
pub async fn track_event<R: Runtime>(
  name: &str,
  app: AppHandle<R>,
  _window: Window<R>,
  state: State<'_, AptabaseState>,
) -> Result<()> {
  let event_ts = OffsetDateTime::now_utc().format(&Rfc3339).unwrap();
  let device_info = device::info();
  let app_version = &app.package_info().version;
  let event = json!({
      "timestamp": event_ts,
      "session_id": "abc",
      "event_name": name,
      "system_props": {
          "os_family": device_info.os_family,
          "os_name": device_info.os_family,
          "os_version": device_info.os_version,
          "os_locale": device_info.os_locale,
          "app_version": app_version.to_string(),
          "app_build_number": "",
          "sdk_version": format!("{}@{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
      },
  });
  let body = json!({"events": vec![event]});

  state.http_client.post(state.config.ingest_api_url.clone())
    .json(&body)
    .header("App-Key", state.config.app_key.clone())
    .header("Content-Type", "application/json")
    .send()
    .await
    .ok();

  Ok(())
}