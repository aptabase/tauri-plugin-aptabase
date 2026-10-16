use std::{
    cmp::min,
    collections::VecDeque,
    sync::{Arc, RwLock},
    time::Duration,
};

use log::{debug, trace};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Url,
};
use serde_json::{json, Value};

use crate::{config::Config, sys::SystemProperties};

static HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct EventDispatcher {
    url: Url,
    queue: Arc<RwLock<VecDeque<Value>>>,
    http_client: reqwest::Client,
}

impl EventDispatcher {
    pub fn new(config: &Config, sys: &SystemProperties) -> Self {
        let mut headers = HeaderMap::new();
        let app_key_header = HeaderValue::from_str(config.app_key.as_str())
            .expect("failed to define App Key header value");
        headers.insert("App-Key", app_key_header);
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let user_agent = format!(
            "{}/{} {}/{} {}",
            sys.os_name, sys.os_version, sys.engine_name, sys.engine_version, sys.locale
        );
        let http_client = reqwest::Client::builder()
            .timeout(HTTP_REQUEST_TIMEOUT)
            .default_headers(headers)
            .user_agent(user_agent)
            .build()
            .expect("could not build http client");

        let queue = Arc::new(RwLock::new(VecDeque::new()));

        Self {
            url: config.ingest_api_url.clone(),
            queue,
            http_client,
        }
    }

    pub fn is_empty(&self) -> bool {
        let queue = self.queue.read().expect("could not lock queue for reading");
        queue.is_empty()
    }

    pub fn enqueue(&self, event: Value) {
        let mut queue = self.queue.write().expect("could not lock queue");
        queue.push_back(event);
    }

    pub fn enqueue_many(&self, events: Vec<Value>) {
        let mut queue = self.queue.write().expect("could not lock queue");
        queue.extend(events);
    }

    fn dequeue_many(&self, max: usize) -> Vec<Value> {
        let mut queue = self.queue.write().expect("could not lock queue");
        if queue.is_empty() {
            return Vec::new();
        }

        let dequeue_len = min(queue.len(), max);
        queue.drain(..dequeue_len).collect()
    }

    pub async fn flush(&self) {
        trace!("flushing tracking events");
        if self.is_empty() {
            trace!("nothing to send");
            return;
        }

        let mut failed_items = Vec::new();
        loop {
            let events_to_send = self.dequeue_many(25);
            if events_to_send.is_empty() {
                break;
            }

            trace!("preparing {} events to send", events_to_send.len());

            let body = json!(events_to_send);
            let response = self
                .http_client
                .post(self.url.clone())
                .json(&body)
                .send()
                .await;
            match response {
                Ok(response) => match response.status().is_success() {
                    true => {
                        trace!("sent {} tracking events", events_to_send.len());
                    }
                    false => {
                        debug!(
                            "failed to track_event with status code {}",
                            response.status()
                        );
                        if response.status().is_server_error() {
                            failed_items.extend(events_to_send);
                        }
                    }
                },
                Err(err) => {
                    failed_items.extend(events_to_send);
                    debug!("failed to track_event: {}", err.to_string());
                }
            }
        }

        self.enqueue_many(failed_items);
    }
}

// Delivery end to end: events tracked on a client and flushed to a local server.
#[cfg(test)]
mod tests {
    use std::{
        net::TcpListener,
        time::{Duration, Instant},
    };

    use serde_json::{json, Value};
    use tauri::async_runtime::block_on;
    use time::{format_description::well_known::Rfc3339, OffsetDateTime};

    use crate::{
        client::AptabaseClient,
        config::Config,
        test_server::{server, server_on, Server},
        InitOptions,
    };

    const APP_KEY: &str = "A-SH-0000000000";

    fn client(base: &str) -> AptabaseClient {
        let options = InitOptions {
            host: Some(base.into()),
            flush_interval: None,
        };
        AptabaseClient::new(&Config::new(APP_KEY.into(), options), "1.4.0".into())
    }

