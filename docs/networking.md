# Rede — Yume Vale

Operações: [deploy.md](deploy.md).

## 1. Arquitetura

| Responsabilidade | Quem |
|---|---|
| Web IO / transporte | Aeronet 0.21 |
| Netcode, canais, replicação, interpolação, visibilidade, `Link.stats.rtt` | Lightyear 0.28 |
| Protocolo, auth, fallback, regras espaciais, input, EMA18, deploy | Projeto |

**Topologia:** ServiceLB (UDP 5000+5001) → server ← Traefik `/ws` (WS:5002). Um processo, **3 listeners**; máx. 16 jogadores.

## 2. Transporte & Fallback

| Cliente | Transporte | Porta | Descoberta |
|---|---|---|---|
| Nativo | UDP (netcode) | 5000 | `YUME_SERVER_ADDR` / `127.0.0.1:5000` |
| Browser | WT (QUIC) → WS | 5001 → 5002 | host da página + 5001 |
| Browser forçado | WS | 5002 | `?transport=ws` |

Fallback: WT **>8s** unconnected → despawn WT → spawn WS permanente. `ConnectionRejected` não ativa.<br>
Dev HTTP: WT com pin `certs/digest.txt`. Prod HTTPS: WT digest vazio (CA normal) → WSS fallback.<br>
`YUME_SERVER_WS_URL` override compile-time. Vazio = host da página em WT:5001; fallback `wss://{host}/ws`.

## 3. Conexão, Auth & Identidade

```
Connect (client_id via Authentication::Manual)
  → Connected/PendingSession
  → IdentityHello {protocol_version: u32, token: String}
  → versão/capacidade/token ok? → Welcome {player_id: u64, token: String}
  → não → ConnectionRejected {reason}
```

**Token** — restaura PlayerId:

| Plataforma | Armazenamento |
|---|---|
| Nativo | `dirs::config_dir()/yume-vale/identity.json` |
| Browser | localStorage `yume_identity_token` |
| Override | `YUME_IDENTITY_TOKEN` env (nativo) |

Retry 2s. Netcode timeout 10s. `PROTOCOL_ID` = `0x59c3_7a72` (u64, cabe em u32 sem perda).

## 4. Mensagens & Replicação

| Canal | Modo | Direção | Freq |
|---|---|---|---|
| InputChannel | SequencedUnreliable | C2S | 30 Hz |
| ReliableChannel | OrderedReliable | bidir | sob demanda |

| Grupo | Mensagens |
|---|---|
| Input | ClientInput |
| Auth | IdentityHello, Welcome, ConnectionRejected |
| Ações | ActionIntent, ActionRejected, EmoteIntent, EmoteBroadcast |
| Plotagem | PlotBuildIntent, PlotRemoveIntent |
| Estado | InventorySnapshot, BondSnapshot, PlotSnapshot |

| Componente | Modo | Interpolação |
|---|---|---|
| PlayerPosition | linear | Lightyear + EMA exponencial 18.0 |
| ResourceNodeState | linear | Lightyear |
| CreatureState | linear | Lightyear |
| PlayerColor | snap | — |
| DecorationState | snap | — |

PlayerColor: servidor atribui (round-robin), chega via replication, **não** no Welcome. Sem predição/reconciliação.

## 5. Timing, Autoridade & RTT

| Loop | Taxa |
|---|---|
| Server runner | 60 Hz |
| FixedUpdate (ambos) | 30 Hz |
| Visibility | 5 Hz / 30u (owner sempre visível) |

Servidor executa Avian3D/Tnua e replica estado. Interesse: players, criaturas, recursos e decoração; owner sempre visível.

RTT via `Link.stats.rtt`; compare p50/p95 sem alterar o runner isoladamente.

## 6. Toolchain

| Grupo | Ferramentas |
|---|---|
| Runtime | Rust 1.96 / edition 2024, Cargo/xtask, Bevy 0.19 |
| Web | Trunk, wasm-bindgen, wasm-opt, nginx |
| Docker / registry | Docker, GHCR |
| CI/CD | GitHub Actions, Argo CD |
| Orquestração | k3s ServiceLB, Traefik, cert-manager, Sealed Secrets |
| Infra | Ansible, UFW |
| DB | SQLx, SQLite (teste), PostgreSQL 16 (prod) |
| QA local | Crossbeam, Playwright, Hammerspoon/screencapture, `Link.stats.rtt` |

## 7. Segurança & Lacunas

TLS: WT produção usa cadeia PEM + PKCS8/CA; WT dev fixa digest; WS não usa TLS no cluster, apenas no Traefik.

| ID | Hoje | Correção | Prio |
|---|---|---|---|
| G1 | Chave dev Netcode no código | ConnectToken emitido pelo backend; segredo só no servidor | P0 |
| G2 | Token de identidade time+PID | CSPRNG | P0 |
| G3 | `PersistenceResource`/worker não iniciados | Conectar no startup | P1 |
| G4 | `check_cert_rotation` não registrado | Registrar no scheduler | P1 |
| G5 | ConnectionRejected enviado; cliente não consome | Consumir e mostrar UI | P1 |
| G6 | Firewall IaC declara UDP 5001, falta 5000 | Declarar UDP 5000 | P1 |
| G7 | HUD nativo rotula UDP como "WT" | Modelar como UDP | P2 |
| G8 | SNAPSHOT_RATE_HZ morto | Remover ou wirear | P3 |
| G9 | Teste `snapshot_rate_is_15` assere 30 | Renomear | P3 |
| G10 | Comentários YAML/Dockerfile: "4 listeners" / "WS only" | Corrigir na fonte | P2 |

## 8. Verificação & Reuso

**Checklist p/ adotar:**
- [ ] Server+client rebuild juntos (protocolo muda → ambos)
- [ ] `PROTOCOL_ID` u64 (cabe em u32 sem truncamento)
- [ ] Crossbeam integration test (sem sockets reais)
- [ ] Manifest wasm real obrigatório (trunk)
- [ ] Testar: browser padrão, `?transport=ws`, nativo
- [ ] Validar RTT p50/p95 após tickrate

Comandos: `./yume-vale.sh {check|play|web|build-web}` e `./yume-vale.sh infra check`.
