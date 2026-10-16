//! Error reports: what the plugin sends to Aptabase's error endpoint (`/api/v0/error`), one report per request.

use std::{
    backtrace::{Backtrace, BacktraceStatus},
    collections::VecDeque,
    panic::PanicHookInfo,
    sync::{mpsc, Arc, RwLock, RwLockWriteGuard},
    time::Duration,
};

use futures::lock::Mutex as AsyncMutex;
use log::debug;
use reqwest::{StatusCode, Url};
use serde_json::{json, Value};

use crate::sys::SystemProperties;

/// The platform every report names, as the other SDKs name theirs ("React Native", ".NET MAUI").
const PLATFORM: &str = "Tauri";

/// The most reports kept waiting for delivery: a crash loop shouldn't grow memory without bound.
const MAX_QUEUED_REPORTS: usize = 25;

// The error endpoint rejects a whole report when a field is longer than these. They count UTF-16 code units, as .NET
// measures a string's length, so a report is truncated the same way before it is sent.
const MAX_ERROR_MESSAGE: usize = 5000;
const MAX_ERROR_TYPE: usize = 100;
const MAX_STACK_TRACE: usize = 10000;
const MAX_OS_NAME: usize = 30;
const MAX_OS_VERSION: usize = 100;
const MAX_APP_VERSION: usize = 50;
const MAX_SDK_VERSION: usize = 40;

/// An error to report to Aptabase. It appears on the Errors page of the dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorReport {
    /// The error's type, e.g. `std::io::error::Error` or `TypeError`. Reports are grouped by it.
    pub error_type: String,
    /// What went wrong.
    pub message: String,
    /// Where it happened, when known.
    pub stack_trace: Option<String>,
    /// Whether the app can't recover from it (severity `fatal`, rather than `error`).
    pub fatal: bool,
}

impl ErrorReport {
    /// A report of an error of `error_type` with `message`.
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
            stack_trace: None,
            fatal: false,
        }
    }

    /// A report of `error`: its type name, its message followed by the errors that caused it, and a backtrace of
    /// where it was reported when `RUST_BACKTRACE` or `RUST_LIB_BACKTRACE` enables backtraces.
    pub fn from_error<E: std::error::Error + ?Sized>(error: &E) -> Self {
        let mut message = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            message.push_str("\nCaused by: ");
            message.push_str(&cause.to_string());
            source = cause.source();
        }
        // A trait object's type name is "dyn core::error::Error", which says nothing about the error.
        let type_name = std::any::type_name::<E>();
        let error_type = if type_name.starts_with("dyn ") {
            "Error"
        } else {
            type_name
        };
        let backtrace = Backtrace::capture();
        Self {
            error_type: error_type.to_string(),
            message,
            stack_trace: (backtrace.status() == BacktraceStatus::Captured)
                .then(|| backtrace.to_string()),
            fatal: false,
        }
    }

    /// The same report with `stack_trace`.
    pub fn with_stack_trace(mut self, stack_trace: impl Into<String>) -> Self {
        self.stack_trace = Some(stack_trace.into());
        self
    }

    /// The same report, fatal or not.
    pub fn with_fatal(mut self, fatal: bool) -> Self {
        self.fatal = fatal;
        self
    }
}

/// How an error was captured, as the error endpoint records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorKind {
    /// It ended the app.
    Crash,
    /// Nothing caught it, but the app carries on: a webview's uncaught error, a panic on a thread of its own.
    Unhandled,
    /// An asynchronous task failed: a panic in a task on Tauri's async runtime, a webview's unhandled rejection.
    TaskException,
    /// The app caught it and reported it.
    Handled,
}

impl ErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            ErrorKind::Crash => "crash",
            ErrorKind::Unhandled => "unhandled",
            ErrorKind::TaskException => "taskException",
            ErrorKind::Handled => "handled",
        }
    }
}

