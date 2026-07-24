#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
CMD="${1:-help}"

case "$CMD" in
    build)
        cd "$ROOT"
        cargo build --workspace "$@"
        ;;
    test)
        cd "$ROOT"
        cargo test --workspace
        ;;
    check)
        cd "$ROOT"
        cargo fmt --all -- --check
        cargo clippy --workspace --all-targets -- -D warnings
        cargo test --workspace
        ;;
    server)
        cd "$ROOT"
        pkill -f "target/debug/server" 2>/dev/null || true
        sleep 1
        cargo run -p server
        ;;
    client)
        cd "$ROOT"
        cargo run -p client
        ;;
    play)
        cd "$ROOT"
        cargo build -p server -p client
        pkill -f "target/debug/server" 2>/dev/null || true
        sleep 1
        ./target/debug/server &
        SERVER_PID=$!
        trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT INT TERM
        sleep 2
        ./target/debug/client
        ;;
    tools)
        shift
        cd "$ROOT"
        cargo run -p tools -- "$@"
        ;;
    generate-cert)
        cd "$ROOT"
        cargo run -p tools -- generate-cert
        ;;
    clean-build)
        TARGET="$ROOT/target"
        if [ -d "$TARGET" ]; then
            SIZE="$(du -sh "$TARGET" | cut -f1)"
            echo "target/ directory size: $SIZE"
            read -r -p "Delete entire target/ (Cargo cache + debug + release artifacts)? [y/N] " REPLY
            if [[ "$REPLY" =~ ^[Yy]$ ]]; then
                echo "Deleting $TARGET ..."
                rm -rf "$TARGET"
                echo "Done. Freed ~$SIZE."
                echo "Next build will recompile from scratch."
            else
                echo "Skipped."
            fi
        else
            echo "Nothing to clean — target/ does not exist."
        fi
        ;;
    web)
        cd "$ROOT"
        # Kill stale processes from previous runs (avoids "address already in use")
        pkill -f "target/debug/server" 2>/dev/null || true
        pkill -f "trunk serve --address 127.0.0.1 --port 8080" 2>/dev/null || true
        sleep 1
        # Placeholder on :8080 so the URL responds immediately during the (long) builds;
        # the page auto-refreshes until trunk takes over.
        PLACEHOLDER_DIR="$(mktemp -d)"
        cat > "$PLACEHOLDER_DIR/index.html" <<'HTML'
<!doctype html><meta charset="utf-8"><meta http-equiv="refresh" content="3">
<title>Yume Vale — compilando…</title>
<body style="font-family:monospace;background:#ffdbe9;color:#7a3b4f;display:grid;place-items:center;height:100vh;margin:0">
<div style="text-align:center"><h1>Yume Vale</h1><p>Compilando… esta página recarrega sozinha quando o jogo estiver pronto.</p></div>
HTML
        python3 -m http.server 8080 --bind 127.0.0.1 --directory "$PLACEHOLDER_DIR" >/dev/null 2>&1 &
        PLACEHOLDER_PID=$!
        trap 'kill "$PLACEHOLDER_PID" 2>/dev/null; kill "$SERVER_PID" 2>/dev/null; pkill -f "trunk serve --address 127.0.0.1 --port 8080" 2>/dev/null || true' EXIT INT TERM
        # Regenerate cert if missing or older than 7 days
        if [ ! -f certs/server.pem ] || [ "$(find certs/server.pem -mtime +7 2>/dev/null)" ]; then
            echo "=== [1/4] Generating WebTransport dev certificate ==="
            cargo run -p tools -- generate-cert
        fi
        echo "=== [2/4] Building server (native) ==="
        cargo build -p server
        ./target/debug/server &
        SERVER_PID=$!
        sleep 2
        echo "=== [3/4] Building wasm client (trunk) — primeira vez demora ==="
        cd "$ROOT/apps/client"
        env PATH="$HOME/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin:$PATH" \
            trunk build
        # Build done: swap placeholder for trunk (binds in ~1s) and open the browser
        kill "$PLACEHOLDER_PID" 2>/dev/null || true
        echo "=== [4/4] Serving at http://127.0.0.1:8080 — a aba abre sozinha ==="
        env PATH="$HOME/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin:$PATH" \
            trunk serve --address 127.0.0.1 --port 8080 --open
        ;;
    map)
        # Grasp codebase viewer: single cached index.html served locally (refetch if > 7 days old)
        CACHE="$HOME/.cache/yume-vale/grasp"
        PORT=8765
        if [ ! -f "$CACHE/index.html" ] || [ "$(find "$CACHE/index.html" -mtime +7 2>/dev/null)" ]; then
            echo "=== Downloading Grasp viewer ==="
            mkdir -p "$CACHE"
            curl -fsSL https://raw.githubusercontent.com/ashfordeOU/grasp/main/index.html -o "$CACHE/index.html"
        fi
        pkill -f "http.server $PORT" 2>/dev/null || true
        sleep 1
        cd "$CACHE"
        python3 -m http.server "$PORT" >/dev/null 2>&1 &
        SERVER_PID=$!
        trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT INT TERM
        sleep 1
        open "http://localhost:$PORT/index.html"
        echo "=== Grasp running at http://localhost:$PORT ==="
        echo "In the browser, open folder: $ROOT"
        echo "Press Ctrl+C to stop."
        wait "$SERVER_PID"
        ;;
    help|*)
        cat <<'EOF'
Yume Vale helper script

Usage: ./yume-vale.sh <command>

Commands:
  build             Build the workspace
  test              Run workspace tests
  check             Run fmt, clippy, and tests
  server            Run the server binary
  client            Run the client binary
  play              Build both, then run server (background) + client (foreground) in one window
  tools <args>      Run the tools binary with extra args
  generate-cert     Generate WebTransport dev certificate
  web               Build server + serve wasm client via trunk (cross-play)
  map               Open Grasp codebase map (treemap + churn heatmap) in browser
  clean-build       Delete the target/ build artifacts (frees disk space)
  help              Show this help
EOF
        ;;
esac
