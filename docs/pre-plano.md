# Pré-Plano — Yume Vale

## Stack essencial

```
Bevy 0.19.0  Rust 2024  Avian 3D 0.7.0  Lightyear 0.28.0  Serde 1.0.228  RON 0.12.2  clap 4.6.1  tracing 0.1.44
```

> Versões fixadas em `Cargo.lock`, atualizadas deliberadamente para as últimas estáveis disponíveis. Sem dependências duplicadas.

## Features

| Feature | Responsabilidade |
|---|---|
| `player` | Movimento, inventário, estado |
| `resources` | Coleta, plantas, minérios, madeira |
| `creatures` | Criaturas que o jogador cuida |
| `housing` | Terreno, construção, decoração |
| `quests` | Tarefas dos habitantes, progressão |
| `social` | Chat, emotes, grupo, colaboração |

## Apps

| App | Bevy | Role |
|---|---|---|
| `client` | Completo (3d+ui+audio) | Render, input, áudio |
| `server` | Headless | Simulação, persistência, autoridade |
| `tools` | — | Validação, inspeção, CLI |

## Decisões-chave

- Servidor autoritário, tick 30 Hz, snapshots 10–15 Hz
- Prediction leve — interpolação + reconciliação suficientes no MVP
- Persistência: SQLite (eventos: login, coleta, construção, salvamento periódico)
- Features como crates separados desde o início
- Isolamento: features comunicam-se por componentes compartilhados, eventos, interfaces explícitas — nunca acesso interno ao `World` alheio
- Asset manifestos RON com IDs estáveis

## Entry point — tmux

Script único que abre servidor + cliente lado a lado:

```bash
#!/usr/bin/env bash
# yume-vale.sh — development entry point
set -euo pipefail

SESSION="yume-vale"

cleanup() {
    tmux kill-session -t "$SESSION" 2>/dev/null || true
}
trap cleanup EXIT

cd "$(dirname "$0")"

tmux new-session -d -s "$SESSION" -n dev
tmux send-keys -t "$SESSION" "cargo run -p server" Enter
tmux split-window -h -t "$SESSION"
tmux send-keys -t "$SESSION" "sleep 2 && cargo run -p client" Enter
tmux select-layout -t "$SESSION" even-horizontal
tmux set-option -t "$SESSION" status off
tmux attach -t "$SESSION"
```

Uso: `./yume-vale.sh` — starta servidor (esquerda) e cliente (direita) em panes lado a lado. `Ctrl+C` ou fechar tmux encerra ambos.

## Ordem de implementação sugerida

1. **game_core** — tipos base, constantes, sem Bevy
2. **game_protocol** — mensagens, canais, componentes replicados
3. **features/player** — movimento, inventário básico
4. **game_server** mínimo — spawn jogador, aceitar input, broadcast
5. **game_client** mínimo — conectar, renderizar jogador, input local
6. **features/resources** — coleta + sincronização
7. **features/creatures** — criaturas + interação
8. **features/housing** — construção + decoração
9. **features/quests** — tasks + progressão
10. **features/social** — chat + grupo
11. **game_assets** — loaders, manifestos, validação
12. **tools** — xtask, validação, inspeção
