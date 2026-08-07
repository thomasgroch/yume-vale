//! Read-only admin API — HTTP + WebSocket observer window into the live game.
//!
//! ## Enabling
//! Set `YUME_ADMIN_TOKEN` to any non-empty string. If unset the plugin is a
//! no-op and no port is bound.
//!
//! ## Endpoints (port 5003 by default)
//! - `GET  /api/admin/v1/health`  — unauthenticated liveness probe
//! - `GET  /api/admin/v1/players` — JSON snapshot; `Authorization: Bearer <token>`
//! - `GET  /api/admin/v1/live`    — WebSocket stream; `?token=<token>`
//!
//! ## Live stream event shapes
//! All events are JSON objects with a `"type"` discriminant:
//! - `{"type":"snapshot","players":[...],"tick":N}` — sent once on WS connect
//! - `{"type":"player_joined","player_id":N,"color":N,"x":F,"y":F,"z":F}`
//! - `{"type":"player_left","player_id":N}`
//! - `{"type":"tick","players":[...],"tick":N}` — every ~0.1 s

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query as AxumQuery, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
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
        let token = match std::env::var("YUME_ADMIN_TOKEN") {
            Ok(t) if !t.is_empty() => t,
            _ => {
                tracing::warn!(
                    "YUME_ADMIN_TOKEN not set — admin API disabled (port {} not bound)",
                    self.port
                );
                return;
            }
        };

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
                rt.block_on(run_axum(port, token, snapshot, tx));
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
// axum server
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AxumState {
    snapshot: Arc<RwLock<AdminSnapshot>>,
    tx: broadcast::Sender<String>,
    token: Arc<str>,
}

fn bearer_ok(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t == token)
        .unwrap_or(false)
}

async fn health() -> &'static str {
    "ok"
}

async fn get_players(State(s): State<AxumState>, headers: HeaderMap) -> Response {
    if !bearer_ok(&headers, &s.token) {
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
    let ok = q
        .token
        .as_deref()
        .map(|t| t == s.token.as_ref())
        .unwrap_or(false);
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
    token: String,
    snapshot: Arc<RwLock<AdminSnapshot>>,
    tx: broadcast::Sender<String>,
) {
    let state = AxumState {
        snapshot,
        tx,
        token: token.into(),
    };

    let app = Router::new()
        .route("/api/admin/v1/health", get(health))
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
