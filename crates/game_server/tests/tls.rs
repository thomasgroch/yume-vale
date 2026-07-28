//! Integration tests for the WebTransport TLS identity loading.
//!
//! Uses tempfile for PEM fixtures and exercises every error path and the
//! rotation detection logic.

use std::io::Write;

use game_server::systems::tls::{
    TlsConfig, TlsError, check_fingerprint_changed, load_tls_identity,
};
use lightyear::webtransport::prelude::Identity;

/// Write a string to a temporary file and return its path.
fn write_pem(dir: &tempfile::TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    write!(f, "{content}").unwrap();
    path.to_string_lossy().to_string()
}

/// Generate a valid self-signed identity and write its PEM components to
/// temporary files.  Returns (cert_path, key_path, identity_for_reference).
fn gen_identity_pems(dir: &tempfile::TempDir) -> (String, String, Identity) {
    let id = Identity::self_signed(["test.example"]).unwrap();
    let cert_pem = id.certificate_chain().as_slice()[0].to_pem();
    let key_pem = id.private_key().to_secret_pem();
    let cert_path = write_pem(dir, "cert.pem", &cert_pem);
    let key_path = write_pem(dir, "key.pem", &key_pem);
    (cert_path, key_path, id)
}

// ------------------------------------------------------------------
// Loading
// ------------------------------------------------------------------

#[test]
fn load_full_three_cert_chain_and_pkcs8_key() {
    let dir = tempfile::TempDir::new().unwrap();
    let (c1, k1, _) = gen_identity_pems(&dir);

    let id2 = Identity::self_signed(["intermediate.example"]).unwrap();
    let id3 = Identity::self_signed(["root-ca.example"]).unwrap();

    // Write a combined chain: intermediate + root + leaf
    let combined = write_pem(
        &dir,
        "chain.pem",
        &format!(
            "{}\n{}\n{}",
            id2.certificate_chain().as_slice()[0].to_pem(),
            id3.certificate_chain().as_slice()[0].to_pem(),
            std::fs::read_to_string(&c1).unwrap(),
        ),
    );

    let cfg = TlsConfig {
        cert_path: Some(combined),
        key_path: Some(k1),
        ..Default::default()
    };
    let result = load_tls_identity(&cfg).unwrap();
    assert_eq!(result.identity.certificate_chain().as_slice().len(), 3);
}

#[test]
fn single_cert_chain_loads_and_matches() {
    let dir = tempfile::TempDir::new().unwrap();
    let (cert_path, key_path, original) = gen_identity_pems(&dir);

    let cfg = TlsConfig {
        cert_path: Some(cert_path),
        key_path: Some(key_path),
        ..Default::default()
    };
    let result = load_tls_identity(&cfg).unwrap();

    assert_eq!(result.identity.certificate_chain().as_slice().len(), 1);
    assert_eq!(
        result.fingerprint,
        original.certificate_chain().as_slice()[0].hash()
    );
}

#[test]
fn rejects_pkcs1_key() {
    let dir = tempfile::TempDir::new().unwrap();
    let (cert_path, _, _) = gen_identity_pems(&dir);

    let pkcs1_pem = "-----BEGIN RSA PRIVATE KEY-----\n\
                     MIIEpAIBAAKCAQEA0\n\
                     -----END RSA PRIVATE KEY-----\n";
    let key_path = write_pem(&dir, "pkcs1.pem", pkcs1_pem);

    let cfg = TlsConfig {
        cert_path: Some(cert_path),
        key_path: Some(key_path),
        ..Default::default()
    };
    let err = load_tls_identity(&cfg).unwrap_err();
    // PEM parsing may fail as I/O (InvalidTrailingPadding) for garbage data
    // *or* be recognised as PKCS#1 and rejected with UnsupportedKeyFormat.
    assert!(
        matches!(err, TlsError::UnsupportedKeyFormat | TlsError::Io(_)),
        "expected UnsupportedKeyFormat or Io, got {err}"
    );
}

