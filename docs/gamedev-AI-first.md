# 🎮🤖 AI-First Game Dev Stack

Stack unificada em Rust para jogos multiplayer 2D/3D executáveis em desktop, web e mobile, priorizando baixa entropia, arquitetura ECS, código compartilhado e desenvolvimento orientado por agentes.

## 🎯 Objetivos

* 🌍 Uma base para Windows, Linux, macOS, WebAssembly, Android e iOS
* 🦀 Rust em cliente, servidor, simulação e ferramentas
* 🧩 ECS como modelo central da aplicação
* 🔒 Servidor autoritário com simulação compartilhada
* 🤖 Estrutura previsível e compreensível por agentes
* 📄 Configuração, cenas e regras versionáveis em texto
* 🔥 Iteração rápida com hot reload de assets e configurações
* 🧪 Simulação testável sem janela, GPU ou conexão real

## 🧱 Stack principal

| Área              | Tecnologia             | Responsabilidade                                                  |
| ----------------- | ---------------------- | ----------------------------------------------------------------- |
| Engine            | **Bevy 0.19**          | Runtime completo, ECS, renderização, input, áudio, assets e cenas |
| Linguagem         | **Rust 2024**          | Toda a aplicação                                                  |
| ECS               | **Bevy ECS**           | Entidades, componentes, recursos, eventos e systems               |
| Render            | **Bevy Render / wgpu** | Renderização 2D e 3D multiplataforma                              |
| UI                | **Bevy UI**            | HUD, menus e interfaces dentro do jogo                            |
| Assets            | **Bevy AssetServer**   | Carregamento, handles e hot reload                                |
| Formato 3D        | **glTF/GLB**           | Modelos, materiais, animações e cenas                             |
| Física            | **Avian**              | Colisão, rigid bodies e queries integradas ao ECS                 |
| Multiplayer       | **Lightyear**          | Replicação, prediction, rollback e interpolation                  |
| Transporte nativo | **UDP + Netcode**      | Desktop e mobile                                                  |
| Transporte web    | **WebTransport**       | Cliente WebAssembly                                               |
| Serialização      | **Serde**              | Protocolos, saves e configuração                                  |
| Configuração      | **RON**                | Dados tipados próximos ao modelo Rust                             |
| CLI               | **clap**               | Ferramentas de build, validação e inspeção                        |
| Logs              | **tracing**            | Logs estruturados no cliente e servidor                           |
| Erros             | **thiserror + anyhow** | Erros de domínio e contexto operacional                           |
| Testes            | **cargo test**         | Sistemas, regras e simulação                                      |
| Benchmarks        | **Criterion**          | Game loop, física, serialização e rede                            |
| Lint/format       | **Clippy + rustfmt**   | Padronização automática                                           |

As versões devem ser fixadas no `Cargo.lock` e atualizadas deliberadamente. Evitar dependências duplicadas para a mesma responsabilidade.

## 🏗️ Arquitetura

O projeto utiliza um Cargo Workspace com executáveis separados e crates compartilhados:

```text
game/
├── Cargo.toml
├── AGENTS.md
├── README.md
├── apps/
│   ├── client/         # Bevy completo: render, áudio, UI e input
│   ├── server/         # Bevy headless: simulação e rede
│   └── tools/          # validação, importação e inspeção
├── crates/
│   ├── game_core/      # regras independentes de plataforma
│   ├── game_protocol/  # mensagens, canais e componentes replicados
│   ├── game_assets/    # tipos, loaders e manifestos
│   ├── game_client/    # apresentação, câmera, UI e efeitos
│   ├── game_server/    # autoridade, sessões e persistência
│   └── features/
│       ├── movement/
│       ├── combat/
│       ├── player/
│       ├── enemies/
│       ├── inventory/
│       └── world/
├── assets/
│   ├── models/
│   ├── textures/
│   ├── audio/
│   ├── scenes/
│   └── config/
├── tests/
└── docs/
```

Cada feature deve ser autocontida:

```text
features/combat/
├── mod.rs
├── components.rs
├── systems.rs
├── events.rs
├── plugin.rs
├── tests.rs
├── README.md
└── AGENTS.md
```

A API pública da feature fica em `mod.rs`. O restante permanece privado por padrão. Uma feature não acessa internamente outra feature; comunica-se por componentes compartilhados, eventos ou interfaces explícitas.

## 🧩 Modelo ECS

* **Entity:** identidade sem comportamento
* **Component:** estado pequeno e serializável
* **System:** transformação de dados por queries
* **Resource:** estado global inevitável e claramente tipado
* **Event/Message:** comunicação desacoplada
* **Plugin:** unidade de composição e inicialização
* **State:** fases explícitas como loading, menu, lobby e gameplay

Regras:

```text
Dados             → Components
Comportamento     → Systems
Configuração      → Resources ou assets RON
Comunicação local → Events
Comunicação remota→ Protocol messages
Composição        → Plugins
```

Evitar entidades-deus, resources genéricos, systems com responsabilidades múltiplas e acesso global indireto.

