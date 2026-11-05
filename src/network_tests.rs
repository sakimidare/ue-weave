use crate::{
    log::Logger,
    network::{self, Policy},
};
use reqwest::blocking::Client;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

fn server(responses: Vec<(&'static str, Duration)>) -> (String, thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut count = 0;
        for (response, delay) in responses {
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() > deadline {
                            return count;
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let _ = stream.read(&mut [0; 4096]);
            count += 1;
            thread::sleep(delay);
            let _ = stream.write_all(response.as_bytes());
        }
        count
    });
    (url, handle)
}

const OK: &str = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
const FAIL: &str =
    "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
fn policy(retries: u8) -> Policy {
    Policy {
        allow_private_network: true,
        retries,
        backoff: Duration::ZERO,
    }
}
fn client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap()
}

#[test]
fn retries_transient_status_then_succeeds() {
    let (url, server) = server(vec![(FAIL, Duration::ZERO), (OK, Duration::ZERO)]);
    let result = network::fetch(&client(), &url, policy(2), None, &Logger::new(false)).unwrap();
    assert_eq!(result.bytes, b"OK");
    assert_eq!(result.attempts, 2);
    assert_eq!(server.join().unwrap(), 2);
}

#[test]
fn bounded_retries_and_disabled_retries() {
    for retries in [0, 2] {
        let (url, server) = server(vec![(FAIL, Duration::ZERO); retries as usize + 1]);
        let error = network::fetch(&client(), &url, policy(retries), None, &Logger::new(false))
            .err()
            .unwrap();
        assert_eq!(error.attempts, retries + 1);
        assert_eq!(error.status, Some(503));
        assert_eq!(error.kind, crate::error::NetworkKind::HttpStatus);
        assert_eq!(server.join().unwrap(), retries as usize + 1);
    }
}

#[test]
fn permanent_status_and_long_retry_after_stop_immediately() {
    for response in [
        "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 120\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 429 Too Many Requests\r\nRetry-After: Wed, 01 Jan 2030 00:00:00 GMT\r\nContent-Length: 0\r\n\r\n",
    ] {
        let (url, server) = server(vec![(response, Duration::ZERO)]);
        let error = network::fetch(&client(), &url, policy(2), None, &Logger::new(false)).err().unwrap();
        assert_eq!(error.attempts, 1);
        assert_eq!(server.join().unwrap(), 1);
    }
}

#[test]
fn rate_limit_and_truncated_body_are_retried() {
    for response in [
        "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Length: 0\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nX",
    ] {
        let (url, server) = server(vec![(response, Duration::ZERO), (OK, Duration::ZERO)]);
        let result =
            network::fetch(&client(), &url, policy(2), Some(100), &Logger::new(false)).unwrap();
        assert_eq!(result.bytes, b"OK");
        assert_eq!(result.attempts, 2);
        assert_eq!(server.join().unwrap(), 2);
    }
}

#[test]
fn timeout_and_tls_report_underlying_causes() {
    let (url, server) = server(vec![(OK, Duration::from_millis(150))]);
    let timeout_client = Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(50))
        .build()
        .unwrap();
    let error = network::fetch(&timeout_client, &url, policy(0), None, &Logger::new(false))
        .err()
        .unwrap();
    assert_eq!(error.kind, crate::error::NetworkKind::Timeout);
    assert!(error.detail.contains("timed out"));
    server.join().unwrap();

    let (url, server) = self::server(vec![(OK, Duration::ZERO)]);
    let error = network::fetch(
        &crate::network_policy::client(
            Duration::from_secs(5),
            true,
            reqwest::header::HeaderMap::new(),
        )
        .unwrap(),
        &url.replace("http:", "https:"),
        policy(2),
        None,
        &Logger::new(false),
    )
    .err()
    .unwrap();
    assert_eq!(
        error.kind,
        crate::error::NetworkKind::Tls,
        "{}",
        error.detail
    );
    assert_eq!(error.attempts, 1);
    server.join().unwrap();
}

#[test]
fn size_limit_checks_headers_and_streamed_bytes() {
    for response in [OK, "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nOK"] {
        let (url, server) = server(vec![(response, Duration::ZERO)]);
        let error = network::fetch(&client(), &url, policy(2), Some(1), &Logger::new(false))
            .err()
            .unwrap();
        assert_eq!(error.kind, crate::error::NetworkKind::SizeLimit);
        assert_eq!(error.attempts, 1);
        server.join().unwrap();
    }
}
