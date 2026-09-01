use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::{redirect::Policy, Client};
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::lookup_host;
use url::{Host, Url};

/// Returns true for addresses that an attacker-controlled URL must never reach.
/// This includes local/private networks, metadata/link-local ranges, and
/// non-routable special-purpose addresses.
pub fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_disallowed_ipv4(ipv4),
        IpAddr::V6(ipv6) => {
            if let Some(mapped) = ipv6.to_ipv4_mapped() {
                return is_disallowed_ipv4(mapped);
            }
            is_disallowed_ipv6(ipv6)
        }
    }
}

fn is_disallowed_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || a == 0
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 240
}

fn is_disallowed_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        || ip.is_multicast()
        || (segments[0] & 0xffc0) == 0xfec0
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
}

/// Applies the URL-level part of the SSRF policy. DNS names are checked again
/// by `PublicDnsResolver` when the HTTP connector actually resolves them.
pub fn validate_public_http_target(raw_url: &str) -> Result<Url, String> {
    let parsed = Url::parse(raw_url).map_err(|_| "Invalid URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only HTTP and HTTPS URLs are allowed".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("URLs containing credentials are not allowed".to_string());
    }

    match parsed.host() {
        Some(Host::Domain(host)) if host.eq_ignore_ascii_case("localhost") => {
            return Err("Localhost URLs are not allowed".to_string());
        }
        Some(Host::Ipv4(ip)) if is_disallowed_ip(IpAddr::V4(ip)) => {
            return Err("Private or local URLs are not allowed".to_string());
        }
        Some(Host::Ipv6(ip)) if is_disallowed_ip(IpAddr::V6(ip)) => {
            return Err("Private or local URLs are not allowed".to_string());
        }
        Some(_) => {}
        None => return Err("URL must include a host".to_string()),
    }

    Ok(parsed)
}

pub fn validate_resolved_addresses(addresses: &[SocketAddr]) -> Result<(), String> {
    if addresses.is_empty() {
        return Err("Host did not resolve to an address".to_string());
    }
    if addresses
        .iter()
        .any(|address| is_disallowed_ip(address.ip()))
    {
        return Err("URL resolves to a private or local address".to_string());
    }
    Ok(())
}

/// Early validation used when storing a URL. Request-time validation remains
/// mandatory because DNS can change after registration.
pub async fn validate_public_http_url(raw_url: &str) -> Result<(), String> {
    let parsed = validate_public_http_target(raw_url)?;
    if !matches!(parsed.host(), Some(Host::Domain(_))) {
        return Ok(());
    }
    let host = parsed.host_str().expect("validated URL has a host");
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "URL must include a valid port".to_string())?;
    let addresses: Vec<SocketAddr> = lookup_host((host, port))
        .await
        .map_err(|_| "Failed to resolve server host".to_string())?
        .collect();
    validate_resolved_addresses(&addresses)
}

#[derive(Debug, Default)]
struct PublicDnsResolver;

impl Resolve for PublicDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addresses: Vec<SocketAddr> = lookup_host((host.as_str(), 0)).await?.collect();
            validate_resolved_addresses(&addresses)
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

/// Builds an HTTP client whose resolver checks every new connection. Redirects
/// and environment proxies are disabled so a validated public target cannot
/// hand the request off to an unvalidated internal destination.
pub fn build_public_http_client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .timeout(timeout)
        .redirect(Policy::none())
        .no_proxy()
        .dns_resolver(Arc::new(PublicDnsResolver))
        .build()
        .map_err(|_| "Failed to create secure HTTP client".to_string())
}
