//! 页面和资源下载共用的 GET 请求与有限重试。
use crate::{error::NetworkError, input, log::Logger};
use reqwest::blocking::Client;
use std::{error::Error, io::Read, time::Duration};

/// 页面和单个资源共用的下载上限（64 MiB）。
pub const MAX_BODY_SIZE: u64 = 64 * 1024 * 1024;

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

fn classify(
    error: &(dyn Error + 'static),
    fallback: crate::error::NetworkKind,
) -> crate::error::NetworkKind {
    let mut current = Some(error);
    while let Some(source) = current {
        if let Some(error) = source.downcast_ref::<crate::network_policy::RoutingError>() {
            return match error {
                crate::network_policy::RoutingError::Dns(_) => crate::error::NetworkKind::Dns,
                _ => crate::error::NetworkKind::NetworkPolicy,
            };
        }
        if source.is::<rustls::Error>() {
            return crate::error::NetworkKind::Tls;
        }
        if let Some(error) = source.downcast_ref::<reqwest::Error>() {
            if error.is_timeout() {
                return crate::error::NetworkKind::Timeout;
            }
        }
        if let Some(error) = source.downcast_ref::<std::io::Error>() {
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
                    | std::io::ErrorKind::BrokenPipe
            ) {
                return crate::error::NetworkKind::Connect;
            }
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) {
                return crate::error::NetworkKind::Timeout;
            }
        }
        // io::Error 内部可能包装 TLS 错误，需要同时检查 get_ref()。
        current = source
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .map(|inner| inner as &(dyn Error + 'static))
            .or_else(|| source.source());
    }
    fallback
}

fn failure<E: Error + Send + Sync + 'static>(
    url: &str,
    error: E,
    fallback: crate::error::NetworkKind,
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
        crate::error::NetworkKind::Timeout
        | crate::error::NetworkKind::Dns
        | crate::error::NetworkKind::Connect
        | crate::error::NetworkKind::ReadResponse => true,
        crate::error::NetworkKind::HttpStatus => {
            matches!(error.status, Some(408 | 429 | 500 | 502 | 503 | 504))
        }
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
    let parsed = url::Url::parse(url)
        .map_err(|error| failure(url, error, crate::error::NetworkKind::InvalidUrl))?;
    crate::network_policy::check_url(&parsed, policy.allow_private_network)
        .map_err(|error| failure(url, error, crate::error::NetworkKind::NetworkPolicy))?;
    let attempts = policy.retries.min(5) + 1;
    let mut attempt = 1;
    loop {
        logger.stage("request", format!("GET {url}，尝试 {attempt}/{attempts}"));
        let mut retry_after = None;
        let result = (|| {
            let response = client.get(url).send().map_err(|error| {
                let fallback = if error.is_connect() {
                    crate::error::NetworkKind::Connect
                } else {
                    crate::error::NetworkKind::RequestFailed
                };
                failure(url, error, fallback)
            })?;
            let resolved = response.url().to_string();
            let status = response.status();
            if !status.is_success() {
                // 日期格式或过长的 Retry-After 会结束本次请求。
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
                    kind: crate::error::NetworkKind::HttpStatus,
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
                    kind: crate::error::NetworkKind::SizeLimit,
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
                .map_err(|error| {
                    failure(&resolved, error, crate::error::NetworkKind::ReadResponse)
                })?;
            if limit.is_some_and(|limit| bytes.len() as u64 > limit) {
                return Err(NetworkError {
                    kind: crate::error::NetworkKind::SizeLimit,
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
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classification_uses_error_types_not_message_words() {
        let misleading = std::io::Error::other("certificate tls ssl dns connection refused");
        assert_eq!(
            classify(&misleading, crate::error::NetworkKind::RequestFailed),
            crate::error::NetworkKind::RequestFailed
        );
        for kind in [
            std::io::ErrorKind::ConnectionAborted,
            std::io::ErrorKind::ConnectionReset,
            std::io::ErrorKind::BrokenPipe,
        ] {
            assert_eq!(
                classify(
                    &std::io::Error::new(kind, "本地化消息"),
                    crate::error::NetworkKind::RequestFailed
                ),
                crate::error::NetworkKind::Connect
            );
        }
    }
    #[test]
    fn typed_tls_errors_stop_retries_even_without_tls_in_message() {
        let tls = rustls::Error::General("record overflow".into());
        let wrapped = std::io::Error::other(std::io::Error::other(tls));
        let error = failure(
            "https://example.org/",
            wrapped,
            crate::error::NetworkKind::Connect,
        );
        assert_eq!(error.kind, crate::error::NetworkKind::Tls);
        assert!(!transient(&error));

        let reset = std::io::Error::from(std::io::ErrorKind::ConnectionAborted);
        let error = failure(
            "https://example.org/",
            reset,
            crate::error::NetworkKind::Connect,
        );
        assert_eq!(error.kind, crate::error::NetworkKind::Connect);
        assert!(transient(&error));
    }
    #[test]
    fn classifies_nested_dns_error_and_retains_cause_chain() {
        let error = std::io::Error::other(std::io::Error::other(
            crate::network_policy::RoutingError::Dns(std::io::Error::other(
                "failed to lookup address information",
            )),
        ));
        let error = failure(
            "https://example.invalid/",
            error,
            crate::error::NetworkKind::Connect,
        );
        assert_eq!(error.kind, crate::error::NetworkKind::Dns);
        assert!(error.details()["cause"]
            .as_str()
            .unwrap()
            .contains("failed to lookup"));
        assert!(transient(&error));
    }
}
