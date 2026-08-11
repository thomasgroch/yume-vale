//! Read-only admin API — HTTP + WebSocket observer window into the live game.
//!
//! ## Enabling
//! Set `YUME_ADMIN_USER` and `YUME_ADMIN_PASSWORD_HASH` (an Argon2id PHC hash
//! string — generate one with `cargo xtask tools hash-admin-password`). If
//! either is unset, or the hash doesn't parse, the plugin is a no-op and no
//! port is bound.
//!
//! ## Auth flow
//! `POST /api/admin/v1/login` exchanges username+password for a bearer
//! session token (opaque, server-held, 12h TTL). All other authenticated
//! endpoints take that token — nothing derived from the password is ever
//! sent again after login. `POST /api/admin/v1/logout` revokes it early.
//!
//! ## Endpoints (port 5003 by default)
//! - `GET  /api/admin/v1/health`  — unauthenticated liveness probe
//! - `POST /api/admin/v1/login`   — `{"username","password"}` → `{"token"}`
//! - `POST /api/admin/v1/logout`  — `Authorization: Bearer <token>`; revokes it
//! - `GET  /api/admin/v1/players` — JSON snapshot; `Authorization: Bearer <token>`
//! - `GET  /api/admin/v1/live`    — WebSocket stream; `?token=<token>`
//!
//! ## Live stream event shapes
//! All events are JSON objects with a `"type"` discriminant:
//! - `{"type":"snapshot","players":[...],"tick":N}` — sent once on WS connect
//! - `{"type":"player_joined","player_id":N,"color":N,"x":F,"y":F,"z":F}`
//! - `{"type":"player_left","player_id":N}`
//! - `{"type":"tick","players":[...],"tick":N}` — every ~0.1 s

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use argon2::{Argon2, PasswordVerifier, password_hash::PasswordHash};
use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query as AxumQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use bevy::ecs::system::Query as BevyQuery;
use bevy::prelude::*;
use game_protocol::{PlayerColor, PlayerPosition};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::systems::connection::ClientPlayer;

// ---------------------------------------------------------------------------
// Public data types (serialised to JSON)
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
pub struct AdminPlayer {
    pub player_id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub color: u8,
}

#[derive(Clone, Serialize, Default)]
pub struct AdminSnapshot {
    pub players: Vec<AdminPlayer>,
    pub online: usize,
    pub tick: u64,
    pub uptime_secs: u64,
}

// ---------------------------------------------------------------------------
// WebSocket live events
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AdminEvent {
    Snapshot {
        players: Vec<AdminPlayer>,
        tick: u64,
    },
    PlayerJoined {
        player_id: u64,
        color: u8,
        x: f32,
        y: f32,
        z: f32,
    },
    PlayerLeft {
        player_id: u64,
    },
    Tick {
        players: Vec<AdminPlayer>,
        tick: u64,
    },
}

