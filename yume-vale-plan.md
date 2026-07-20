# 🌸 Yume Vale — Plano de Desenvolvimento

> Aventura social multiplayer online em visão isométrica 3D. Sem combate — explorar, coletar, cuidar de criaturas, decorar e colaborar.

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
| Multiplayer | **Lightyear 0.28.0** | Replicação, lobby, ações |
| Serialização | **Serde 1.0.228** | Protocolo, saves, config |
| Configuração | **RON 0.12.2** | Dados tipados próximos ao Rust |
| CLI | **clap 4.6.1** | Ferramentas de build e validação |
| Logs | **tracing 0.1.44** | Logs estruturados |
| Erros | **thiserror 2.0.18 + anyhow 1.0.103** | Erros de domínio e contexto |
| Integração física/rede | **lightyear_avian3d 0.28.0** | Sincronização Avian ↔ Lightyear |
| Testes | **cargo test** | Systems e regras |
| Lint/format | **Clippy + rustfmt** | Padronização |

> Versões fixadas em `Cargo.lock`, atualizadas deliberadamente. Sem dependências duplicadas. Últimas versões estáveis verificadas em 15 de julho de 2026: Bevy 0.19.0, Avian 3D 0.7.0, Lightyear 0.28.0, Serde 1.0.228, RON 0.12.2, clap 4.6.1, tracing 0.1.44, thiserror 2.0.18, anyhow 1.0.103. `lightyear_avian3d` 0.28.0 cuida da integração física-rede.

---

## 🏗️ Arquitetura

```
yume-vale/
├── Cargo.toml              # workspace
├── Cargo.lock
├── AGENTS.md
├── README.md
├── apps/
│   ├── client/             # Bevy completo: render, áudio, UI, input
│   ├── server/             # Bevy headless: simulação, persistência, rede
│   └── tools/              # validação, importação, inspeção
├── crates/
│   ├── game_core/          # regras independentes de plataforma
│   ├── game_protocol/      # mensagens, canais, componentes replicados
│   ├── game_assets/        # tipos, loaders, manifestos
│   ├── game_client/        # apresentação, câmera, UI, efeitos
│   ├── game_server/        # autoridade, sessões, persistência
│   └── features/
│       ├── player/         # movimento, inventário, estado do jogador
│       ├── resources/      # coleta, plantas, minérios, madeira
│       ├── creatures/      # criaturas que o jogador cuida
│       ├── housing/        # terreno, construção, decoração
│       ├── quests/         # tarefas dos habitantes, progressão
│       └── social/         # chat, emotes, grupo, colaboração
├── assets/
│   ├── models/
│   ├── textures/
│   ├── audio/
│   ├── scenes/
│   └── config/
├── tests/
└── docs/
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

### Padrão inicial

- Tick simulação: 30 Hz (Yume Vale não exige 60 Hz — ações mais lentas)
- Snapshots: 10–15 Hz
- Render: taxa disponível
- Input: coletado por frame, consumido por tick

---

## 🌐 Multiplayer

### Modelo

Servidor autoritário. Cliente transmite intenções (andar, interagir, coletar), nunca posição final ou criação de entidades.

```
ClientInput {
    tick,
    movement,       // direção + correr
    interact,       // qual entidade/alvo
    action,         // coletar, plantar, decorar
    chat_message,
}
```

### Técnicas

- **Client-side prediction:** movimento local imediato (opcional para MVP — latência baixa pode ser suficiente com interpolação)
- **Snapshot interpolation:** entidades remotas entre snapshots
- **Interest management:** somente entidades no campo visual do jogador
- **Rate limiting:** ações por segundo
- **Protocol versioning:** validação na conexão

### Canais

```
Input / ações rápidas → unreliable sequenced
Snapshots             → unreliable sequenced
Spawn/despawn         → reliable ordered
Lobby/chat            → reliable ordered
Persistência          → reliable ordered
```

Yume Vale tem necessidades de rede mais leves que um jogo de ação — pode-se começar com prediction simplificado ou até sem prediction para o MVP, usando interpolação e reconciliação do servidor.

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

Assets com IDs estáveis e manifestos explícitos:

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

### CLI via xtask

```bash
cargo xtask check
cargo xtask test
cargo xtask validate-assets
cargo xtask run-client
cargo xtask run-server
```

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

- Cliente e servidor em memória (transporte local por channels)
- Spawn, replicação, ações coletivas
- Reconexão e incompatibilidade de protocolo
- Validação de assets e configs
- Persistência de mundo

### Determinismo (desejável)

Seed + estado inicial + inputs → checksum esperado na simulação.

---

## 🔥 Desenvolvimento

```bash
cargo run -p client
cargo run -p server
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

