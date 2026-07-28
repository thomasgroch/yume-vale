//! TLS identity loading for WebTransport.
//!
//! Production mode loads a certificate chain (all PEM certificates in order) plus a
//! PKCS#8-only private key from configured file paths.  Dev mode (no paths) generates
//! a self-signed identity at load time.
//!
//! A periodic system checks the on-disk certificate for rotation — if the SHA-256
//! fingerprint of the end-entity cert changes the server emits `AppExit::Success`
//! so that a Kubernetes pod can gracefully restart and pick up the new identity.

use aeronet_webtransport::wtransport::tls::{
    Certificate, CertificateChain, PrivateKey, Sha256Digest,
};
use bevy::prelude::*;
use lightyear::webtransport::prelude::Identity;
use rustls_pemfile::Item;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// TLS configuration for the WebTransport listener.
///
/// When `cert_path` and `key_path` are both `Some`, the server loads production
/// PEM files from those paths.  When either is `None` a self-signed identity
/// is generated (suitable for local development).
///
/// `check_interval_ticks` controls how often the on-disk certificate is
/// re-hashed for rotation detection.  Set to 0 to disable checks.
#[derive(Resource, Clone)]
pub struct TlsConfig {
    /// Path to PEM certificate file (may contain a full chain of certificates).
    pub cert_path: Option<String>,
    /// Path to PEM private key file (PKCS#8 format only).
    pub key_path: Option<String>,
    /// Number of ticks between certificate rotation checks.  Default: 600
    /// (≈20 s at 30 Hz).  0 disables rotation checks entirely.
    pub check_interval_ticks: u32,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: None,
            key_path: None,
            check_interval_ticks: 600,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Typed errors that can occur while loading a WebTransport TLS identity.
#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    /// The certificate file does not exist on disk.
    #[error("certificate file not found: {0}")]
    CertNotFound(String),

    /// The private key file does not exist on disk.
    #[error("private key file not found: {0}")]
    KeyNotFound(String),

    /// The PEM file contained no certificate sections.
    #[error("no certificates found in PEM file")]
    NoCertificates,

    /// A certificate at the given index could not be parsed.
    #[error("failed to parse certificate #{index}: {detail}")]
    CertParseError {
        /// Position of the certificate in the PEM chain (0-based).
        index: usize,
        /// Human-readable parse failure description.
        detail: String,
    },

    /// The private key section could not be parsed.
    #[error("failed to parse private key: {0}")]
    KeyParseError(String),

    /// The key is not PKCS#8 (only PKCS#8 is accepted for production).
    #[error("unsupported key format — only PKCS#8 is accepted")]
    UnsupportedKeyFormat,

    /// Generic I/O error while reading a PEM file.
    #[error("I/O error: {0}")]
    Io(String),
}

// ---------------------------------------------------------------------------
// Identity resource
// ---------------------------------------------------------------------------

/// Loaded TLS identity together with metadata for rotation detection.
///
/// Inserted as a Bevy resource so that `setup_server` can borrow the
/// [`Identity`] for the `WebTransportServerIo` without re-loading from disk.
#[derive(Debug, Resource)]
pub struct TlsIdentity {
    /// The parsed WebTransport identity (certificate chain + private key).
    pub identity: Identity,
    /// SHA-256 fingerprint of the end-entity (first) certificate.
    pub fingerprint: Sha256Digest,
    /// Path to the certificate file (re-used by the rotation checker).
    pub cert_path: Option<String>,
    /// Path to the private key file (re-used by the rotation checker).
    pub key_path: Option<String>,
    /// Accumulated tick counter for rotation checks.
    pub tick_counter: u32,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Load the WebTransport TLS identity from the given configuration.
///
/// ## Production mode
///
/// When both `cert_path` and `key_path` are `Some`:
/// 1. Read the certificate PEM file — all certificates in the file are loaded
///    in order to form the full chain (end-entity first, then intermediates).
/// 2. Read the key PEM file and extract the FIRST private key section.
/// 3. Accept **only** PKCS#8 keys (reject PKCS#1 RSA and SEC1 EC keys).
/// 4. Return a `TlsIdentity` containing the `Identity`, the SHA-256
///    fingerprint of the end-entity cert, and the file paths for rotation
///    checking.
///
/// ## Dev mode
///
/// When either path is missing, generate a self-signed identity via
/// `Identity::self_signed` (ECDSA P-256, 14-day validity, SANs for
/// localhost / 127.0.0.1 / ::1).
pub fn load_tls_identity(config: &TlsConfig) -> Result<TlsIdentity, TlsError> {
    match (&config.cert_path, &config.key_path) {
        (Some(cert_path), Some(key_path)) => load_production_identity(cert_path, key_path),
        _ => {
            let identity = Identity::self_signed(["localhost", "127.0.0.1", "::1"])
                .expect("self-signed WT identity");
            let fingerprint = identity.certificate_chain().as_slice()[0].hash();
            Ok(TlsIdentity {
                identity,
                fingerprint,
                cert_path: None,
                key_path: None,
                tick_counter: 0,
            })
        }
    }
}

/// Internal: parse production PEM files.
fn load_production_identity(cert_path: &str, key_path: &str) -> Result<TlsIdentity, TlsError> {
    let cert_data = std::fs::read(cert_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TlsError::CertNotFound(cert_path.to_owned())
        } else {
            TlsError::Io(e.to_string())
        }
    })?;

    let key_data = std::fs::read(key_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            TlsError::KeyNotFound(key_path.to_owned())
        } else {
            TlsError::Io(e.to_string())
        }
    })?;

    // ---- certificate chain (all PEM certificates in order) ----
    let mut cert_reader = std::io::Cursor::new(&cert_data);
    let cert_results: Vec<Result<Certificate, TlsError>> = rustls_pemfile::certs(&mut cert_reader)
        .enumerate()
        .map(|(idx, result)| {
            let der = result.map_err(|e| TlsError::Io(e.to_string()))?;
            Certificate::from_der(der.to_vec()).map_err(|e| TlsError::CertParseError {
                index: idx,
                detail: e.to_string(),
            })
        })
        .collect();

    if cert_results.is_empty() {
        return Err(TlsError::NoCertificates);
    }
    let certs: Vec<Certificate> = cert_results.into_iter().collect::<Result<_, _>>()?;
    let fingerprint = certs[0].hash();
    let chain = CertificateChain::new(certs);

    // ---- private key (PKCS#8 only) ----
    let pkcs8_der = extract_pkcs8_key(&key_data)?;
    let private_key = PrivateKey::from_der_pkcs8(pkcs8_der);

    let identity = Identity::new(chain, private_key);

    Ok(TlsIdentity {
        identity,
        fingerprint,
        cert_path: Some(cert_path.to_owned()),
        key_path: Some(key_path.to_owned()),
        tick_counter: 0,
    })
}

