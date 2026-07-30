# 🌸 Yume Vale

Protótipo de aventura social multiplayer 3D — um vale encantado onde raposas exploram, pulam e se encontram. Sem combate: o foco é presença compartilhada num mundo fofo.

**Jogue agora:** https://yume.lab.thomasdev.xyz (desktop e mobile)

## Stack

| Área | Tecnologia |
|---|---|
| Engine | Bevy 0.19 (Rust 2024) |
| Multiplayer | Lightyear 0.28 (servidor autoritário, replicação + mensagens) |
| Física | Avian3D 0.7 + Tnua (server-side) |
| Cliente web | wasm32 + trunk, servido por nginx |
| Deploy | Docker → ghcr.io → k3s + Argo CD (GitOps) |

## Quickstart

```bash
./yume-vale.sh play    # builda servidor+cliente e roda os dois (nativo)
./yume-vale.sh web     # cert dev + servidor + trunk serve em http://127.0.0.1:8080
./yume-vale.sh check   # fmt + clippy (-D warnings) + testes
```

Todos os comandos passam por `./yume-vale.sh` — rode `./yume-vale.sh help` para a lista completa.

Builds wasm usam toolchain rustup dedicada (injetada pelo script); builds nativos usam o cargo do Homebrew. Detalhes em [AGENTS.md](AGENTS.md).

## Controles

| Ação | Desktop | Mobile |
|---|---|---|
| Andar | WASD / setas | joystick virtual |
| Correr | Shift | — |
| Pular | Espaço | botão de pulo |
| Câmera | arrastar (botão direito) | arrastar |
| Zoom | scroll | — |
| Debug inspector | F3 | — |

## Estrutura

```
apps/      client (Bevy completo), server (headless), tools (cert dev)
crates/    game_core, game_protocol, game_client, game_server, features/player
assets/    modelos GLB (raposa Meshy + arena), config
deploy/    manifests k3s (namespace, server, client, ingress, Argo CD)
docs/      deploy, planejamento, screenshots
```

## Documentação

- [AGENTS.md](AGENTS.md) — guia para agentes: comandos, mapa do código, invariantes
- [docs/deploy.md](docs/deploy.md) — pipeline CI/CD, k3s, Argo CD, ingress, variáveis de ambiente
- [docs/networking.md](docs/networking.md) — blueprint de rede: transporte, protocolo, replicação, tickrate
- [yume-vale-plan.md](yume-vale-plan.md) — plano de desenvolvimento (visão + estado atual)
- [docs/gamedev-AI-first.md](docs/gamedev-AI-first.md) — filosofia da stack