/// What a panic ends. In a task spawned on an async runtime (an async command, `tauri::async_runtime::spawn`), only
/// that task, which the runtime catches, on whatever thread it runs (a single-thread runtime runs its tasks on the
/// main thread); otherwise on the main thread (Rust names it "main"), the app; on any other thread, only that thread.
/// With `panic = "abort"`, every panic ends the app.
pub(crate) fn panic_kind(in_spawned_task: bool, on_main_thread: bool, aborts: bool) -> ErrorKind {
    if aborts {
        ErrorKind::Crash
    } else if in_spawned_task {
        ErrorKind::TaskException
    } else if on_main_thread {
        ErrorKind::Crash
    } else {
        ErrorKind::Unhandled
    }
}

/// The report of a panic, and how it was captured. The location is part of the message, so the same message from
/// two places (`called \`Option::unwrap()\` on a \`None\` value`) is reported as two errors.
pub(crate) fn panic_report(info: &PanicHookInfo<'_>, message: &str) -> (ErrorReport, ErrorKind) {
    let thread = std::thread::current();
    let kind = panic_kind(
        tokio::task::try_id().is_some(),
        thread.name() == Some("main"),
        cfg!(panic = "abort"),
    );
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
    let message = match &location {
        Some(at) => format!("{message} (at {at})"),
        None => message.to_string(),
    };
    let stack_trace = format!(
        "thread '{}' panicked at {}\n{}",
        thread.name().unwrap_or("<unnamed>"),
        location.as_deref().unwrap_or("an unknown location"),
        Backtrace::force_capture()
    );
    let report = ErrorReport {
        error_type: "panic".into(),
        message,
        stack_trace: Some(stack_trace),
        fatal: kind == ErrorKind::Crash,
    };
    (report, kind)
}

/// The body the error endpoint takes for `report`, stamped with the time and session it was captured in.
pub(crate) fn report_body(
    report: &ErrorReport,
    kind: ErrorKind,
    session_id: &str,
    timestamp: &str,
    sys: &SystemProperties,
    app_version: &str,
) -> Value {
    // The endpoint refuses a report without a type or a message, so neither is ever sent empty.
    let error_type = match report.error_type.trim() {
        "" => "Error",
        t => t,
    };
    let message = match report.message.trim() {
        "" => error_type,
        m => m,
    };
    json!({
        "errorMessage": truncate(message, MAX_ERROR_MESSAGE),
        "errorType": truncate(error_type, MAX_ERROR_TYPE),
        "stackTrace": report.stack_trace.as_deref().map(|s| truncate(s, MAX_STACK_TRACE)),
        "timestamp": timestamp,
        "sessionId": session_id,
        "platform": PLATFORM,
        // Unlike the events endpoint, the error endpoint doesn't read the OS from the User-Agent.
        "osName": truncate(&sys.os_name, MAX_OS_NAME),
        "osVersion": truncate(&sys.os_version, MAX_OS_VERSION),
        "appVersion": truncate(app_version, MAX_APP_VERSION),
        "sdkVersion": truncate(concat!(env!("CARGO_PKG_NAME"), "@", env!("CARGO_PKG_VERSION")), MAX_SDK_VERSION),
        "severity": if report.fatal { "fatal" } else { "error" },
        "kind": kind.as_str(),
        "isDebug": sys.is_debug,
    })
}

/// `value`, cut to at most `max` UTF-16 code units on a character boundary.
fn truncate(value: &str, max: usize) -> String {
    let mut units = 0;
    for (at, c) in value.char_indices() {
        units += c.len_utf16();
        if units > max {
            return value[..at].to_string();
        }
    }
    value.to_string()
}

/// Whether a report the server answered with `status` (or didn't answer) can be delivered by sending it again.
/// No connection, a timeout (408), the per-IP rate limit (429) and server errors pass; 403 (the month's error quota
/// is used up, until the 1st) and any other refusal never will.
fn retry_later(status: Option<StatusCode>) -> bool {
    match status {
        None => true,
        Some(s) => {
            s == StatusCode::REQUEST_TIMEOUT
                || s == StatusCode::TOO_MANY_REQUESTS
                || s.is_server_error()
        }
    }
}

