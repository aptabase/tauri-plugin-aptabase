use serde_json::{json, Value};
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
  props: Option<Value>,
  _app: AppHandle<R>,
  _window: Window<R>,
  state: State<'_, AptabaseState>,
) -> Result<()> {

  let session = state.eval_session();
  let body = json!({
      "timestamp": OffsetDateTime::now_utc().format(&Rfc3339).unwrap(),
      "userId": state.user_id,
      "session": {
        "id": session.id,
        "startedAt": session.started_at.format(&Rfc3339).unwrap(),
      },
      "eventName": name,
      "systemProps": {
          "osName": state.sys_info.os_name,
          "osVersion": state.sys_info.os_version,
          "locale": state.sys_info.locale,
          "frameworkName": state.sys_info.framework_name,
          "frameworkVersion": state.sys_info.framework_version,
          "engineName": state.sys_info.engine_name,
          "engineVersion": state.sys_info.engine_version,
          "appVersion": state.app_version,
          "sdkVersion": concat!(env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION"))
      },
      "props": props
  });

  let url = state.config.ingest_api_url.clone();

  state.http_client.post(url).json(&body).send().await.ok();
  Ok(())
}