//! TLS support for PostgreSQL connections.
//!
//! TLS is opt-in via the connection string's `sslmode`. When `sslmode` is
//! `require`, `verify-ca`, `verify-full`, or `prefer`, the pool uses a rustls
//! connector with the system's native root certificates. Otherwise (the
//! default, or `sslmode=disable`/`allow`) connections stay plaintext, matching
//! the previous behavior exactly.

use rustls::ClientConfig;
use tokio_postgres_rustls::MakeRustlsConnect;

/// Return `true` if the connection string opts into TLS via `sslmode`.
pub fn wants_tls(connection_string: &str) -> bool {
    sslmode(connection_string)
        .map(|m| {
            matches!(
                m.as_str(),
                "require" | "verify-ca" | "verify-full" | "prefer"
            )
        })
        .unwrap_or(false)
}

/// Extract the `sslmode` value from a key=value or URL-style connection string.
fn sslmode(connection_string: &str) -> Option<String> {
    // Handle both "key=value ..." and "postgres://...?sslmode=..." forms by
    // scanning for the sslmode token anywhere after a '=' delimiter.
    let lower = connection_string.to_ascii_lowercase();
    let idx = lower.find("sslmode=")?;
    let rest = &lower[idx + "sslmode=".len()..];
    let end = rest.find([' ', '&', '\'']).unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

/// Install the rustls `ring` crypto provider as the process default.
///
/// Idempotent — only the first install in the process wins; later calls are
/// ignored. Call this anywhere a rustls-backed client may be built (Postgres
/// TLS via [`make_connector`], or the data-import HTTP client which uses reqwest
/// with `rustls-no-provider`) so the process never lacks a default
/// `CryptoProvider`. Keeping it in the library — rather than only in the binary's
/// `main` — ensures library consumers get correct TLS too.
pub fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Build a rustls connector loading the OS trust store.
///
/// Installs the ring crypto provider as the process default on first call
/// (idempotent — a second install is ignored).
pub fn make_connector() -> anyhow::Result<MakeRustlsConnect> {
    // Safe to call repeatedly; only the first install wins.
    ensure_crypto_provider();

    let mut roots = rustls::RootCertStore::empty();
    let result = rustls_native_certs::load_native_certs();
    if !result.errors.is_empty() {
        tracing::warn!(
            "Some native root certificates failed to load: {:?}",
            result.errors
        );
    }
    for cert in result.certs {
        // Skip individual malformed certs rather than failing the whole pool.
        let _ = roots.add(cert);
    }
    if roots.is_empty() {
        anyhow::bail!("No native root certificates available for TLS verification");
    }

    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    Ok(MakeRustlsConnect::new(config))
}

/// Build an axum-server rustls config for the HTTP transport from a PEM
/// certificate chain and private key.
///
/// Server-side TLS is opt-in: the HTTP server only calls this when both a
/// `--tls-cert` and `--tls-key` are configured, otherwise it stays plaintext.
/// Uses the process default `ring` crypto provider (installed here on first
/// call), matching the Postgres client and reqwest paths — no aws-lc/cmake.
pub async fn server_config(
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
) -> anyhow::Result<axum_server::tls_rustls::RustlsConfig> {
    // The rustls ServerConfig builder reads the process default CryptoProvider;
    // ensure ring is installed before we build it.
    ensure_crypto_provider();
    axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "failed to load TLS certificate '{}' and key '{}': {e}",
                cert_path.display(),
                key_path.display()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sslmode_url_form() {
        assert_eq!(
            sslmode("postgres://u:p@h/db?sslmode=require"),
            Some("require".to_string())
        );
    }

    #[test]
    fn test_sslmode_kv_form() {
        assert_eq!(
            sslmode("host=localhost sslmode=verify-full dbname=x"),
            Some("verify-full".to_string())
        );
    }

    #[test]
    fn test_wants_tls() {
        assert!(wants_tls("postgres://h/db?sslmode=require"));
        assert!(wants_tls("sslmode=verify-ca"));
        assert!(wants_tls("sslmode=prefer"));
        assert!(!wants_tls("postgres://h/db?sslmode=disable"));
        assert!(!wants_tls("postgres://h/db")); // default: plaintext
        assert!(!wants_tls("host=localhost dbname=x"));
    }
}