/// Sends error reports one per request, right away, keeping the ones that can be delivered later for the next flush.
pub(crate) struct ErrorDispatcher {
    url: Url,
    queue: RwLock<VecDeque<Value>>,
    flushing: AsyncMutex<()>,
    http_client: reqwest::Client,
    /// The same client without idle connections, for a crash's report on a runtime of its own: a kept connection is
    /// served by the runtime that opened it, which may be the one the crashing thread is blocking.
    crash_client: reqwest::Client,
}

impl ErrorDispatcher {
    pub fn new(url: Url, http_client: reqwest::Client, crash_client: reqwest::Client) -> Self {
        Self {
            url,
            queue: RwLock::new(VecDeque::new()),
            flushing: AsyncMutex::new(()),
            http_client,
            crash_client,
        }
    }

    // Reports are queued from panic hooks too, where a lock poisoned by another panic must not panic again.
    fn queue(&self) -> RwLockWriteGuard<'_, VecDeque<Value>> {
        self.queue.write().unwrap_or_else(|e| e.into_inner())
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.queue().len()
    }

    /// Queues a report; false when the queue is full and the report was dropped. A fatal report finding the queue full
    /// takes the place of the oldest report that isn't.
    pub fn enqueue(&self, report: Value) -> bool {
        let mut queue = self.queue();
        if queue.len() >= MAX_QUEUED_REPORTS {
            let fatal = |r: &Value| r["severity"] == "fatal";
            match queue
                .iter()
                .position(|r| !fatal(r))
                .filter(|_| fatal(&report))
            {
                Some(oldest) => {
                    queue.remove(oldest);
                    debug!("error report queue is full, dropping an older report for a fatal one");
                }
                None => {
                    debug!("error report queue is full, dropping the report");
                    return false;
                }
            }
        }
        queue.push_back(report);
        true
    }

    /// Sends the queued reports, oldest first. One that can't be delivered now stays queued, with those behind it.
    pub async fn flush(&self) {
        let _flushing = self.flushing.lock().await;
        loop {
            let Some(report) = self.queue().pop_front() else {
                return;
            };
            if !self.send(&self.http_client, &report).await {
                self.queue().push_front(report);
                return;
            }
        }
    }

    /// Sends one report: true once it is settled (delivered, or refused for good), false when it should be sent
    /// again later.
    async fn send(&self, client: &reqwest::Client, report: &Value) -> bool {
        let response = client.post(self.url.clone()).json(report).send().await;
        let status = match response {
            Ok(response) if response.status().is_success() => return true,
            Ok(response) => Some(response.status()),
            Err(err) => {
                debug!("failed to send error report: {err}");
                None
            }
        };
        let retry = retry_later(status);
        if let Some(status) = status {
            debug!(
                "error report refused with status {status}{}",
                if retry { ", will retry" } else { ", dropped" }
            );
        }
        !retry
    }
}

/// Sends a crash's report, then the reports queued before it, waiting at most `timeout` (the app is about to exit).
///
/// The crash goes straight out, never through the queue: a full queue can't drop it, and it doesn't wait for a flush
/// in progress, which may be a send on the very runtime the crashing thread is stuck on. For the same reason it is
/// sent from a thread and a runtime of its own, on a connection of its own: the crashing thread may be the only one of
/// the runtime Tauri uses (an app can hand it a single-thread runtime), and that runtime runs nothing while it waits.
pub(crate) fn send_crash_blocking(
    dispatcher: &Arc<ErrorDispatcher>,
    crash: Value,
    timeout: Duration,
) {
    let dispatcher = dispatcher.clone();
    block_on_own_thread(
        async move {
            let client = &dispatcher.crash_client;
            dispatcher.send(client, &crash).await;
            loop {
                let Some(report) = dispatcher.queue().pop_front() else {
                    break;
                };
                if !dispatcher.send(client, &report).await {
                    dispatcher.queue().push_front(report);
                    break;
                }
            }
        },
        timeout,
    );
}