/// Read a PEM file and return the DER bytes of the **first** PKCS#8 private
/// key.  Rejects PKCS#1 (RSA) and SEC1 (EC) keys with `UnsupportedKeyFormat`.
fn extract_pkcs8_key(key_data: &[u8]) -> Result<Vec<u8>, TlsError> {
    let mut reader = std::io::Cursor::new(key_data);
    for item in rustls_pemfile::read_all(&mut reader) {
        let item = item.map_err(|e| TlsError::Io(e.to_string()))?;
        match item {
            Item::Pkcs8Key(key) => return Ok(key.secret_pkcs8_der().to_vec()),
            Item::Pkcs1Key(_) | Item::Sec1Key(_) => {
                return Err(TlsError::UnsupportedKeyFormat);
            }
            _ => continue, // skip certificates, CRLs, etc.
        }
    }
    Err(TlsError::KeyParseError(
        "no private key section found".into(),
    ))
}

// ---------------------------------------------------------------------------
// Startup system
// ---------------------------------------------------------------------------

/// Startup system that loads the TLS identity and inserts it as a resource.
///
/// Panics (during development) if production paths are configured and loading
/// fails — a misconfigured server should not silently fall back to self-signed.
pub fn load_tls_identity_system(world: &mut World) {
    let config = world
        .get_resource::<TlsConfig>()
        .cloned()
        .unwrap_or_default();

    match load_tls_identity(&config) {
        Ok(tls_id) => {
            let mode = if tls_id.cert_path.is_some() {
                "production"
            } else {
                "dev (self-signed)"
            };
            tracing::info!(mode, "WebTransport TLS identity loaded");
            world.insert_resource(tls_id);
        }
        Err(e) => {
            tracing::error!("failed to load WebTransport TLS identity: {e}");
            // In dev mode (no paths configured) this should never happen
            // because we fall through to self-signed; in production it's a
            // fatal startup error.
            panic!("WebTransport TLS identity load failed: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Rotation check
// ---------------------------------------------------------------------------

/// Pure check: given the current identity and cert file data, returns `true`
/// if the certificate has been rotated (its fingerprint differs).
///
/// Exposed for testing — the Bevy system below calls this internally.
pub fn check_fingerprint_changed(identity: &TlsIdentity, cert_data: &[u8]) -> bool {
    let new_fingerprint = match rustls_pemfile::certs(&mut std::io::Cursor::new(cert_data)).next() {
        Some(Ok(der)) => match Certificate::from_der(der.to_vec()) {
            Ok(c) => c.hash(),
            Err(_) => return false,
        },
        _ => return false,
    };
    new_fingerprint != identity.fingerprint
}

/// Periodic system that checks whether the on-disk certificate has changed.
///
/// Every `check_interval_ticks` ticks the system re-reads the cert file and
/// tests for rotation via [`check_fingerprint_changed`].  If the fingerprint
/// differs from the one loaded at startup, the system emits
/// `AppExit::Success` so that a Kubernetes pod can gracefully restart and
/// pick up the new identity.
///
/// When no cert path is configured (dev mode) or `check_interval_ticks` is 0
/// the system is a no-op.
pub fn check_cert_rotation(
    mut identity: ResMut<TlsIdentity>,
    config: Res<TlsConfig>,
    mut exit: MessageWriter<AppExit>,
) {
    let interval = config.check_interval_ticks;
    if interval == 0 {
        return;
    }

    identity.tick_counter += 1;
    if identity.tick_counter < interval {
        return;
    }
    identity.tick_counter = 0;

    let Some(ref cert_path) = identity.cert_path else {
        return; // dev mode — no files to watch
    };

    let cert_data = match std::fs::read(cert_path) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(path = %cert_path, error = %e, "cert rotation check: cannot read");
            return;
        }
    };

    if check_fingerprint_changed(&identity, &cert_data) {
        tracing::info!("certificate fingerprint changed — exiting for graceful restart");
        exit.write(AppExit::Success);
    }
}