## 🔁 Schedules e game loop

A simulação roda em tick fixo; apresentação e UI rodam por frame:

```text
PreStartup  → registro e configuração
Startup     → inicialização
FixedPreUpdate
FixedUpdate → input, gameplay, física e autoridade
FixedPostUpdate
Update      → câmera, animação, UI e efeitos
PostUpdate  → sincronização visual e render
```

Padrão inicial:

```text
Simulation tick: 60 Hz
Snapshots:       20–30 Hz
Render:          taxa disponível
Input:           coletado por frame e consumido por tick
```

A frequência deve ser configurável. Sistemas determinísticos não usam tempo de frame, relógio do sistema, ordem instável de coleções ou aleatoriedade sem seed.

## 🌐 Multiplayer

### Modelo

O servidor é autoritário. O cliente transmite intenções, nunca posição final, dano confirmado ou criação arbitrária de entidades.

```text
ClientInput {
    tick,
    movement,
    aim,
    actions
}
```

O servidor valida o input, executa a simulação e replica o estado autorizado.

### Técnicas

* **Client-side prediction:** o jogador local executa imediatamente seus inputs
* **Server reconciliation:** snapshots corrigem o estado e reaplicam inputs pendentes
* **Snapshot interpolation:** entidades remotas são renderizadas entre snapshots
* **Rollback:** estados anteriores podem ser restaurados e simulados novamente
* **Lag compensation:** histórico limitado permite validar ações no tempo percebido
* **Interest management:** somente entidades relevantes são replicadas
* **Rate limiting:** inputs e comandos possuem limites por conexão
* **Protocol versioning:** cliente e servidor validam compatibilidade antes da sessão

### Canais

```text
Input         → unreliable sequenced
Snapshots     → unreliable sequenced
Spawn/despawn → reliable ordered
Lobby/chat    → reliable ordered
Assets/config → reliable ordered
```

O protocolo fica exclusivamente em `game_protocol`. Componentes de apresentação, handles de assets e tipos específicos da plataforma não atravessam a rede.

## ♻️ Determinismo

Determinismo é obrigatório na simulação compartilhada e desejável no restante:

* Tick fixo e numerado
* RNG com seed explícita
* Inputs graváveis
* Ordem de execução declarada
* Estado inicial reproduzível
* Checksums periódicos
* Replays baseados em seed + inputs
* Física configurada igualmente entre os peers quando usada na prediction

Quando determinismo completo não for viável, o servidor continua autoritário e replica snapshots.

## 🎨 Assets e conteúdo

Formatos preferidos:

| Conteúdo            | Formato                  |
| ------------------- | ------------------------ |
| Modelos e animações | `.glb` / `.gltf`         |
| Texturas            | `.png`, `.webp`, `.ktx2` |
| Áudio               | `.ogg`                   |
| Configurações       | `.ron`                   |
| Tabelas simples     | `.csv`                   |
| Shaders             | `.wgsl`                  |
| Fontes              | `.ttf` / `.otf`          |

Assets devem ter IDs estáveis e manifestos explícitos:

```ron
(
    id: "weapon.blaster",
    damage: 20,
    cooldown_ms: 250,
    projectile_speed: 18.0,
    model: "models/weapons/blaster.glb",
)
```

Evitar lógica escondida em cenas, nomes mágicos de nós e formatos proprietários. Todo asset referenciado deve ser validável por CLI e CI.

## 🤖 Regras AI-first

O agente deve conseguir alterar uma feature abrindo poucos arquivos. Cada módulo relevante contém:

```text
README.md   → finalidade, fluxo e exemplos
AGENTS.md   → limites, invariantes e comandos
tests.rs    → comportamento esperado
plugin.rs   → composição pública
```

Regras obrigatórias:

* Uma solução canônica para cada problema
* Contratos pequenos e tipos explícitos
* Dependências visíveis no `Cargo.toml`
* Nenhuma configuração crítica implícita
* Nenhum arquivo concentrando múltiplos domínios
* Testes próximos da feature
* Comandos reproduzíveis e não interativos
* Logs estruturados e mensagens de erro acionáveis
* Alterações arquiteturais atualizam documentação e testes

O agente pode operar o projeto por CLI:

```bash
cargo xtask check
cargo xtask test
cargo xtask validate-assets
cargo xtask run-client
cargo xtask run-server
cargo xtask replay <arquivo>
```

`xtask` centraliza tarefas do projeto e evita scripts dispersos em Bash, npm ou ferramentas específicas do sistema.

## 🧪 Testes

### Unitários

Testar regras puras e sistemas isolados:

```rust
#[test]
fn damage_reduces_health() {
    let mut app = App::new();
    app.add_plugins(CombatPlugin);
    // Monta o World, executa um tick e verifica componentes.
}
```

### Integração

* Cliente e servidor em memória
* Transporte local por channels
* Spawn, replication e despawn
* Prediction e reconciliation
* Reconexão e incompatibilidade de protocolo
* Validação de assets e configurações

