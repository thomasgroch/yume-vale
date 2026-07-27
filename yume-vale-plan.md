# 🌸 Yume Vale — Plano de Desenvolvimento

> Aventura social multiplayer online em visão isométrica 3D. Sem combate — explorar, coletar, cuidar de criaturas, decorar e colaborar.
>
> **Estado (27/07/2026):** protótipo multiplayer funcional e no ar em https://yume.lab.thomasdev.xyz — movimento com física (Tnua+Avian3d), raposa animada, arena "Ruínas de Cristal", cross-play nativo/web/mobile, deploy GitOps (Docker → ghcr → k3s + Argo CD). As features abaixo (resources, creatures, housing, quests, social) são a **visão futura** — hoje o workspace é intencionalmente mínimo (ver `crates/`).

---

## 🎯 Conceito

**O que se faz:** Até 16 jogadores exploram um vale encantado a pé, coletam recursos, completam tarefas para os habitantes, cuidam de criaturas e decoram seu espaço. Podem se encontrar livremente, interagir com o ambiente e colaborar em objetivos coletivos que transformam o mundo ao redor.

**O que se vê:** Mundo 3D polido, visual fofo inspirado no charme Nintendo. Paleta pastel quente, personagens arredondados e expressivos, vegetação exuberante, arquitetura whimsical. Prados, florestas, praias de cristal e jardins noturnos estrelados.

🎭 **Gênero:** Aventura / Social / Casual  
🎥 **Perspectiva:** 3D isométrica  
📊 **Complexidade:** Baixa  
🎨 **Estilo Visual:** Polido estilo Nintendo, fofo e colorido  
👥 **Jogadores:** Multiplayer online (até 16 por sessão)

---

## 🧱 Stack principal

| Área | Tecnologia | Responsabilidade |
|---|---|---|
| Engine | **Bevy 0.19.0** | ECS, render, input, áudio, assets |
| Linguagem | **Rust 2024** | Toda a aplicação |
| Render | **Bevy Render / wgpu** | 2D e 3D multiplataforma |
| UI | **Bevy UI** | HUD, menus, inventário |
| Assets | **Bevy AssetServer** | Carregamento, handles, hot reload |
| Formato 3D | **glTF/GLB** | Modelos, materiais, animações |
| Física | **Avian 3D 0.7.0** | Colisão, queries (leve — sem rigid bodies complexos) |
| Multiplayer | **Lightyear 0.28.0** | Replicação, mensagens |
| Transporte web | **aeronet_websocket / aeronet_webtransport 0.21** | Listeners WS/WT (testes in-memory via bevy_replicon) |
| Movimento | **bevy-tnua 0.32 + bevy-tnua-avian3d 0.12** | Character controller (walk/jump) |
| Serialização | **Serde 1.0.228** | Protocolo, saves, config |
| Configuração | **RON 0.12.2** | Dados tipados próximos ao Rust |
| Logs | **tracing 0.1.44** | Logs estruturados |
| Erros | **thiserror 2.0.18** | Erros de domínio |
| Testes | **cargo test** | Systems e regras |
| Lint/format | **Clippy + rustfmt** | Padronização |

> Versões fixadas em `Cargo.lock`, atualizadas deliberadamente. Sem dependências duplicadas. Últimas versões estáveis verificadas em 15 de julho de 2026: Bevy 0.19.0, Avian 3D 0.7.0, Lightyear 0.28.0, Serde 1.0.228, RON 0.12.2, clap 4.6.1, tracing 0.1.44, thiserror 2.0.18, anyhow 1.0.103.

---

## 🏗️ Arquitetura

### Árvore atual (protótipo)

```
yume-vale/
├── Cargo.toml              # workspace
├── AGENTS.md / README.md
├── apps/
│   ├── client/             # Bevy completo + index.html/nginx.conf/dist (wasm)
│   ├── server/             # Bevy headless: simulação, física, rede
│   └── tools/              # geração do certificado dev (WebTransport)
├── crates/
│   ├── game_core/          # regras independentes de plataforma (layouts determinísticos, constantes)
│   ├── game_protocol/      # mensagens, canais, componentes replicados, paleta
│   ├── game_client/        # conexão, input, câmera, visuais, menu, HUD, touch, debug
│   ├── game_server/        # autoridade: listeners, spawn, input, física
│   └── features/
│       └── player/         # movimento (Tnua), inventário, estado do jogador
├── assets/                 # GLBs (raposa Meshy, arena), config
├── deploy/                 # manifests k3s + Argo CD (ver docs/deploy.md)
└── docs/                   # deploy, planejamento, screenshots
```

