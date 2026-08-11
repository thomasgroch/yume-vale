use clap::{Parser, Subcommand};
use rcgen::{CertificateParams, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use time::OffsetDateTime;

#[derive(Parser)]
#[command(name = "yume-tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a self-signed WebTransport dev certificate (max 14-day validity).
    GenerateCert,
    /// Hash an admin panel password for YUME_ADMIN_PASSWORD_HASH.
    ///
    /// Prompts for the password (hidden input, typed twice to confirm) and
    /// prints an Argon2id PHC hash string to stdout — never the plaintext.
    HashAdminPassword,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::GenerateCert => generate_cert(),
        Commands::HashAdminPassword => hash_admin_password(),
    }
}

fn generate_cert() {
    let cert_dir = Path::new("certs");
    fs::create_dir_all(cert_dir).expect("failed to create certs/ directory");

    let cert_pem_path = cert_dir.join("server.pem");
    let key_pem_path = cert_dir.join("key.pem");
    let digest_path = cert_dir.join("digest.txt");

    // Generate ECDSA P-256 key pair
    let key_pair =
        KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("failed to generate key pair");

    // Set validity: now -> now + 13 days (browsers reject >14 day self-signed)
    let now = OffsetDateTime::now_utc();
    let not_after = now
        .checked_add(time::Duration::days(13))
        .expect("validity overflow");

    let mut params = CertificateParams::new(vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ])
    .expect("invalid SANs");

    params.not_before = now;
    params.not_after = not_after;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];

    let cert = params.self_signed(&key_pair).expect("failed to self-sign");

    // Write PEM files
    fs::write(&cert_pem_path, cert.pem()).expect("failed to write cert.pem");
    fs::write(&key_pem_path, key_pair.serialize_pem()).expect("failed to write key.pem");

    // Compute SHA-256 digest of DER-encoded certificate (lowercase hex, no colons)
    let der = cert.der();
    let mut hasher = Sha256::new();
    hasher.update(der);
    let hash = hasher.finalize();
    let digest = hex::encode(hash);

    fs::write(&digest_path, &digest).expect("failed to write digest.txt");

    tracing::info!("Certificate generated!");
    tracing::info!("  cert: {}", cert_pem_path.display());
    tracing::info!("  key:  {}", key_pem_path.display());
    tracing::info!("  sha256 digest: {digest}");
    tracing::info!("  valid until: {}", not_after);
}

fn hash_admin_password() {
    use argon2::Argon2;
    use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};

    let password = rpassword::prompt_password("Admin password: ").expect("failed to read password");
    let confirm =
        rpassword::prompt_password("Confirm password: ").expect("failed to read password");

    if password.is_empty() {
        eprintln!("Password must not be empty.");
        std::process::exit(1);
    }
    if password != confirm {
        eprintln!("Passwords did not match.");
        std::process::exit(1);
    }

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("failed to hash password")
        .to_string();

    println!("{hash}");
    eprintln!("\nSet this as YUME_ADMIN_PASSWORD_HASH (e.g. in the yume-admin secret).");
}
