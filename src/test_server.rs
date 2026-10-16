//! A local HTTP server for tests: it answers each request with the next status it was given and keeps what it was
//! sent (the App-Key header and the JSON body). A status of 0 never answers: the request stays in flight.

// Shared with the integration tests, which don't use all of it.
#![allow(dead_code)]

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
};

use reqwest::Url;
use serde_json::Value;

/// A server that answers each request with the next status in `statuses`, keeping what it was sent.
pub(crate) struct Server {
    /// Where it listens, e.g. `http://127.0.0.1:50211`.
    pub base: String,
    /// Its error endpoint.
    pub url: Url,
    pub requests: Arc<Mutex<Vec<(String, Value)>>>,
}

pub(crate) fn server(statuses: &[u16]) -> Server {
    server_on(TcpListener::bind("127.0.0.1:0").unwrap(), statuses)
}

/// [`server`] on a listener the test bound itself, e.g. on a port an earlier request found closed.
pub(crate) fn server_on(listener: TcpListener, statuses: &[u16]) -> Server {
    let base = format!("http://{}", listener.local_addr().unwrap());
    let url = format!("{base}/api/v0/error").parse().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    let statuses = statuses.to_vec();
    std::thread::spawn(move || {
        let mut unanswered = Vec::new();
        for (stream, status) in listener.incoming().zip(statuses) {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let (mut app_key, mut length) = (String::new(), 0);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let lower = line.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                if lower.starts_with("app-key:") {
                    app_key = line["app-key:".len()..].trim().to_string();
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            seen.lock()
                .unwrap()
                .push((app_key, serde_json::from_slice(&body).unwrap()));
            if status == 0 {
                unanswered.push(stream);
                continue;
            }
            let reply =
                format!("HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            stream.write_all(reply.as_bytes()).unwrap();
        }
        // The last request answered: keep the unanswered ones in flight until the test ends.
        std::thread::sleep(std::time::Duration::from_secs(60));
        drop(unanswered);
    });
    Server {
        base,
        url,
        requests,
    }
}