impl AdminEvent {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Bevy resource — bridges ECS ↔ axum
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct AdminApiState {
    pub snapshot: Arc<RwLock<AdminSnapshot>>,
    pub tx: broadcast::Sender<String>,
    known_ids: HashSet<u64>,
    tick: u64,
    started: std::time::Instant,
}

impl AdminApiState {
    fn new(tx: broadcast::Sender<String>) -> Self {
        Self {
            snapshot: Arc::new(RwLock::new(AdminSnapshot::default())),
            tx,
            known_ids: HashSet::new(),
            tick: 0,
            started: std::time::Instant::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Bevy plugin
// ---------------------------------------------------------------------------

pub struct AdminApiPlugin {
    pub port: u16,
}

impl Default for AdminApiPlugin {
    fn default() -> Self {
        Self { port: 5003 }
    }
}

impl Plugin for AdminApiPlugin {
    fn build(&self, app: &mut App) {
        let admin_user = std::env::var("YUME_ADMIN_USER").unwrap_or_default();
        let admin_password_hash = std::env::var("YUME_ADMIN_PASSWORD_HASH").unwrap_or_default();
        if admin_user.is_empty() || admin_password_hash.is_empty() {
            tracing::warn!(
                "YUME_ADMIN_USER/YUME_ADMIN_PASSWORD_HASH not set — admin API disabled (port {} not bound)",
                self.port
            );
            return;
        }
        // Fail fast on a malformed hash rather than silently accepting a
        // config that can never let anyone log in.
        if PasswordHash::new(&admin_password_hash).is_err() {
            tracing::error!(
                "YUME_ADMIN_PASSWORD_HASH is not a valid Argon2 PHC hash — admin API disabled. \
                 Generate one with `cargo xtask tools hash-admin-password`."
            );
            return;
        }

        let (tx, _) = broadcast::channel::<String>(512);
        let state = AdminApiState::new(tx.clone());
        let snapshot = Arc::clone(&state.snapshot);
        let port = self.port;

        std::thread::Builder::new()
            .name("yume-admin-api".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("admin api tokio runtime");
                rt.block_on(run_axum(
                    port,
                    admin_user,
                    admin_password_hash,
                    snapshot,
                    tx,
                ));
            })
            .expect("spawn admin-api thread");

        app.insert_resource(state);
        app.add_systems(FixedUpdate, sync_admin_snapshot);
    }
}

// ---------------------------------------------------------------------------
// Bevy system — runs every tick, pushes diffs to WS subscribers
// ---------------------------------------------------------------------------

/// Broadcast a position tick every N server ticks (~0.1 s at 60 Hz).
const TICK_BROADCAST_INTERVAL: u64 = 6;

pub fn sync_admin_snapshot(
    mut api: ResMut<AdminApiState>,
    clients: BevyQuery<&ClientPlayer>,
    player_data: BevyQuery<(&PlayerPosition, &PlayerColor)>,
) {
    api.tick += 1;
    let tick = api.tick;
    let uptime_secs = api.started.elapsed().as_secs();

    let current: Vec<AdminPlayer> = clients
        .iter()
        .filter_map(|cp| {
            player_data
                .get(cp.player_entity)
                .ok()
                .map(|(pos, col)| AdminPlayer {
                    player_id: cp.player_id.get(),
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                    color: col.0,
                })
        })
        .collect();

    let current_ids: HashSet<u64> = current.iter().map(|p| p.player_id).collect();

    // Detect joins
    for p in &current {
        if !api.known_ids.contains(&p.player_id) {
            let _ = api.tx.send(
                AdminEvent::PlayerJoined {
                    player_id: p.player_id,
                    color: p.color,
                    x: p.x,
                    y: p.y,
                    z: p.z,
                }
                .to_json(),
            );
        }
    }
    // Detect leaves
    for &id in &api.known_ids {
        if !current_ids.contains(&id) {
            let _ = api
                .tx
                .send(AdminEvent::PlayerLeft { player_id: id }.to_json());
        }
    }
    api.known_ids = current_ids;

    // Periodic position broadcast
    if tick % TICK_BROADCAST_INTERVAL == 0 {
        let _ = api.tx.send(
            AdminEvent::Tick {
                players: current.clone(),
                tick,
            }
            .to_json(),
        );
    }

    // Update REST snapshot
    if let Ok(mut snap) = api.snapshot.write() {
        snap.online = current.len();
        snap.players = current;
        snap.tick = tick;
        snap.uptime_secs = uptime_secs;
    }
}

// ---------------------------------------------------------------------------
// Credential check (pure — testable without spinning up axum)
// ---------------------------------------------------------------------------

/// Verify a plaintext password against a stored Argon2id PHC hash string.
/// Returns `false` on any parse or mismatch error — a malformed stored hash
/// must never be treated as "any password works".
fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Checks a login attempt against the single configured admin account.
///
/// Always runs the (deliberately slow) Argon2 verification, even when the
/// username is already wrong, so response timing can't distinguish "bad
/// username" from "bad password".
fn check_credentials(
    configured_user: &str,
    configured_hash: &str,
    given_user: &str,
    given_password: &str,
) -> bool {
    let password_ok = verify_password(given_password, configured_hash);
    let user_ok = given_user == configured_user;
    password_ok && user_ok
}

/// A cryptographically random 256-bit session token, hex-encoded.
fn generate_session_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Session store
// ---------------------------------------------------------------------------

/// How long a login session stays valid before requiring a fresh login.
const SESSION_TTL: Duration = Duration::from_secs(60 * 60 * 12);

/// In-memory bearer-session store: login mints a token here, every other
/// authenticated request just checks presence + expiry — the password is
/// never needed again after login.
#[derive(Default)]
struct SessionStore {
    sessions: RwLock<HashMap<String, Instant>>,
}

impl SessionStore {
    /// Mint and store a new session token, sweeping expired entries first
    /// so the map doesn't grow unbounded over a long-running server.
    fn issue(&self) -> String {
        let token = generate_session_token();
        let now = Instant::now();
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.retain(|_, exp| *exp > now);
            sessions.insert(token.clone(), now + SESSION_TTL);
        }
        token
    }

    fn is_valid(&self, token: &str) -> bool {
        let now = Instant::now();
        self.sessions
            .read()
            .map(|m| m.get(token).is_some_and(|exp| *exp > now))
            .unwrap_or(false)
    }

    fn revoke(&self, token: &str) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.remove(token);
        }
    }

    #[cfg(test)]
    fn insert_for_test(&self, token: &str, expires_at: Instant) {
        self.sessions
            .write()
            .unwrap()
            .insert(token.to_string(), expires_at);
    }
}

// ---------------------------------------------------------------------------
// axum server
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AxumState {
    snapshot: Arc<RwLock<AdminSnapshot>>,
    tx: broadcast::Sender<String>,
    admin_user: Arc<str>,
    admin_password_hash: Arc<str>,
    sessions: Arc<SessionStore>,
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
}

