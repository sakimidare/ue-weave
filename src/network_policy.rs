//! 校验 DNS 返回地址及每次重定向的目标。
use reqwest::{
    blocking::Client,
    dns::{Addrs, Name, Resolve, Resolving},
};
use std::{
    io,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};
use url::{Host, Url};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RoutingError {
    #[error("目标地址属于私网或保留网段；访问此网络请设置 --allow-private-network")]
    Restricted,
    #[error("请求需要 HTTP/HTTPS 地址和独立于 URL 的身份信息")]
    InvalidAddress,
    #[error("域名解析失败：{0}")]
    Dns(#[source] io::Error),
}

fn policy_error() -> RoutingError {
    RoutingError::Restricted
}

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && (b == 168 || (b == 0 && matches!(c, 0 | 2)) || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return public_ip(v4.into());
            }
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

pub fn check_url(url: &Url, allow_private: bool) -> Result<(), RoutingError> {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(RoutingError::InvalidAddress);
    }
    if allow_private {
        return Ok(());
    }
    match url.host() {
        Some(Host::Ipv4(ip)) if public_ip(ip.into()) => Ok(()),
        Some(Host::Ipv6(ip)) if public_ip(ip.into()) => Ok(()),
        Some(Host::Domain(host))
            if host.trim_end_matches('.') != "localhost"
                && !host.trim_end_matches('.').ends_with(".localhost") =>
        {
            Ok(())
        }
        _ => Err(policy_error()),
    }
}

struct CheckedResolver {
    allow_private: bool,
}
impl Resolve for CheckedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let allow_private = self.allow_private;
        Box::pin(async move {
            let addresses: Vec<SocketAddr> = tokio::task::spawn_blocking(move || {
                (name.as_str(), 0)
                    .to_socket_addrs()
                    .map(|iter| iter.collect::<Vec<_>>())
            })
            .await?
            .map_err(RoutingError::Dns)?;
            if addresses.is_empty() {
                return Err(RoutingError::Dns(io::Error::new(
                    io::ErrorKind::NotFound,
                    "域名未返回地址",
                ))
                .into());
            }
            if !allow_private && addresses.iter().any(|address| !public_ip(address.ip())) {
                return Err(policy_error().into());
            }
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

pub fn client(timeout: Duration, allow_private: bool) -> Result<Client, reqwest::Error> {
    let builder = Client::builder()
        .no_proxy()
        .timeout(timeout)
        .user_agent(concat!("ue-weave/", env!("CARGO_PKG_VERSION")))
        .redirect(redirect_policy(allow_private))
        .dns_resolver(Arc::new(CheckedResolver { allow_private }));
    builder.build()
}

fn redirect_policy(allow_private: bool) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("重定向次数达到 10 次");
        }
        match check_url(attempt.url(), allow_private) {
            Ok(()) => attempt.follow(),
            Err(error) => attempt.error(error),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirect_to_loopback_stops_before_second_request() {
        use std::io::{Read, Write};
        let source = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let source_addr = source.local_addr().unwrap();
        let target_addr = target.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = source.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let _ = stream.read(&mut [0; 2048]);
            write!(stream, "HTTP/1.1 302 Found\r\nLocation: http://{target_addr}/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .resolve("public.example", source_addr)
            .redirect(redirect_policy(false))
            .build()
            .unwrap();
        let result = crate::network::fetch(
            &client,
            &format!("http://public.example:{}/", source_addr.port()),
            crate::network::Policy::default(),
            Some(1024),
            &crate::log::Logger::new(false),
        );
        assert_eq!(
            result.err().unwrap().kind,
            crate::error::NetworkKind::NetworkPolicy
        );
        server.join().unwrap();
        assert_eq!(
            target.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn filters_special_networks_and_normalized_literal_hosts() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "192.168.2.1",
            "100.64.0.1",
            "192.0.2.1",
            "198.18.0.1",
            "224.1.1.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "2001:db8::1",
            "2002:7f00::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "2001:4860:4860::8888",
            "2606:4700:4700::1111",
        ] {
            assert!(public_ip(ip.parse().unwrap()), "{ip}");
        }
        for url in [
            "http://127.1",
            "http://2130706433",
            "http://localhost.",
            "http://[::1]",
            "http://0x7f000001",
        ] {
            assert!(check_url(&Url::parse(url).unwrap(), false).is_err());
            assert!(check_url(&Url::parse(url).unwrap(), true).is_ok());
        }
    }
}
