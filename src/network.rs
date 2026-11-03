//! Bounded GET retries shared by page and asset downloads.
use crate::{error::NetworkError, input, log::Logger};
use reqwest::blocking::Client;
use std::{error::Error, io::Read, time::Duration};

#[derive(Clone, Copy)]
pub struct Policy {
    pub allow_private_network: bool,
    pub retries: u8,
    pub backoff: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            allow_private_network: false,
            retries: 2,
            backoff: Duration::from_millis(250),
        }
    }
}

pub struct Download {
    pub bytes: Vec<u8>,
    pub url: String,
    pub charset: Option<String>,
    pub attempts: u8,
    pub status: u16,
    pub content_type: Option<String>,
    pub content_disposition: Option<String>,
}

fn chain(error: &(dyn Error + 'static)) -> String {
    let mut messages = vec![error.to_string()];
    let mut source = error.source();
    while let Some(error) = source {
        let message = error.to_string();
        if messages.last() != Some(&message) {
            messages.push(message);
        }
        source = error.source();
    }
    messages.join(": ")
}

fn classify(error: &(dyn Error + 'static), fallback: &'static str) -> &'static str {
    if chain(error).contains("network-policy:") {
        return "network_policy";
    }
    let mut current = Some(error);
    while let Some(source) = current {
        if let Some(error) = source.downcast_ref::<reqwest::Error>() {
            if error.is_timeout() {
                return "timeout";
            }
        }
        if let Some(error) = source.downcast_ref::<std::io::Error>() {
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) {
                return "timeout";
            }
        }
        current = source.source();
    }
    // TLS and DNS backends expose their diagnostics through the source chain.
    let message = chain(error.source().unwrap_or(error)).to_lowercase();
    if ["certificate", "tls", "ssl"]
        .iter()
        .any(|s| message.contains(s))
    {
        "tls"
    } else if [
        "dns",
        "name or service not known",
        "name resolution",
        "nodename nor servname",
        "failed to lookup address",
    ]
    .iter()
    .any(|s| message.contains(s))
    {
        "dns"
    } else if message.contains("timed out") {
        "timeout"
    } else if [
        "connection reset",
        "connection closed",
        "connection refused",
        "broken pipe",
    ]
    .iter()
    .any(|s| message.contains(s))
    {
        "connect"
    } else {
        fallback
    }
}

fn failure<E: Error + Send + Sync + 'static>(
    url: &str,
    error: E,
    fallback: &'static str,
) -> NetworkError {
    NetworkError {
        kind: classify(&error, fallback),
        url: url.into(),
        attempts: 1,
        status: None,
        detail: chain(&error),
        source: Some(Box::new(error)),
    }
}

fn transient(error: &NetworkError) -> bool {
    match error.kind {
        "timeout" | "dns" | "connect" | "read_response" => true,
        "http_status" => matches!(error.status, Some(408 | 429 | 500 | 502 | 503 | 504)),
        _ => false,
    }
}

pub fn fetch(
    client: &Client,
    url: &str,
    policy: Policy,
    limit: Option<u64>,
    logger: &Logger,
) -> Result<Download, NetworkError> {
    let parsed = url::Url::parse(url).map_err(|error| failure(url, error, "invalid_url"))?;
    crate::network_policy::check_url(&parsed, policy.allow_private_network)
        .map_err(|error| failure(url, error, "network_policy"))?;
    let attempts = policy.retries.min(5) + 1;
    for attempt in 1..=attempts {
        logger.stage("request", format!("GET {url}，尝试 {attempt}/{attempts}"));
        let mut retry_after = None;
        let result = (|| {
            let response = client.get(url).send().map_err(|error| {
                let fallback = if error.is_connect() {
                    "connect"
                } else {
                    "request_failed"
                };
                failure(url, error, fallback)
            })?;
            let resolved = response.url().to_string();
            let status = response.status();
            if !status.is_success() {
                // Date-form and long Retry-After values end this request.
                retry_after = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .map(|header| {
                        header
                            .to_str()
                            .ok()
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(u64::MAX)
                    });
                return Err(NetworkError {
                    kind: "http_status",
                    url: resolved,
                    attempts: attempt,
                    status: Some(status.as_u16()),
                    detail: format!(
                        "HTTP {status}{}",
                        if retry_after.is_some_and(|seconds| seconds > 30) {
                            "；Retry-After 等待时间超过 30 秒或使用日期格式，本次结束请求"
                        } else {
                            ""
                        }
                    ),
                    source: None,
                });
            }
            if limit.is_some_and(|limit| response.content_length().is_some_and(|size| size > limit))
            {
                return Err(NetworkError {
                    kind: "size_limit",
                    url: resolved,
                    attempts: attempt,
                    status: Some(status.as_u16()),
                    detail: format!("资源超过 {} 字节限制", limit.unwrap()),
                    source: None,
                });
            }
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|s| s.to_str().ok())
                .map(str::to_string);
            let charset = content_type.as_deref().and_then(input::charset);
            let content_disposition = response
                .headers()
                .get(reqwest::header::CONTENT_DISPOSITION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let mut bytes = Vec::new();
            response
                .take(limit.map(|n| n.saturating_add(1)).unwrap_or(u64::MAX))
                .read_to_end(&mut bytes)
                .map_err(|error| failure(&resolved, error, "read_response"))?;
            if limit.is_some_and(|limit| bytes.len() as u64 > limit) {
                return Err(NetworkError {
                    kind: "size_limit",
                    url: resolved,
                    attempts: attempt,
                    status: Some(status.as_u16()),
                    detail: format!("资源超过 {} 字节限制", limit.unwrap()),
                    source: None,
                });
            }
            Ok(Download {
                bytes,
                url: resolved,
                charset,
                attempts: attempt,
                status: status.as_u16(),
                content_type,
                content_disposition,
            })
        })();
        match result {
            Ok(download) => return Ok(download),
            Err(mut error) => {
                error.attempts = attempt;
                if attempt == attempts
                    || !transient(&error)
                    || retry_after.is_some_and(|seconds| seconds > 30)
                {
                    return Err(error);
                }
                let delay = policy
                    .backoff
                    .saturating_mul(1 << (attempt - 1))
                    .max(Duration::from_secs(retry_after.unwrap_or(0)));
                logger.retry(&error, delay);
                std::thread::sleep(delay);
            }
        }
    }
    unreachable!("at least one attempt")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classifies_nested_dns_error_and_retains_cause_chain() {
        let error = std::io::Error::other(std::io::Error::other(
            "dns error: failed to lookup address information",
        ));
        let error = failure("https://example.invalid/", error, "connect");
        assert_eq!(error.kind, "dns");
        assert!(error.details()["cause"]
            .as_str()
            .unwrap()
            .contains("failed to lookup"));
        assert!(transient(&error));
    }
}
