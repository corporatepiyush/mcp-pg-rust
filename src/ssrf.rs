//! SSRF protection for outbound URL fetches (`import_from_url`).
//!
//! Validates that a user-supplied URL uses an allowed scheme and resolves
//! only to public IP addresses, blocking access to loopback, private,
//! link-local (incl. the cloud metadata endpoint 169.254.169.254), and
//! unique-local ranges.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::errors::MCPError;

/// Return `true` if the IP must NOT be reachable from a user-controlled fetch.
pub const fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => is_blocked_v6(v6),
    }
}

const fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()            // 127.0.0.0/8
        || ip.is_private()      // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local()   // 169.254/16  (incl. 169.254.169.254 metadata)
        || ip.is_unspecified()  // 0.0.0.0
        || ip.is_broadcast()    // 255.255.255.255
        || ip.is_documentation()
        || is_shared_v4(ip) // 100.64/10 carrier-grade NAT
}

/// 100.64.0.0/10 — RFC 6598 shared address space (no stable std helper).
const fn is_shared_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 100 && (o[1] & 0b1100_0000) == 0b0100_0000
}

const fn is_blocked_v6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    // IPv4-mapped (::ffff:a.b.c.d) — apply the v4 rules to the embedded addr.
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_blocked_v4(v4);
    }
    let seg = ip.segments();
    let first = seg[0];
    // fc00::/7 unique local, fe80::/10 link local.
    (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
}

/// Validate a user-supplied import URL and return the resolved, allowed
/// `host:port` authority. Rejects non-http(s) schemes and any host that
/// resolves to a blocked address.
pub async fn validate_import_url(url: &str) -> Result<(), MCPError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| MCPError::InvalidParams(format!("Invalid URL: {e}")))?;

    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(MCPError::InvalidParams(format!(
            "URL scheme '{scheme}' is not allowed; only http and https are permitted"
        )));
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| MCPError::InvalidParams("URL has no host".into()))?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| MCPError::InvalidParams("URL has no port".into()))?;

    // Resolve and ensure every candidate address is public.
    let addrs = tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| MCPError::InvalidParams(format!("Failed to resolve host '{host}': {e}")))?;

    let mut any = false;
    for addr in addrs {
        any = true;
        if is_blocked_ip(addr.ip()) {
            return Err(MCPError::InvalidParams(format!(
                "URL host '{host}' resolves to a blocked (private/loopback/link-local) address"
            )));
        }
    }
    if !any {
        return Err(MCPError::InvalidParams(format!(
            "URL host '{host}' did not resolve to any address"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn ip(s: &str) -> IpAddr {
        IpAddr::from_str(s).unwrap()
    }

    #[test]
    fn test_blocked_v4() {
        assert!(is_blocked_ip(ip("127.0.0.1")));
        assert!(is_blocked_ip(ip("10.0.0.5")));
        assert!(is_blocked_ip(ip("172.16.3.4")));
        assert!(is_blocked_ip(ip("192.168.1.1")));
        assert!(is_blocked_ip(ip("169.254.169.254"))); // cloud metadata
        assert!(is_blocked_ip(ip("0.0.0.0")));
        assert!(is_blocked_ip(ip("100.64.1.1"))); // CGNAT
    }

    #[test]
    fn test_allowed_v4() {
        assert!(!is_blocked_ip(ip("1.1.1.1")));
        assert!(!is_blocked_ip(ip("8.8.8.8")));
        assert!(!is_blocked_ip(ip("93.184.216.34")));
    }

    #[test]
    fn test_blocked_v6() {
        assert!(is_blocked_ip(ip("::1")));
        assert!(is_blocked_ip(ip("::")));
        assert!(is_blocked_ip(ip("fc00::1")));
        assert!(is_blocked_ip(ip("fe80::1")));
        assert!(is_blocked_ip(ip("::ffff:127.0.0.1"))); // mapped loopback
    }

    #[test]
    fn test_allowed_v6() {
        assert!(!is_blocked_ip(ip("2606:4700:4700::1111")));
    }

    #[tokio::test]
    async fn test_validate_rejects_scheme() {
        let err = validate_import_url("file:///etc/passwd").await.unwrap_err();
        assert!(err.to_string().contains("scheme"));
        let err = validate_import_url("ftp://example.com/x")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("scheme"));
    }

    #[tokio::test]
    async fn test_validate_rejects_loopback_literal() {
        let err = validate_import_url("http://127.0.0.1:8080/x")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("blocked"));
    }
}
