use serde_json::Value;
use tauri::{command, State};

use crate::state::AptabaseState;
use crate::track_event::internal_track_event;

#[command]
pub fn track_event(
  state: State<'_, AptabaseState>,
  name: &str,
  props: Option<Value>,
) {
  internal_track_event(state, name, props)
}