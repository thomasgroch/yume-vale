#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
CMD="${1:-help}"

# Commands that delegate entirely to xtask (no process management)
# Matching by leading substring via case is safe because "check" is
# not a prefix of any other command, "cert" uniquely matches, etc.
case "$CMD" in
    build|test|check|build-web|docker-build|cert|tools|generate-cert)
        shift 2>/dev/null || true
        exec cargo xtask "$CMD" "$@"
        ;;
    infra|validate-world|validate-assets|persistence-smoke|entropy|status)
        shift 2>/dev/null || true
        exec cargo xtask "$CMD" "$@"
        ;;
    help)
        exec cargo xtask help 2>/dev/null || cat <<'EOF'
Yume Vale helper script — delegate to cargo xtask

Usage: ./yume-vale.sh <command> [args...]

Full command list: cargo xtask help
Interactive commands: ./yume-vale.sh play | web | map | server | client
  These wrap cargo xtask with process management (pkill, background, trap, browser).
EOF
        ;;
esac

# Remaining commands involve process orchestration — shell retains
# pkill, trap, background/wait, browser-open, interactive confirmation.
case "$CMD" in
    server)
        pkill -x server 2>/dev/null || true
        sleep 1
        exec cargo xtask server
        ;;
    client)
        exec cargo xtask client
        ;;
    play)
        cd "$ROOT"
        cargo xtask build
        pkill -x server 2>/dev/null || true
        sleep 1
        cargo xtask server &
        SERVER_PID=$!
        trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT INT TERM
        sleep 2
        cargo xtask client
        ;;
    web)
        cd "$ROOT"
        # Kill stale processes from previous runs
        pkill -x server 2>/dev/null || true
        pkill -f "trunk [s]erve" 2>/dev/null || true
        sleep 1
        # Placeholder on :8080 so the URL responds immediately during builds
        PLACEHOLDER_DIR="$(mktemp -d)"
        cat > "$PLACEHOLDER_DIR/index.html" <<'HTML'
<!doctype html><meta charset="utf-8"><meta http-equiv="refresh" content="3">
<title>Yume Vale — compilando…</title>
<body style="font-family:monospace;background:#ffdbe9;color:#7a3b4f;display:grid;place-items:center;height:100vh;margin:0">
<div style="text-align:center"><h1>Yume Vale</h1><p>Compilando… esta página recarrega sozinha quando o jogo estiver pronto.</p></div>
HTML
        python3 -m http.server 8080 --bind 127.0.0.1 --directory "$PLACEHOLDER_DIR" >/dev/null 2>&1 &
        PLACEHOLDER_PID=$!
        trap 'kill "$PLACEHOLDER_PID" 2>/dev/null; kill "$SERVER_PID" 2>/dev/null; pkill -f "trunk [s]erve" 2>/dev/null || true' EXIT INT TERM
        # Cert check + build server
        cargo xtask cert
        cargo xtask build -p server
        ./target/debug/server &
        SERVER_PID=$!
        sleep 2
        # Build wasm
        echo "=== Building wasm client (trunk) — primeira vez demora ==="
        cargo xtask build-web
        # Swap placeholder for trunk
        kill "$PLACEHOLDER_PID" 2>/dev/null || true
        echo "=== Serving at http://127.0.0.1:8080 — Ctrl+C to stop ==="
        cargo xtask web-serve --open
        ;;
    map)
        # When --check is passed as the second arg, delegate entirely to xtask
        if [ "${2:-}" = "--check" ]; then
            exec cargo xtask map --check
        fi
        CACHE="$HOME/.cache/yume-vale/grasp"
        PORT=8765
        # Download / refresh cache
        cargo xtask map --check
        # Kill any stale http.server on our port
        pkill -f "http.serve[r] ${PORT}" 2>/dev/null || true
        sleep 1
        mkdir -p "$CACHE"
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
    *)
        echo "Unknown command: $CMD"
        echo "Usage: ./yume-vale.sh <command>"
        echo "Try: cargo xtask help"
        exit 1
        ;;
esac
