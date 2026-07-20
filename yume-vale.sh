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
    world)
        cd "$ROOT"
        mkdir -p assets
        cargo run -p tools -- generate-world --output assets/world.ron
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
    help|*)
        cat <<'EOF'
Yume Vale helper script

Usage: ./yume-vale.sh <command>

Commands:
  build          Build the workspace
  test           Run workspace tests
  check          Run fmt, clippy, and tests
  server         Run the server binary
  client         Run the client binary
  play           Build both, then run server (background) + client (foreground) in one window
  tools <args>   Run the tools binary with extra args
  world          Generate assets/world.ron
  clean-build    Delete the target/ build artifacts (frees disk space)
  help           Show this help
EOF
        ;;
esac