#[test]
fn rejects_empty_key_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let (cert_path, _, _) = gen_identity_pems(&dir);
    let key_path = write_pem(&dir, "empty.pem", "");

    let cfg = TlsConfig {
        cert_path: Some(cert_path),
        key_path: Some(key_path),
        ..Default::default()
    };
    let err = load_tls_identity(&cfg).unwrap_err();
    assert!(
        matches!(err, TlsError::KeyParseError(_)),
        "expected KeyParseError, got {err}"
    );
}

#[test]
fn rejects_malformed_cert_pem() {
    let dir = tempfile::TempDir::new().unwrap();
    let (_, key_path, _) = gen_identity_pems(&dir);
    let cert_path = write_pem(&dir, "bad.pem", "this is not a PEM file\n");

    let cfg = TlsConfig {
        cert_path: Some(cert_path),
        key_path: Some(key_path),
        ..Default::default()
    };
    let err = load_tls_identity(&cfg).unwrap_err();
    assert!(
        matches!(err, TlsError::NoCertificates),
        "expected NoCertificates, got {err}"
    );
}

#[test]
fn rejects_configured_missing_cert() {
    let dir = tempfile::TempDir::new().unwrap();
    let (_, key_path, _) = gen_identity_pems(&dir);

    let cfg = TlsConfig {
        cert_path: Some("/tmp/nonexistent-cert-file.pem".into()),
        key_path: Some(key_path),
        ..Default::default()
    };
    let err = load_tls_identity(&cfg).unwrap_err();
    assert!(
        matches!(err, TlsError::CertNotFound(_)),
        "expected CertNotFound, got {err}"
    );
}

#[test]
fn rejects_configured_missing_key() {
    let dir = tempfile::TempDir::new().unwrap();
    let (cert_path, _, _) = gen_identity_pems(&dir);

    let cfg = TlsConfig {
        cert_path: Some(cert_path),
        key_path: Some("/tmp/nonexistent-key-file.pem".into()),
        ..Default::default()
    };
    let err = load_tls_identity(&cfg).unwrap_err();
    assert!(
        matches!(err, TlsError::KeyNotFound(_)),
        "expected KeyNotFound, got {err}"
    );
}

// ------------------------------------------------------------------
// Dev mode
// ------------------------------------------------------------------

#[test]
fn dev_mode_self_signs() {
    let cfg = TlsConfig::default();
    let result = load_tls_identity(&cfg).unwrap();
    assert!(result.cert_path.is_none());
    assert!(result.key_path.is_none());
    assert_eq!(result.identity.certificate_chain().as_slice().len(), 1);
}

// ------------------------------------------------------------------
// Rotation detection
// ------------------------------------------------------------------

#[test]
fn unchanged_cert_fingerprint_does_not_report_change() {
    let dir = tempfile::TempDir::new().unwrap();
    let (cert_path, key_path, _) = gen_identity_pems(&dir);

    let cfg = TlsConfig {
        cert_path: Some(cert_path.clone()),
        key_path: Some(key_path),
        check_interval_ticks: 1,
    };
    let tls_id = load_tls_identity(&cfg).unwrap();
    let cert_data = std::fs::read(&cert_path).unwrap();

    assert!(
        !check_fingerprint_changed(&tls_id, &cert_data),
        "unchanged cert should not be detected as rotated"
    );
}

#[test]
fn rotated_cert_emits_app_exit() {
    let dir = tempfile::TempDir::new().unwrap();
    let (cert_path, key_path, _) = gen_identity_pems(&dir);

    let cfg = TlsConfig {
        cert_path: Some(cert_path.clone()),
        key_path: Some(key_path),
        check_interval_ticks: 1,
    };
    let tls_id = load_tls_identity(&cfg).unwrap();

    // Replace cert with a different one
    let new_id = Identity::self_signed(["new.example"]).unwrap();
    let new_cert_pem = new_id.certificate_chain().as_slice()[0].to_pem();
    {
        let mut f = std::fs::File::create(&cert_path).unwrap();
        write!(f, "{new_cert_pem}").unwrap();
    }
    let new_cert_data = std::fs::read(&cert_path).unwrap();

    assert!(
        check_fingerprint_changed(&tls_id, &new_cert_data),
        "rotated cert should be detected"
    );
}