    /// The event names in each request the server got.
    fn batches(server: &Server) -> Vec<Vec<String>> {
        let requests = server.requests.lock().unwrap();
        requests
            .iter()
            .map(|(_, body)| {
                let events = body.as_array().unwrap();
                events
                    .iter()
                    .map(|event| event["eventName"].as_str().unwrap().to_string())
                    .collect()
            })
            .collect()
    }

    fn keys(value: &Value) -> Vec<&str> {
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn events_go_out_in_batches_of_25() {
        let server = server(&[200, 200, 200]);
        let client = client(&server.base);
        for i in 0..60 {
            client.track_event(&format!("event_{i}"), None).unwrap();
        }
        block_on(client.flush());
        let sizes: Vec<usize> = batches(&server).iter().map(Vec::len).collect();
        assert_eq!(sizes, [25, 25, 10]);
    }

    #[test]
    fn a_server_error_keeps_the_events_for_the_next_flush() {
        let server = server(&[503, 200]);
        let client = client(&server.base);
        client.track_event("app_started", None).unwrap();
        client.track_event("app_exited", None).unwrap();
        block_on(client.flush());
        block_on(client.flush());
        assert_eq!(
            batches(&server),
            [["app_started", "app_exited"], ["app_started", "app_exited"]]
        );
    }

    #[test]
    fn a_network_failure_keeps_the_events_for_the_next_flush() {
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let client = client(&format!("http://{addr}"));
        client.track_event("app_started", None).unwrap();
        // Nothing listens on the port yet: the connection is refused.
        block_on(client.flush());
        let server = server_on(TcpListener::bind(addr).unwrap(), &[200]);
        block_on(client.flush());
        assert_eq!(batches(&server), [["app_started"]]);
    }

    #[test]
    fn a_client_error_drops_the_events() {
        let server = server(&[400, 200]);
        let client = client(&server.base);
        client.track_event("app_started", None).unwrap();
        block_on(client.flush());
        block_on(client.flush());
        assert_eq!(batches(&server), [["app_started"]]);
    }

    #[test]
    fn events_are_sent_in_the_aptabase_format() {
        let server = server(&[200]);
        let client = client(&server.base);
        let props = json!({ "plan": "pro", "seats": 3 });
        client
            .track_event("app_started", Some(props.clone()))
            .unwrap();
        block_on(client.flush());

        let requests = server.requests.lock().unwrap();
        let (app_key, body) = &requests[0];
        assert_eq!(app_key, APP_KEY);
        let event = &body[0];
        assert_eq!(
            keys(event),
            [
                "eventName",
                "props",
                "sessionId",
                "systemProps",
                "timestamp"
            ]
        );
        assert_eq!(event["eventName"], "app_started");
        assert_eq!(event["props"], props);
        assert!(!event["sessionId"].as_str().unwrap().is_empty());
        OffsetDateTime::parse(event["timestamp"].as_str().unwrap(), &Rfc3339).unwrap();

        let system = &event["systemProps"];
        assert_eq!(
            keys(system),
            [
                "appVersion",
                "engineName",
                "engineVersion",
                "isDebug",
                "locale",
                "osName",
                "osVersion",
                "sdkVersion"
            ]
        );
        assert_eq!(system["appVersion"], "1.4.0");
        assert_eq!(
            system["sdkVersion"],
            concat!("tauri-plugin-aptabase@", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(system["isDebug"], cfg!(debug_assertions));
    }

    #[test]
    fn polling_flushes_without_a_manual_flush() {
        let server = server(&[200]);
        let client = client(&server.base);
        client.start_polling(Duration::from_millis(50));
        client.track_event("app_started", None).unwrap();
        let started = Instant::now();
        while server.requests.lock().unwrap().is_empty()
            && started.elapsed() < Duration::from_secs(5)
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(batches(&server), [["app_started"]]);
    }
}