### Visão futura (features planejadas)

```
features/
├── resources/      # coleta, plantas, minérios, madeira
├── creatures/      # criaturas que o jogador cuida
├── housing/        # terreno, construção, decoração
├── quests/         # tarefas dos habitantes, progressão
└── social/         # chat, emotes, grupo, colaboração
```

Cada feature autocontida:

```
features/resources/
├── mod.rs      # API pública
├── components.rs
├── systems.rs
├── events.rs
├── plugin.rs
├── tests.rs
├── README.md
└── AGENTS.md
```

Uma feature não acessa interna de outra — comunicação por componentes compartilhados, eventos ou interfaces explícitas.

---

## 🧩 ECS + Game Loop

### Modelo

| Dado | Forma |
|---|---|
| Estado | Componentes pequenos e serializáveis |
| Comportamento | Systems |
| Configuração | Resources ou RON |
| Comunicação local | Events |
| Comunicação remota | Protocol messages |
| Composição | Plugins |
| Fases | States: Loading, Menu, InGame |

### Schedules

```
PreStartup  → registro e configuração
Startup     → inicialização do mundo
FixedPreUpdate
FixedUpdate → input, gameplay, física, autoridade
FixedPostUpdate
Update      → câmera, animação, UI, efeitos
PostUpdate  → sincronização visual e render
```

### Padrão atual

- Tick simulação: 30 Hz (Yume Vale não exige 60 Hz — ações mais lentas)
- Snapshots: 30 Hz
- Render: taxa disponível
- Input: coletado por frame, consumido por tick

---

## 🌐 Multiplayer

### Modelo

Servidor autoritário. Cliente transmite intenções (andar, correr, pular), nunca posição final ou criação de entidades.

Implementado hoje:

```rust
ClientInput { tick, move_x: i8, move_z: i8, run: bool, jump: bool }  // escala i8: 127
Welcome     { ... }                                                   // server → client
PlayerPosition  // componente replicado com interpolação linear
PlayerColor     // componente replicado (atribuído pelo servidor, round-robin)
```

Futuro: `interact`, `action`, `chat_message`.

### Técnicas

- **Client-side prediction:** movimento local imediato (opcional para MVP — latência baixa pode ser suficiente com interpolação)
- **Snapshot interpolation:** entidades remotas entre snapshots
- **Interest management:** somente entidades no campo visual do jogador
- **Rate limiting:** ações por segundo
- **Protocol versioning:** validação na conexão

### Canais (implementado)

```
ClientInput  → sequenced unreliable, 30 Hz, ClientToServer
Confiável    → ordered reliable, bidirecional (Welcome, futuro spawn/chat)
```

**Replicação:** `PlayerPosition` (com interpolação linear) e `PlayerColor` usam a feature `replication` do Lightyear — posições fluem como componentes replicados, não como mensagens manuais. Movimento não usa client-side prediction: o cliente interpola as posições replicadas (fator 20.0), suficiente para o ritmo do jogo.

---

## 🎨 Assets e conteúdo

| Conteúdo | Formato |
|---|---|
| Modelos e animações | `.glb` / `.gltf` |
| Texturas | `.png`, `.webp`, `.ktx2` |
| Áudio | `.ogg` |
| Configurações | `.ron` |
| Tabelas simples | `.csv` |
| Shaders | `.wgsl` |
| Fontes | `.ttf` / `.otf` |

Assets com IDs estáveis e manifestos explícitos (visão futura — hoje os assets são GLBs gerados via Meshy AI carregados direto pelo AssetServer: raposa rigada + animações em `assets/models/fox/`, arena em `assets/models/arena/`):

