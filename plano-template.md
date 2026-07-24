# 🌸 [Nome do Jogo] — Plano de Desenvolvimento

> [Tagline de uma linha]

---

## 🎯 Conceito

**O que se faz:**

**O que se vê:**

🎭 **Gênero:**
🎥 **Perspectiva:**
📊 **Complexidade:**
🎨 **Estilo Visual:**
👥 **Jogadores:**

---

## 🧱 Stack principal

| Área | Tecnologia | Responsabilidade |
|---|---|---|
| Engine |  |  |
| Linguagem |  |  |
| Render |  |  |
| UI |  |  |
| Assets |  |  |
| Formato 3D |  |  |
| Física |  |  |
| Multiplayer |  |  |
| Serialização |  |  |
| Configuração |  |  |
| CLI |  |  |
| Logs |  |  |
| Erros |  |  |
| Testes |  |  |
| Lint/format |  |  |

> [Nota sobre versionamento / Cargo.lock]

---

## 🏗️ Arquitetura

```
[projeto]/
├──
├── apps/
│   ├── client/             #
│   ├── server/             #
│   └── tools/              #
├── crates/
│   ├── game_core/          #
│   ├── game_protocol/      #
│   ├── game_assets/        #
│   ├── game_client/        #
│   ├── game_server/        #
│   └── features/
│       ├── [feature]/      #
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
features/[feature]/
├── mod.rs      # API pública
├── components.rs
├── systems.rs
├── events.rs
├── plugin.rs
├── tests.rs
├── README.md
└── AGENTS.md
```

[Regra de isolamento entre features]

---

## 🧩 ECS + Game Loop

### Modelo

| Dado | Forma |
|---|---|
| Estado |  |
| Comportamento |  |
| Configuração |  |
| Comunicação local |  |
| Comunicação remota |  |
| Composição |  |
| Fases |  |

### Schedules

```
PreStartup  →
Startup     →
FixedPreUpdate
FixedUpdate →
FixedPostUpdate
Update      →
PostUpdate  →
```

### Padrão inicial

- Tick simulação:
- Snapshots:
- Render:
- Input:

---

## 🌐 Multiplayer

### Modelo

[Modelo de autoridade]

```
ClientInput {
    tick,
}
```

### Técnicas

- **Client-side prediction:**
- **Snapshot interpolation:**
- **Interest management:**
- **Rate limiting:**
- **Protocol versioning:**

### Canais

```
Input / ações rápidas →
Snapshots             →
Spawn/despawn         →
Lobby/chat            →
Persistência          →
```

---

## 🎨 Assets e conteúdo

| Conteúdo | Formato |
|---|---|
| Modelos e animações |  |
| Texturas |  |
| Áudio |  |
| Configurações |  |
| Tabelas simples |  |
| Shaders |  |
| Fontes |  |

[Regra de IDs estáveis e manifestos]

```ron
(
)
```

---

## 🤖 Regras AI-first

-

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
fn exemplo() {
}
```

### Integração

-

### Determinismo (desejável)

[seed + estado + inputs → checksum]

---

## 🔥 Desenvolvimento

```bash
cargo run -p client
cargo run -p server
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

-

---

## 🌍 Build e distribuição

### Desktop
```bash
```

### Web
```bash
```

### Servidor
```bash
```

---

## 🚀 Infraestrutura

| Componente | Escolha |
|---|---|
| Cliente web |  |
| Assets estáticos |  |
| Game server |  |
| Processo |  |
| Proxy/TLS |  |
| Observabilidade |  |
| CI |  |
| Persistência (MVP) |  |
| Persistência (escala) |  |

> [Regra de persistência]

---

## 🧠 Lições relevantes

### [Tema]

---

## 🚫 Evitar

-

---

## ✅ Decisão central

```
Engine:            
Arquitetura:       
Cliente/servidor:  
Multiplayer:       
Autoridade:        
Configuração:      
Plataformas:       
Persistência:      
Automação:         
Princípio:         
```

> [Frase-guia do projeto]
