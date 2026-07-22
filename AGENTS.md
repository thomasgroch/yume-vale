# Yume Vale — Agent Guide

Multiplayer 3D prototype. Bevy 0.19 + Lightyear 0.28, Rust 2024, server-authoritative.

## Commands (always via ./yume-vale.sh)

- `play` — build server+client together, run both (native)
- `web` — dev cert + server + trunk serve at http://127.0.0.1:8080 (browser)
- `test` / `check` — cargo test / fmt+clippy+test (clippy is `-D warnings`)

Wasm builds: ONLY via `env PATH="$HOME/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin:$PATH" cargo ...` (target wasm32-unknown-unknown). Native builds use plain (Homebrew) cargo.

## Map

- `crates/game_core` — platform-independent rules, no Bevy
- `crates/game_protocol` — messages, channels, replicated components, palette
- `crates/game_client` — ClientPlugin: connection (UDP native / WebTransport+WebSocket wasm), input, camera, visuals
- `crates/game_server` — ServerPlugin: 3 listeners (UDP:5000, WT:5001, WS:5002), spawn/dedup, input application
- `crates/features/player` — Player components, movement, inventory
- `apps/{client,server,tools}` — binaries; tools = dev cert generation

## Invariants (do not break)

- Gameplay code has NO `#[cfg]` platform branches — platform differences live in client connection setup only
- Server assigns `PlayerColor` (round-robin); clients never color by local/remote
- `client_id` unique per client instance (`YUME_CLIENT_ID` env overrides, native only)
- Rebuild server+client TOGETHER after protocol changes (stale pair = InvalidMagic)
- `game_server` test app: ServerPlugins BEFORE ProtocolPlugin
- Never commit `target/`, `certs/`, `dist/`
- `./yume-vale.sh web` regenerates the dev cert when older than 7 days (14-day browser limit)
