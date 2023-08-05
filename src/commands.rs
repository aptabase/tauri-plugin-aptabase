use std::sync::Arc;
use serde_json::Value;
use tauri::{command, State};

use crate::client::AptabaseClient;

#[command]
pub fn track_event(
  client: State<'_, Arc<AptabaseClient>>,
  name: &str,
  props: Option<Value>,
) {
  client.track_event(name, props);
}