//! Transport authentication helpers.
//!
//! The TCP and HTTP transports are network-exposed and, unlike the stdio
//! transport, are not implicitly trusted. When an auth token is configured,
//! every TCP connection and HTTP `/rpc` request must present it.

/// Constant-time byte comparison to avoid leaking the token via timing.
///
/// Length is compared first (and short-circuits), which leaks only the
/// token *length* — standard and acceptable for a shared secret.
#[inline]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Verify a presented token against the configured secret in constant time.
#[inline]
pub fn verify_token(configured: &str, presented: &str) -> bool {
    constant_time_eq(configured.as_bytes(), presented.as_bytes())
}

/// True if `host` refers to a loopback interface (or `localhost`).
///
/// Used to decide whether running without an auth token is safe: loopback
/// binds are only reachable from the local machine, non-loopback binds are not.
pub fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreu"));
        assert!(!constant_time_eq(b"secret", b"secr"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn test_verify_token() {
        assert!(verify_token("hunter2", "hunter2"));
        assert!(!verify_token("hunter2", "Hunter2"));
        assert!(!verify_token("hunter2", ""));
    }

    #[test]
    fn test_is_loopback_host() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("LOCALHOST"));
        assert!(!is_loopback_host("0.0.0.0"));
        assert!(!is_loopback_host("::"));
        assert!(!is_loopback_host("192.168.1.10"));
        assert!(!is_loopback_host("example.com"));
    }
}
