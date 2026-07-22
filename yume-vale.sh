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
        # Regenerate cert if missing or older than 7 days
        if [ ! -f certs/server.pem ] || [ "$(find certs/server.pem -mtime +7 2>/dev/null)" ]; then
            echo "=== Generating WebTransport dev certificate ==="
            cargo run -p tools -- generate-cert
        fi
        # Build server with Homebrew cargo
        cargo build -p server
        # Start server in background
        pkill -f "target/debug/server" 2>/dev/null || true
        sleep 1
        ./target/debug/server &
        SERVER_PID=$!
        trap 'kill "$SERVER_PID" 2>/dev/null; kill %1 2>/dev/null; pkill -f "trunk" 2>/dev/null || true' EXIT INT TERM
        sleep 2
        # Serve wasm client with rustup toolchain (trunk must run from the crate dir)
        echo "=== Starting trunk dev server ==="
        cd "$ROOT/apps/client"
        env PATH="$HOME/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin:$PATH" \
            trunk serve --address 127.0.0.1 --port 8080 --open
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
  clean-build       Delete the target/ build artifacts (frees disk space)
  help              Show this help
EOF
        ;;
esac