### Determinismo

```text
seed + estado inicial + inputs → checksum esperado
```

O mesmo replay deve produzir o mesmo resultado em execuções repetidas.

### Performance

Benchmarks mínimos:

* Tempo por tick
* Quantidade de entidades processadas
* Física por tick
* Bytes por cliente por segundo
* Serialização de snapshots
* Tempo de carregamento
* Tamanho do bundle WebAssembly

## 🔥 Desenvolvimento

```bash
cargo run -p client
cargo run -p server
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Durante desenvolvimento:

* Hot reload para assets, cenas, shaders e configurações
* Recompilação Rust acelerada com linker apropriado
* Features Cargo para separar `client`, `server`, `web` e `dev`
* Servidor headless sem renderização, áudio ou janela
* Ferramentas executadas pelo mesmo workspace

Hot reload de código Rust não deve ser requisito arquitetural. Estado persistente deve sobreviver por dados serializáveis, não por memória implícita do processo.

## 🌍 Build e distribuição

### Desktop

```bash
cargo build --release -p client
```

Distribuir binário e assets para Windows, Linux e macOS.

### Web

```bash
cargo build --release \
  --target wasm32-unknown-unknown \
  -p client \
  --features web
```

Processar o `.wasm`, comprimir assets e publicar em Cloudflare Pages. O cliente web conecta ao servidor por WebTransport ou fallback compatível definido pelo projeto.

### Mobile

Gerar builds Android e iOS a partir do mesmo cliente, mantendo adaptações de lifecycle, input, permissões e packaging isoladas em módulos de plataforma.

### Servidor

```bash
cargo build --release -p server \
  --no-default-features \
  --features server
```

Executar como processo ou container em VPS. Cada instância hospeda uma ou mais partidas conforme o perfil medido. Métricas e logs devem existir antes da otimização de orquestração.

## 🚀 Infraestrutura inicial

| Componente             | Escolha                 |
| ---------------------- | ----------------------- |
| Cliente web            | Cloudflare Pages        |
| Assets estáticos       | Cloudflare CDN / R2     |
| Game server            | VPS Linux               |
| Processo               | systemd ou container    |
| Proxy/TLS              | Caddy                   |
| Observabilidade        | tracing + OpenTelemetry |
| CI                     | GitHub Actions          |
| Artefatos desktop      | GitHub Releases         |
| Persistência inicial   | SQLite                  |
| Persistência escalável | PostgreSQL              |

O game server não deve depender do banco durante cada tick. Persistência acontece em eventos definidos: login, início/fim de partida, checkpoints e alterações de inventário.

## 🚫 Evitar

* Estado global sem tipo e proprietário
* Lógica de jogo dentro da camada visual
* Código duplicado entre cliente e servidor
* Protocolo definido junto à UI
* Números mágicos espalhados
* Assets binários como única fonte de configuração
* Dependência de editor visual para operações básicas
* Features com acesso direto irrestrito ao `World`
* Sistemas dependentes da ordem acidental de execução
* Crates diferentes resolvendo a mesma responsabilidade
* abstrações prematuras sobre APIs ainda pequenas

## ✅ Decisão central

```text
Engine completa:    Bevy
Arquitetura:        ECS + plugins por feature
Cliente/servidor:   Rust compartilhando game_core
Multiplayer:        Lightyear
Autoridade:         servidor
Configuração:       RON + Serde
Plataformas:        desktop + web + mobile
Automação:          cargo xtask
Princípio:          contexto mínimo e contratos explícitos
```

> Uma feature deve poder ser compreendida, testada e modificada sem explorar o projeto inteiro.

---

## 🎯 Ideias de Jogos

> **Formato de cada ideia:**
> - **Conceito** — mecânica, regras, jogabilidade (o que se faz)
> - **Ambientação** — visual, tema, estilo, cenário (o que se vê)

---

### Yume Vale

📋 **Visão Geral do Projeto**

**Conceito**
Aventura social multiplayer online em visão isométrica 3D. Sem combate — os jogadores exploram um vale encantado a pé, coletam recursos, completam tarefas para os habitantes, cuidam de criaturas e decoram seu espaço. Até 16 jogadores compartilham o mesmo vale, podendo se encontrar livremente, interagir com o ambiente e colaborar em objetivos coletivos que transformam o mundo ao redor.

**Ambientação**
Mundo 3D polido com visual fofo inspirado no charme Nintendo. Paleta pastel quente, personagens arredondados e expressivos, vegetação exuberante, arquitetura whimsical. Cada região tem personalidade própria: prados ensolarados, florestas aconchegantes, praias de cristal e jardins noturnos estrelados. Tudo convida à exploração tranquila e à imersão.

**Classificação**
🎭 **Gênero:** Aventura / Social / Casual
🎥 **Perspectiva:** 3D isométrica
📊 **Complexidade:** Baixa
🎨 **Estilo Visual:** Polido estilo Nintendo, fofo e colorido
👥 **Jogadores:** Multiplayer online (até 16 por sessão)

---