- Hot reload de assets, cenas, shaders e configs (nativo do Bevy)
- Features Cargo para `client`, `server`, `web`, `dev`
- Servidor headless sem render/áudio/janela
- Estado persistente serializável (não memória implícita)

---

## 🌍 Build e distribuição

### Desktop
```bash
cargo build --release -p client
```

### Web
```bash
cargo build --release --target wasm32-unknown-unknown -p client --features web
```

Processar `.wasm`, comprimir assets, publicar em Cloudflare Pages. Cliente web conecta ao servidor por WebTransport.

#### ✅ Implementado (20/07/2026) — dev local com cross-play

O servidor roda **3 listeners no mesmo processo** (uma entidade servidora por transporte, todas com `NetcodeServer`):

| Transporte | Porta | Clientes | TLS |
|---|---|---|---|
| UDP + Netcode | 5000 | nativos (Win/Linux/macOS) | netcode |
| WebTransport (HTTP/3) | 5001 | browser (padrão) | self-signed dev, hash pinning |
| WebSocket (`ws://`) | 5002 | browser (fallback via `?transport=ws`) | nenhum (localhost) |

- Pipeline: `trunk serve` a partir de `apps/client` (`./yume-vale.sh web` — gera cert se velho, sobe o servidor, serve em `http://127.0.0.1:8080`)
- Certificado dev: `cargo run -p tools -- generate-cert` → `certs/{server.pem,key.pem,digest.txt}` (gitignored, validade 13 dias — limite do browser para hash pinning; o script regenera após 7)
- O digest é embutido no wasm via `include_str!` (cert estável em disco → digest estável)
- `client_id` no wasm via `getrandom` (SystemTime/PID não existem); rustflags wasm em `.cargo/config.toml` (`getrandom_backend="wasm_js"`, `web_sys_unstable_apis`) — target-specific, não afeta nativo
- Toolchain wasm: rustup 1.96.0 (mesma versão do cargo Homebrew); o script injeta o PATH só nos comandos web
- Verificado ao vivo: 2 browsers (WebTransport) + 1 cliente nativo (UDP) no mesmo servidor, cores consistentes

**Pendente para produção:** deploy Cloudflare Pages, TLS real (WebTransport exige; Caddy não faz proxy de datagramas QUIC — avaliar terminação TLS no próprio game server), fallback automático WT→WS.

### Servidor
```bash
cargo build --release -p server --no-default-features --features server
```

---

## 🚀 Infraestrutura

| Componente | Escolha |
|---|---|
| Cliente web | Cloudflare Pages |
| Assets estáticos | Cloudflare CDN / R2 |
| Game server | VPS Linux |
| Processo | systemd ou container |
| Proxy/TLS | Caddy |
| Observabilidade | tracing + OpenTelemetry |
| CI | GitHub Actions |
| Persistência (MVP) | SQLite (via `game_server`) |
| Persistência (escala) | PostgreSQL |

> Servidor não depende do banco durante cada tick. Persistência em eventos: login, coleta, construção, salvamento periódico do mundo.

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
Multiplayer:       Lightyear (perfil leve — 30 Hz tick, sem prediction crítica)
Autoridade:        servidor
Configuração:      RON + Serde
Plataformas:       desktop + web + mobile (nesta ordem)
Persistência:      SQLite (MVP) → PostgreSQL
Automação:         cargo xtask
Princípio:         contexto mínimo e contratos explícitos
```

> Uma feature deve poder ser compreendida, testada e modificada sem explorar o projeto inteiro.