/// Runs `work` on a thread and a runtime of its own, waiting for it at most `timeout`: what a panic hook can do, since
/// it may run on the only thread of the runtime Tauri uses, which runs nothing while the hook waits.
pub(crate) fn block_on_own_thread<F>(work: F, timeout: Duration)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let (done, wait) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("aptabase-flush".into())
        .spawn(move || {
            if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                runtime.block_on(work);
            }
            let _ = done.send(());
        });
    if spawned.is_ok() {
        let _ = wait.recv_timeout(timeout);
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::test_server::server;

    fn sys() -> SystemProperties {
        SystemProperties {
            is_debug: true,
            os_name: "macOS".into(),
            os_version: "15.4".into(),
            locale: "en-US".into(),
            engine_name: "WebKit".into(),
            engine_version: "620.1".into(),
        }
    }

    fn body(report: &ErrorReport, kind: ErrorKind) -> Value {
        report_body(report, kind, "123", "2026-10-16T12:00:00Z", &sys(), "1.4.0")
    }

    #[test]
    fn a_report_has_every_field_the_endpoint_takes() {
        let b = body(
            &ErrorReport::new("TypeError", "TypeError: x is undefined"),
            ErrorKind::Handled,
        );
        assert_eq!(b["errorType"], "TypeError");
        assert_eq!(b["errorMessage"], "TypeError: x is undefined");
        assert_eq!(b["stackTrace"], Value::Null);
        assert_eq!(b["platform"], "Tauri");
        assert_eq!(b["osName"], "macOS");
        assert_eq!(b["osVersion"], "15.4");
        assert_eq!(b["appVersion"], "1.4.0");
        assert_eq!(b["sessionId"], "123");
        assert_eq!(b["severity"], "error");
        assert_eq!(b["kind"], "handled");
        assert_eq!(b["isDebug"], true);
        assert!(b["sdkVersion"]
            .as_str()
            .unwrap()
            .starts_with("tauri-plugin-aptabase@"));
    }

    #[test]
    fn severity_and_kind_follow_how_it_was_captured() {
        let fatal = ErrorReport::new("panic", "boom").with_fatal(true);
        let b = body(&fatal, ErrorKind::Crash);
        assert_eq!(
            (b["severity"].as_str(), b["kind"].as_str()),
            (Some("fatal"), Some("crash"))
        );
        let b = body(&ErrorReport::new("Error", "no"), ErrorKind::TaskException);
        assert_eq!(
            (b["severity"].as_str(), b["kind"].as_str()),
            (Some("error"), Some("taskException"))
        );
        let b = body(&ErrorReport::new("Error", "no"), ErrorKind::Unhandled);
        assert_eq!(b["kind"], "unhandled");
    }

    #[test]
    fn a_report_is_never_sent_without_a_type_or_a_message() {
        let b = body(&ErrorReport::new(" ", ""), ErrorKind::Handled);
        assert_eq!(
            (b["errorType"].as_str(), b["errorMessage"].as_str()),
            (Some("Error"), Some("Error"))
        );
        let b = body(&ErrorReport::new("io::Error", "  "), ErrorKind::Handled);
        assert_eq!(b["errorMessage"], "io::Error");
    }

    #[test]
    fn fields_are_cut_to_the_endpoints_limits_in_utf16_units() {
        // An emoji is two UTF-16 units: 2,600 of them are 5,200 units, past the message's 5,000.
        let long = "😀".repeat(2600);
        let b = body(
            &ErrorReport::new("E".repeat(150), long).with_stack_trace("s".repeat(12_000)),
            ErrorKind::Handled,
        );
        let message = b["errorMessage"].as_str().unwrap();
        assert_eq!(message.encode_utf16().count(), 5000);
        assert!(message.chars().all(|c| c == '😀'));
        assert_eq!(b["errorType"].as_str().unwrap().len(), 100);
        assert_eq!(b["stackTrace"].as_str().unwrap().len(), 10_000);
        assert_eq!(truncate("a😀", 2), "a", "a character is never split");
    }

    #[derive(Debug)]
    struct Outer(std::io::Error);

    impl std::fmt::Display for Outer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "could not read settings")
        }
    }

    impl std::error::Error for Outer {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn an_error_is_reported_with_its_type_and_its_causes() {
        let err = Outer(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "settings.json is missing",
        ));
        let report = ErrorReport::from_error(&err);
        assert!(
            report.error_type.ends_with("Outer"),
            "{}",
            report.error_type
        );
        assert_eq!(
            report.message,
            "could not read settings\nCaused by: settings.json is missing"
        );
        assert!(!report.fatal);
        let as_dyn: &dyn std::error::Error = &err;
        assert_eq!(ErrorReport::from_error(as_dyn).error_type, "Error");
    }

    #[test]
    fn a_panic_is_a_crash_only_where_it_ends_the_app() {
        // (in a spawned task, on the main thread, panic = "abort")
        assert_eq!(panic_kind(false, true, false), ErrorKind::Crash);
        assert_eq!(panic_kind(true, false, false), ErrorKind::TaskException);
        assert_eq!(
            panic_kind(true, true, false),
            ErrorKind::TaskException,
            "a single-thread runtime runs its tasks on the main thread"
        );
        assert_eq!(panic_kind(false, false, false), ErrorKind::Unhandled);
        assert_eq!(panic_kind(true, false, true), ErrorKind::Crash);
    }

    #[test]
    fn only_what_can_be_delivered_later_is_retried() {
        for status in [408, 429, 500, 502, 503] {
            assert!(retry_later(StatusCode::from_u16(status).ok()), "{status}");
        }
        for status in [400, 401, 403, 404, 413] {
            assert!(!retry_later(StatusCode::from_u16(status).ok()), "{status}");
        }
        assert!(retry_later(None), "no connection");
    }

    fn dispatcher(url: Url) -> Arc<ErrorDispatcher> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "App-Key",
            reqwest::header::HeaderValue::from_static("A-DEV-0000000000"),
        );
        let client = |idle: usize| {
            reqwest::Client::builder()
                .default_headers(headers.clone())
                .pool_max_idle_per_host(idle)
                .build()
                .unwrap()
        };
        Arc::new(ErrorDispatcher::new(url, client(usize::MAX), client(0)))
    }

    fn report(n: u32) -> Value {
        json!({ "errorType": "Error", "errorMessage": format!("error {n}") })
    }

    #[test]
    fn reports_are_sent_one_per_request_with_the_app_key() {
        let server = server(&[202, 202]);
        let errors = dispatcher(server.url.clone());
        errors.enqueue(report(1));
        errors.enqueue(report(2));
        tauri::async_runtime::block_on(errors.flush());
        assert_eq!(errors.len(), 0);
        let sent = server.requests.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], ("A-DEV-0000000000".to_string(), report(1)));
        assert_eq!(sent[1].1, report(2));
    }

    #[test]
    fn a_report_the_server_can_take_later_is_kept_and_sent_on_the_next_flush() {
        let server = server(&[503, 202, 202]);
        let errors = dispatcher(server.url.clone());
        errors.enqueue(report(1));
        errors.enqueue(report(2));
        tauri::async_runtime::block_on(errors.flush());
        assert_eq!(
            errors.len(),
            2,
            "the failed report and the one behind it wait"
        );
        tauri::async_runtime::block_on(errors.flush());
        assert_eq!(errors.len(), 0);
        let sent: Vec<Value> = server
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|(_, b)| b.clone())
            .collect();
        assert_eq!(
            sent,
            [report(1), report(1), report(2)],
            "in order, the first one twice"
        );
    }

    #[test]
    fn a_report_over_quota_is_dropped_not_retried() {
        let server = server(&[403, 202]);
        let errors = dispatcher(server.url.clone());
        errors.enqueue(report(1));
        errors.enqueue(report(2));
        tauri::async_runtime::block_on(errors.flush());
        assert_eq!(errors.len(), 0);
        assert_eq!(server.requests.lock().unwrap().len(), 2);
    }

    #[test]
    fn the_queue_holds_at_most_25_reports() {
        let errors = dispatcher("http://127.0.0.1:9/api/v0/error".parse().unwrap());
        for n in 0..30 {
            assert_eq!(errors.enqueue(report(n)), n < 25, "{n}");
        }
        assert_eq!(errors.len(), 25);
    }

    fn fatal(n: u32) -> Value {
        json!({ "errorType": "panic", "errorMessage": format!("crash {n}"), "severity": "fatal" })
    }

    #[test]
    fn a_fatal_report_finding_the_queue_full_takes_the_oldest_others_place() {
        let errors = dispatcher("http://127.0.0.1:9/api/v0/error".parse().unwrap());
        errors.enqueue(fatal(0));
        for n in 1..25 {
            errors.enqueue(report(n));
        }
        assert!(
            !errors.enqueue(report(25)),
            "a report that isn't fatal is dropped"
        );
        assert!(errors.enqueue(fatal(26)));
        let queue = errors.queue();
        assert_eq!(queue.len(), 25);
        assert_eq!(queue[0], fatal(0), "fatal reports are kept");
        assert_eq!(queue[1], report(2), "the oldest other report made room");
        assert_eq!(queue[24], fatal(26));
    }

    #[test]
    fn a_full_queue_of_fatal_reports_drops_the_next() {
        let errors = dispatcher("http://127.0.0.1:9/api/v0/error".parse().unwrap());
        for n in 0..25 {
            errors.enqueue(fatal(n));
        }
        assert!(!errors.enqueue(fatal(25)));
        assert_eq!(errors.len(), 25);
    }

    // A crash's report goes out first, from outside any runtime as a panic hook on the main thread is, and even when
    // the queue is full; the reports queued before it follow.
    #[test]
    fn a_crash_is_sent_first_even_with_the_queue_full() {
        let server = server(&[202; 26]);
        let errors = dispatcher(server.url.clone());
        for n in 0..25 {
            errors.enqueue(report(n));
        }
        let sending = errors.clone();
        std::thread::spawn(move || send_crash_blocking(&sending, fatal(0), Duration::from_secs(5)))
            .join()
            .unwrap();
        let sent = server.requests.lock().unwrap();
        assert_eq!(sent.len(), 26);
        assert_eq!(sent[0].1, fatal(0));
        assert_eq!(sent[1].1, report(0));
        assert_eq!(errors.len(), 0);
    }

    // A flush in progress may be a send on the very runtime the crashing thread is stuck on: it can't finish, so the
    // crash doesn't wait for it.
    #[test]
    fn a_crash_does_not_wait_for_a_flush_in_progress() {
        let server = server(&[202]);
        let errors = dispatcher(server.url.clone());
        let _in_progress = futures::executor::block_on(errors.flushing.lock());
        let started = std::time::Instant::now();
        send_crash_blocking(&errors, fatal(0), Duration::from_secs(3));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(server.requests.lock().unwrap()[0].1, fatal(0));
    }

    #[test]
    fn a_crash_waits_at_most_its_timeout() {
        // Answered never: the send is still waiting when the timeout returns.
        let server = server(&[0]);
        let errors = dispatcher(server.url.clone());
        let started = std::time::Instant::now();
        send_crash_blocking(&errors, fatal(0), Duration::from_millis(300));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }
}