```ron
(
    id: "creature.fluffball",
    name: "Fluffball",
    model: "models/creatures/fluffball.glb",
    animations: {
        idle: "models/creatures/fluffball_idle.glb",
        happy: "models/creatures/fluffball_happy.glb",
    },
    food: "resource.berry",
    growth_time_s: 300,
)
```

---

## 🤖 Regras AI-first

- Uma solução canônica para cada problema
- Contratos pequenos e tipos explícitos
- Dependências visíveis no `Cargo.toml`
- Nenhuma configuração crítica implícita
- Nenhum arquivo concentrando múltiplos domínios
- Testes próximos da feature
- Comandos reproduzíveis e não interativos
- Logs estruturados e erros acionáveis
- Alterações arquiteturais atualizam docs e testes

### Automação

Hoje: `./yume-vale.sh` (build/test/check/play/web/map/clean-build). `cargo xtask` fica como opção futura se o script crescer.

---

## 🧪 Testes

### Unitários

```rust
#[test]
fn collecting_resource_adds_to_inventory() {
    let mut app = App::new();
    app.add_plugins(ResourcesPlugin);
    // spawn jogador + recurso, executa tick, verifica inventário
}
```

### Integração

Implementado em `crates/game_server/tests/integration.rs`: cliente e servidor em memória (transporte local via channels/bevy_replicon), spawn, dedup de reconexão, aplicação de input.

Futuro: reconexão e incompatibilidade de protocolo, validação de assets e configs, persistência de mundo.

### Determinismo (desejável)

Seed + estado inicial + inputs → checksum esperado na simulação.

---

## 🔥 Desenvolvimento

```bash
./yume-vale.sh play     # servidor + cliente nativos juntos
./yume-vale.sh web      # servidor + cliente wasm em http://127.0.0.1:8080
./yume-vale.sh test     # cargo test --workspace
./yume-vale.sh check    # fmt + clippy (-D warnings) + testes
```

- Assets GLB/texturas carregados via AssetServer
- Servidor headless sem render/áudio/janela (MinimalPlugins + ScheduleRunner 30 Hz)
- Builds wasm somente via toolchain rustup dedicada (ver AGENTS.md)

---

## 🌍 Build e distribuição

### Desktop
```bash
cargo build --release -p client
```

### Web

Cliente wasm (trunk) servido como estático pelo nginx. Cliente web conecta ao servidor por **WebSocket** (produção) ou WebTransport (dev local, `?transport=ws` para fallback).

#### ✅ Implementado — dev local com cross-play

O servidor roda **3 listeners no mesmo processo** (uma entidade servidora por transporte, todas com `NetcodeServer`):

| Transporte | Porta | Clientes | TLS |
|---|---|---|---|
| UDP + Netcode | 5000 | nativos (Win/Linux/macOS) | netcode |
| WebTransport (HTTP/3) | 5001 | browser (dev local) | self-signed dev, hash pinning |
| WebSocket (`ws://`) | 5002 | browser (produção + fallback) | Traefik/cert-manager em produção |

- Pipeline: `trunk serve` a partir de `apps/client` (`./yume-vale.sh web` — gera cert se velho, sobe o servidor, serve em `http://127.0.0.1:8080`)
- Certificado dev: `./yume-vale.sh generate-cert` → `certs/{server.pem,key.pem,digest.txt}` (gitignored, validade 13 dias; o script regenera após 7)
- O digest é embutido no wasm via `include_str!` (cert estável em disco → digest estável)
- Toolchain wasm: rustup 1.96.0; o script injeta o PATH só nos comandos web
- Reconexão resiliente: cliente retenta `Connect` a cada 2s; servidor despawna player obsoleto com o mesmo `PlayerId` (sem duplicatas/fantasmas)
- Mobile: touch auto-detectado (joystick + botão de pulo), texturas WebP, wasm release com `wasm-opt -Oz` (shim binaryen v131 no Dockerfile — ver comentários em `Dockerfile.client`)

#### ✅ Implementado — produção (k3s + Argo CD)

Live em https://yume.lab.thomasdev.xyz. Pipeline completo em [docs/deploy.md](docs/deploy.md):

