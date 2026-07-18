use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use axum::http::header;
use futures_util::StreamExt;
use reqwest::redirect;
use url::{Host, Url};

const MAX_REDIRECTS: usize = 4;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// HTTP fetcher for model-chosen URLs. Every hop (including redirects) is
/// resolved first and rejected if any address points inside the private
/// network, then the connection is pinned to the vetted addresses so a
/// second DNS answer cannot redirect it.
pub struct SafeFetcher {
    timeout: Duration,
    max_bytes: usize,
}

#[derive(Debug)]
pub struct FetchedBody {
    pub final_url: Url,
    pub content_type: String,
    pub body: String,
    pub truncated: bool,
}

impl SafeFetcher {
    pub fn new(timeout: Duration, max_bytes: usize) -> Self {
        Self { timeout, max_bytes }
    }

    pub async fn fetch(&self, url: &str) -> Result<FetchedBody> {
        tokio::time::timeout(self.timeout, self.fetch_inner(url))
            .await
            .map_err(|_| anyhow!("request timed out after {}s", self.timeout.as_secs()))?
    }

    async fn fetch_inner(&self, url: &str) -> Result<FetchedBody> {
        let mut url = validate_url(url)?;
        for _ in 0..=MAX_REDIRECTS {
            let response = self.request(&url).await?;
            if response.status().is_redirection() {
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .context("redirect without a Location header")?;
                url = validate_url(url.join(location)?.as_str())?;
                continue;
            }
            if !response.status().is_success() {
                bail!("HTTP {}", response.status());
            }

            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("text/html")
                .to_ascii_lowercase();
            if !is_text_content_type(&content_type) {
                bail!("unsupported content type '{content_type}'");
            }
            let mut body = Vec::with_capacity(8 * 1024);
            let mut truncated = false;
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.context("failed to read response body")?;
                let remaining = self.max_bytes - body.len();
                if chunk.len() >= remaining {
                    body.extend_from_slice(&chunk[..remaining]);
                    truncated = true;
                    break;
                }
                body.extend_from_slice(&chunk);
            }
            return Ok(FetchedBody {
                final_url: url,
                content_type,
                body: String::from_utf8_lossy(&body).into_owned(),
                truncated,
            });
        }
        bail!("too many redirects")
    }

    async fn request(&self, url: &Url) -> Result<reqwest::Response> {
        let host = url.host().context("URL has no host")?.to_owned();
        let port = url.port_or_known_default().unwrap_or(443);
        let mut builder = reqwest::Client::builder()
            .redirect(redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(self.timeout)
            .user_agent(USER_AGENT);
        match host {
            Host::Ipv4(ip) => check_ip(IpAddr::V4(ip))?,
            Host::Ipv6(ip) => check_ip(IpAddr::V6(ip))?,
            Host::Domain(domain) => {
                let addrs: Vec<SocketAddr> = tokio::net::lookup_host((domain.as_str(), port))
                    .await
                    .with_context(|| format!("failed to resolve '{domain}'"))?
                    .collect();
                if addrs.is_empty() {
                    bail!("'{domain}' did not resolve to any address");
                }
                for addr in &addrs {
                    check_ip(addr.ip())?;
                }
                builder = builder.resolve_to_addrs(&domain, &addrs);
            }
        }
        let client = builder.build()?;
        Ok(client.get(url.clone()).send().await?)
    }
}

pub fn validate_url(raw: &str) -> Result<Url> {
    let url = Url::parse(raw.trim()).with_context(|| format!("invalid URL '{raw}'"))?;
    if url.scheme() != "https" {
        bail!("only https URLs are allowed");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("URLs with embedded credentials are not allowed");
    }
    if url.host().is_none() {
        bail!("URL has no host");
    }
    Ok(url)
}

fn check_ip(ip: IpAddr) -> Result<()> {
    if is_forbidden_ip(ip) {
        bail!("address {ip} points inside the private network");
    }
    Ok(())
}

pub fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped().or_else(|| v6.to_ipv4()) {
                return is_forbidden_v4(v4);
            }
            let segments = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
                || (segments[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        }
    }
}

fn is_forbidden_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback()
        || octets[0] == 0 // 0.0.0.0/8 "this network" (also catches ::1 mapped via to_ipv4)
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || (octets[0] == 100 && (64..=127).contains(&octets[1])) // 100.64.0.0/10 CGNAT (Tailscale)
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0) // 192.0.0.0/24 IETF
        || (octets[0] == 198 && (octets[1] & 0xfe) == 18) // 198.18.0.0/15 benchmarking
        || octets[0] >= 240 // 240.0.0.0/4 reserved
}

fn is_text_content_type(content_type: &str) -> bool {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or(content_type)
        .trim();
    essence.starts_with("text/")
        || essence == "application/json"
        || essence == "application/xml"
        || essence.ends_with("+xml")
        || essence.ends_with("+json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_private_and_special_addresses() {
        for forbidden in [
            "127.0.0.1",
            "0.0.0.0",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "100.127.255.254",
            "192.0.0.10",
            "198.18.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "240.0.0.1",
            "::1",
            "::",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "ff02::1",
            "::ffff:10.0.0.1",
            "::ffff:127.0.0.1",
        ] {
            let ip: IpAddr = forbidden.parse().unwrap();
            assert!(is_forbidden_ip(ip), "{forbidden} should be forbidden");
        }
    }

    #[test]
    fn allows_public_addresses() {
        for allowed in [
            "1.1.1.1",
            "8.8.8.8",
            "104.16.0.1",
            "2606:4700::1111",
            "::ffff:1.1.1.1",
        ] {
            let ip: IpAddr = allowed.parse().unwrap();
            assert!(!is_forbidden_ip(ip), "{allowed} should be allowed");
        }
    }

    #[test]
    fn validates_url_scheme_and_shape() {
        assert!(validate_url("https://example.com/page").is_ok());
        assert!(validate_url("http://example.com").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("https://user:pass@example.com").is_err());
        assert!(validate_url("https://user@example.com").is_err());
        assert!(validate_url("not a url").is_err());
        assert!(validate_url("data:text/html,hello").is_err());
    }

    #[tokio::test]
    async fn fetch_rejects_forbidden_hosts_before_connecting() {
        let fetcher = SafeFetcher::new(Duration::from_secs(2), 1024);
        for url in [
            "http://169.254.169.254/latest/meta-data/",
            "http://127.0.0.1:8787/v1/health",
            "http://100.100.1.1/",
            "http://[::1]/",
            "file:///etc/passwd",
        ] {
            let error = fetcher.fetch(url).await.unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("private network") || message.contains("https"),
                "{url} should be blocked, got: {message}"
            );
        }
    }

    #[test]
    fn accepts_text_content_types_only() {
        assert!(is_text_content_type("text/html; charset=utf-8"));
        assert!(is_text_content_type("text/plain"));
        assert!(is_text_content_type("application/json"));
        assert!(is_text_content_type("application/xhtml+xml"));
        assert!(!is_text_content_type("image/png"));
        assert!(!is_text_content_type("application/octet-stream"));
        assert!(!is_text_content_type("application/pdf"));
    }
}