async fn login(State(s): State<AxumState>, Json(body): Json<LoginRequest>) -> Response {
    let ok = check_credentials(
        &s.admin_user,
        &s.admin_password_hash,
        &body.username,
        &body.password,
    );
    if !ok {
        return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response();
    }
    let token = s.sessions.issue();
    Json(LoginResponse { token }).into_response()
}

async fn logout(State(s): State<AxumState>, headers: HeaderMap) -> StatusCode {
    if let Some(token) = bearer_token(&headers) {
        s.sessions.revoke(token);
    }
    StatusCode::NO_CONTENT
}

async fn get_players(State(s): State<AxumState>, headers: HeaderMap) -> Response {
    let authorized = bearer_token(&headers).is_some_and(|t| s.sessions.is_valid(t));
    if !authorized {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    let snap = s.snapshot.read().unwrap().clone();
    Json(snap).into_response()
}

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
}

async fn live_ws(
    State(s): State<AxumState>,
    AxumQuery(q): AxumQuery<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let ok = q.token.as_deref().is_some_and(|t| s.sessions.is_valid(t));
    if !ok {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    ws.on_upgrade(move |socket| handle_ws(socket, s))
}

async fn handle_ws(mut socket: WebSocket, s: AxumState) {
    // Initial full snapshot
    {
        let snap = s.snapshot.read().unwrap().clone();
        let event = AdminEvent::Snapshot {
            players: snap.players,
            tick: snap.tick,
        };
        let _ = socket.send(Message::Text(event.to_json().into())).await;
    }

    let mut rx = s.tx.subscribe();

    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(json) => {
                    if socket.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(d))) => { let _ = socket.send(Message::Pong(d)).await; }
                _ => {}
            },
        }
    }
}

async fn run_axum(
    port: u16,
    admin_user: String,
    admin_password_hash: String,
    snapshot: Arc<RwLock<AdminSnapshot>>,
    tx: broadcast::Sender<String>,
) {
    let state = AxumState {
        snapshot,
        tx,
        admin_user: admin_user.into(),
        admin_password_hash: admin_password_hash.into(),
        sessions: Arc::new(SessionStore::default()),
    };

    let app = Router::new()
        .route("/api/admin/v1/health", get(health))
        .route("/api/admin/v1/login", post(login))
        .route("/api/admin/v1/logout", post(logout))
        .route("/api/admin/v1/players", get(get_players))
        .route("/api/admin/v1/live", get(live_ws))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind admin API port");
    tracing::info!("admin API listening on {addr}");
    axum::serve(listener, app)
        .await
        .expect("admin API serve error");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_for_test(password: &str) -> String {
        use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .expect("hash password")
            .to_string()
    }

    #[test]
    fn verify_password_accepts_correct_password() {
        let hash = hash_for_test("correct horse battery staple");
        assert!(verify_password("correct horse battery staple", &hash));
    }

    #[test]
    fn verify_password_rejects_wrong_password() {
        let hash = hash_for_test("correct horse battery staple");
        assert!(!verify_password("wrong password", &hash));
    }

    #[test]
    fn verify_password_rejects_malformed_hash() {
        assert!(!verify_password("anything", "not-a-valid-phc-hash"));
    }

    #[test]
    fn check_credentials_requires_both_user_and_password_match() {
        let hash = hash_for_test("hunter2");

        assert!(check_credentials("admin", &hash, "admin", "hunter2"));
        assert!(!check_credentials("admin", &hash, "admin", "wrong"));
        assert!(!check_credentials(
            "admin",
            &hash,
            "someone-else",
            "hunter2"
        ));
        assert!(!check_credentials("admin", &hash, "someone-else", "wrong"));
    }

    #[test]
    fn generate_session_token_is_64_hex_chars_and_unique() {
        let a = generate_session_token();
        let b = generate_session_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "two tokens should not collide");
    }

    #[test]
    fn session_store_issued_token_is_valid_until_revoked() {
        let store = SessionStore::default();
        let token = store.issue();

        assert!(store.is_valid(&token));
        store.revoke(&token);
        assert!(!store.is_valid(&token));
    }

    #[test]
    fn session_store_rejects_unknown_token() {
        let store = SessionStore::default();
        assert!(!store.is_valid("never-issued"));
    }

    #[test]
    fn session_store_rejects_expired_token() {
        let store = SessionStore::default();
        store.insert_for_test("stale", Instant::now() - Duration::from_secs(1));
        assert!(!store.is_valid("stale"));
    }

    #[test]
    fn session_store_issue_sweeps_expired_entries() {
        let store = SessionStore::default();
        store.insert_for_test("stale", Instant::now() - Duration::from_secs(1));
        store.issue();
        assert_eq!(
            store.sessions.read().unwrap().len(),
            1,
            "stale entry should be swept"
        );
    }
}
