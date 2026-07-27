# Yume Vale — Agent Guide

Multiplayer 3D prototype. Bevy 0.19 + Lightyear 0.28, Rust 2024, server-authoritative. Live at https://yume.lab.thomasdev.xyz.

## Commands (always via ./yume-vale.sh)

- `play` — build server+client together, run both (native)
- `web` — dev cert + server + trunk serve at http://127.0.0.1:8080 (browser)
- `test` / `check` — cargo test / fmt+clippy+test (clippy is `-D warnings`)
- `map` — Grasp codebase map (treemap + churn heatmap) served at http://localhost:8765 (viewer cached in ~/.cache/yume-vale/grasp, refetched if > 7 days)

Wasm builds: ONLY via `env PATH="$HOME/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin:$PATH" cargo ...` (target wasm32-unknown-unknown). Native builds use plain (Homebrew) cargo.

## Map

- `crates/game_core` — platform-independent rules, no Bevy (arena/decorations deterministic layouts, constants, inventory, resources)
- `crates/game_protocol` — messages, channels, replicated components (`PlayerPosition` interpolated, `PlayerColor`), palette
- `crates/game_client` — ClientPlugin. Modules: `connection` (UDP native / WT+WS wasm, retry, welcome), `input`, `camera` (orbit+zoom+touch), `visuals` (fox GLB + animation), `arena` + `decorations` (world visuals), `menu` (play flow), `hud` (status/ping/version), `touch` (joystick+jump, auto-detected), `debug` (F3 bevy-inspector-egui)
- `crates/game_server` — ServerPlugin: 3 listeners (UDP:5000, WT:5001, WS:5002), spawn/dedup, input application, Tnua+Avian3d physics; `systems/` split into connection/input/snapshot/setup
- `crates/features/player` — Player components, movement (Tnua scheme), inventory
- `apps/{client,server,tools}` — binaries; tools = dev cert generation. `apps/client` also holds `index.html`, `nginx.conf`, `dist/`
- `deploy/` — k3s manifests (namespace, server, client, ingress, Argo CD app); see docs/deploy.md

## Deploy / CI

- Push to `main` → `.github/workflows/images.yml` builds `Dockerfile.{server,client}` → ghcr.io, then pins `sha-<commit>` into `deploy/1*.yaml`/`2*.yaml` (`[skip ci]` commit) → Argo CD rolls the k3s pods
- Ingress (Traefik + cert-manager): `/` → nginx client (static wasm), `/ws` → server WS:5002; UDP:5000 exposed via LoadBalancer for native clients
- Client wasm gets `YUME_SERVER_WS_URL` baked at Docker build time (empty = derive from page host)

## Invariants (do not break)

- Gameplay code has NO `#[cfg]` platform branches — platform differences live in client connection setup only
- Server assigns `PlayerColor` (round-robin); clients never color by local/remote
- `client_id` unique per client instance (`YUME_CLIENT_ID` env overrides, native only)
- Rebuild server+client TOGETHER after protocol changes (stale pair = InvalidMagic)
- `game_server` test app: ServerPlugins BEFORE ProtocolPlugin; call `app.finish()` before `app.update()`
- Never commit `target/`, `certs/`, `dist/`
- `./yume-vale.sh web` regenerates the dev cert when older than 7 days (14-day browser limit)
