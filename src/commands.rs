use serde_json::Value;
use std::sync::Arc;
use tauri::{command, State};

use crate::{
    client::AptabaseClient,
    errors::{ErrorKind, ErrorReport},
};

#[command]
pub fn track_event(
    client: State<'_, Arc<AptabaseClient>>,
    name: &str,
    props: Option<Value>,
) -> Result<(), String> {
    client.track_event(name, props)
}

/// An error from the webview: one `trackError` reports (handled, or fatal), or one crash reporting caught
/// (`kind` "unhandled" for an uncaught error, "taskException" for an unhandled promise rejection).
#[command]
pub fn track_error(
    client: State<'_, Arc<AptabaseClient>>,
    error_type: String,
    message: String,
    stack_trace: Option<String>,
    fatal: Option<bool>,
    kind: Option<String>,
) {
    let fatal = fatal.unwrap_or(false);
    let kind = match kind.as_deref() {
        Some("unhandled") => ErrorKind::Unhandled,
        Some("taskException") => ErrorKind::TaskException,
        _ if fatal => ErrorKind::Crash,
        _ => ErrorKind::Handled,
    };
    let report = ErrorReport {
        error_type,
        message,
        stack_trace,
        fatal,
    };
    client.track_error_as(report, kind);
}
