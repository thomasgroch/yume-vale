use bevy::prelude::*;
use lightyear::prelude::*;

/// Try to load WebTransport Identity from PEM files synchronously.
/// Falls back to `None` if files are missing or malformed.
fn load_wt_identity(
    cert_path: &str,
    key_path: &str,
) -> Option<lightyear::webtransport::prelude::Identity> {
    use aeronet_webtransport::wtransport::tls::*;
    use std::fs;
    use std::io::Cursor;

    let cert_data = fs::read(cert_path).ok()?;
    let key_data = fs::read(key_path).ok()?;

    let cert_der_vec = rustls_pemfile::certs(&mut Cursor::new(&cert_data))
        .next()?
        .ok()?;
    let certificate = Certificate::from_der(cert_der_vec.to_vec()).ok()?;

    let key_result = rustls_pemfile::private_key(&mut Cursor::new(&key_data));
    let key_der = match key_result {
        Ok(Some(k)) => k,
        _ => return None,
    };

    // rcgen generates PKCS#8 keys, so from_der_pkcs8 is correct here
    let private_key = PrivateKey::from_der_pkcs8(key_der.secret_der().to_vec());

    Some(Identity::new(
        CertificateChain::new(vec![certificate]),
        private_key,
    ))
}

/// Spawns the Lightyear server entities (UDP, WebTransport, WebSocket) and starts them.
pub fn setup_server(mut commands: Commands) {
    use lightyear::prelude::server::*;
    use std::net::SocketAddr;

    let config = NetcodeConfig::default()
        .with_protocol_id(game_protocol::PROTOCOL_ID)
        .with_key(game_protocol::PRIVATE_KEY);

    // UDP / Netcode listener (existing native transport)
    tracing::info!("starting UDP server on 127.0.0.1:5000");
    let udp_entity = commands
        .spawn((
            NetcodeServer::new(config.clone()),
            LocalAddr(SocketAddr::from(([127, 0, 0, 1], 5000))),
            ServerUdpIo::default(),
        ))
        .id();
    commands.entity(udp_entity).trigger(|e| Start { entity: e });

    // WebTransport listener (browser clients)
    tracing::info!("starting WebTransport server on 127.0.0.1:5001");
    let wt_identity = load_wt_identity("certs/server.pem", "certs/key.pem").unwrap_or_else(|| {
        tracing::warn!(
            "failed to load WT certs, generating self-signed (client hash pinning will not work)"
        );
        lightyear::webtransport::prelude::Identity::self_signed(["localhost", "127.0.0.1", "::1"])
            .expect("self-signed WT identity")
    });
    let wt_entity = commands
        .spawn((
            NetcodeServer::new(config.clone()),
            LocalAddr(SocketAddr::from(([127, 0, 0, 1], 5001))),
            WebTransportServerIo {
                certificate: wt_identity,
            },
        ))
        .id();
    commands.entity(wt_entity).trigger(|e| Start { entity: e });

    // WebSocket listener (browser clients, fallback)
    tracing::info!("starting WebSocket server on 127.0.0.1:5002");
    let ws_config = aeronet_websocket::server::ServerConfig::builder()
        .with_bind_address(SocketAddr::from(([127, 0, 0, 1], 5002)))
        .with_no_encryption();
    let ws_entity = commands
        .spawn((
            NetcodeServer::new(config),
            LocalAddr(SocketAddr::from(([127, 0, 0, 1], 5002))),
            WebSocketServerIo { config: ws_config },
        ))
        .id();
    commands.entity(ws_entity).trigger(|e| Start { entity: e });
}