- Push em `main` → GitHub Actions builda `Dockerfile.{server,client}` → `ghcr.io/thomasgroch/yume-vale-{server,client}:sha-<commit>`
- O workflow pin o sha nos manifests (`deploy/1*.yaml`/`2*.yaml`, commit `[skip ci]`) → Argo CD faz o rollout
- Ingress Traefik: `/` → nginx (wasm estático), `/ws` → servidor WS:5002; UDP:5000 via Service LoadBalancer para clientes nativos
- TLS automático via cert-manager (letsencrypt-prod); `YUME_SERVER_WS_URL` embutida no wasm no build Docker (vazio = deriva do host da página)

**Pendente para produção:** WebTransport em produção (exige terminação QUIC no próprio game server — proxy de datagramas QUIC não é trivial), fallback automático WT→WS no cliente.

### Servidor
```bash
cargo build --release -p server
```

---

## 🌍 Infraestrutura (atual)

| Componente | Escolha |
|---|---|
| Cliente web | nginx servindo wasm estático (imagem Docker própria) |
| Game server | Container Linux (imagem Docker própria) |
| Registry | ghcr.io |
| Cluster | k3s (namespace `yume-vale`) |
| GitOps | Argo CD (`deploy/argocd-application.yaml`) |
| Proxy/TLS | Traefik Ingress + cert-manager (Let's Encrypt) |
| CI | GitHub Actions (`.github/workflows/images.yml`) |
| Logs | tracing (stdout → logs do pod) |

> Servidor autoritário single-replica (`strategy: Recreate`) — um mundo por processo. Persistência (SQLite → PostgreSQL) é visão futura, junto com as features de gameplay.

---

## 🧠 Lições relevantes (do desenvolvimento anterior)

### Features Cargo
`default-features = false` no workspace não pode ser sobrescrito por `default-features = false` no crate. O workspace define o valor; crates só adicionam features.

```toml
# workspace
bevy = { version = "0.19.0", default-features = false }

# crate — herda default-features=false
bevy = { workspace = true, features = ["3d", "ui"] }
```

### Bevy 0.19 — pontos de atenção
- `bevy_ecs`, `bevy_math`, `bevy_transform` são dependências internas **sempre compiladas** — não são feature flags
- `Query::get_single_mut` → `single_mut` (retorna `Result<Mut<T>>`)
- `AmbientLight` agora é `Component`, não `Resource`
- `font_size: f32` → `FontSize::Px(f32)`
- `StandardMaterial::emissive` é `LinearRgba`, não `Color`

### Avian 0.7
- Requer `f32` + `3d` + `parry-f32` features
- Sem `f32`, tipos matemáticos (`Scalar`, `Vector`, etc.) simplesmente não existem

### Organização do workspace
- Estrutura de features como crates separados desde o início — as fronteiras disciplinam as responsabilidades e evitam refatoração custosa depois.
- A complexidade de Cargo (features compatíveis, dependências duplicadas) é gerenciável e justificada pelo isolamento conceitual.

---

## 🚫 Evitar

- Estado global sem tipo e proprietário
- Lógica de jogo dentro da camada visual
- Código duplicado entre cliente e servidor
- Protocolo definido junto à UI
- Números mágicos espalhados
- Assets binários como única fonte de configuração
- Dependência de editor visual para operações básicas
- Features com acesso direto irrestrito ao `World`
- Sistemas dependentes da ordem acidental de execução
- Abstrações prematuras sobre APIs ainda pequenas

---

## ✅ Decisão central

```
Engine:            Bevy 0.19
Arquitetura:       ECS + plugins por feature
Cliente/servidor:  Rust compartilhando game_core
Multiplayer:       Lightyear (perfil leve — 30 Hz tick, replicação + interpolação, sem prediction)
Autoridade:        servidor
Configuração:      RON + Serde
Plataformas:       desktop + web + mobile (todas funcionando)
Deploy:            Docker → ghcr → k3s + Argo CD
Persistência:      SQLite (MVP) → PostgreSQL  [futuro]
Automação:         yume-vale.sh
Princípio:         contexto mínimo e contratos explícitos
```

> Uma feature deve poder ser compreendida, testada e modificada sem explorar o projeto inteiro.
