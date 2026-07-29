# Deploy — Yume Vale

Produção: **https://yume.lab.thomasdev.xyz** — k3s + Argo CD (GitOps). Todo push em `main` vai ao ar automaticamente.

## Pipeline

```
push em main
  └─ GitHub Actions (.github/workflows/images.yml)
       ├─ check: fmt + clippy + tests (SQLite + PostgreSQL)
       ├─ wasm: cargo build --target wasm32-unknown-unknown
       └─ build:
            ├─ Dockerfile.server → ghcr.io/thomasgroch/yume-vale-server:{latest,sha-<commit>}
            └─ Dockerfile.client → ghcr.io/thomasgroch/yume-vale-client:{latest,sha-<commit>}
  └─ job rollout: sed pin sha-<commit> em deploy/10-server.yaml e deploy/20-client.yaml
       └─ commit "deploy: pin images ... [skip ci]" → push
  └─ Argo CD (Application yume-vale, path deploy/, automated+prune+selfHeal)
       └─ rollout dos pods no namespace yume-vale
```

O pin do sha é necessário porque `:latest` nunca muda o manifest — sem diff o Argo CD não faz rollout. O `[skip ci]` evita loop do workflow.

## Imagens

| Imagem | Base final | Conteúdo |
|---|---|---|
| `yume-vale-server` | distroless-ish (binário server) | game server headless, 4 listeners |
| `yume-vale-client` | `nginx:1-alpine` | wasm release (trunk) + assets estáticos |

Notas do build do cliente (`Dockerfile.client`):

- `trunk build --release` com `CARGO_BUILD_JOBS=2` (LTO + wasm-opt estouravam 8GB de RAM)
- **Shim de wasm-opt**: trunk roda wasm-opt v123 sem feature flags, mas o wasm usa bulk-memory (padrão Rust ≥ 1.87) — um shim em `/usr/local/bin/wasm-opt` chama binaryen v131 com `--enable-bulk-memory-opt --enable-nontrapping-float-to-int --enable-sign-ext` e força `-Oz`
- `certs/digest.txt` é criado com conteúdo dummy no build (o `include_str!` do digest WT exige o arquivo; produção conecta via WS e nunca usa WT)
- `ARG YUME_SERVER_WS_URL` — embutida no wasm em compile time (`option_env!`); vazio = o cliente deriva `wss://{host}/ws` do host da página (ver `crates/game_client/src/connection.rs`)
- Texturas em WebP (o `nginx.conf` usa o `mime.types` padrão do nginx — um bloco `types` custom quebrou o HTML uma vez; não reintroduzir)

## Rede / portas

| Porta | Protocolo | Uso | Exposição |
|---|---|---|---|
| 5000 | UDP | netcode (clientes nativos) | Service `yume-server-udp` tipo LoadBalancer (ServiceLB do k3s) |
| 5001 | UDP/QUIC | WebTransport (browser) | Service `yume-server-udp` tipo LoadBalancer (ServiceLB do k3s) |
| 5002 | TCP | WebSocket (browser) | ClusterIP `yume-server` → Ingress `/ws` |

Ingress (Traefik + cert-manager, issuer `letsencrypt-prod`, secret `yume-tls`):

- `yume.lab.thomasdev.xyz/` → `yume-client:80` (nginx, wasm estático)
- `yume.lab.thomasdev.xyz/ws` → `yume-server:5002` (o WS aceita o path `/ws` como está — **não** adicionar middleware de strip)

O certificado TLS é gerenciado por um recurso `Certificate` do cert-manager (`deploy/40-tls-certificate.yaml`) com chave RSA PKCS#8 (necessário para WebTransport). O Secret `yume-tls` é montado no pod do servidor em `/etc/yume-tls/`.

## Manifests (`deploy/`)

| Arquivo | Conteúdo |
|---|---|
| `00-namespace.yaml` | namespace `yume-vale` |
| `10-server.yaml` | Deployment server (1 réplica, `Recreate`, probes TCP no WS) + Service ClusterIP (WS) + Service LoadBalancer (UDP 5000+5001) |
| `20-client.yaml` | Deployment client (nginx) + Service |
| `30-ingress.yaml` | Ingress `/` e `/ws` com TLS (sem annotation de issuer — o Certificate 40 é quem cria) |
| `35-postgres.yaml` | StatefulSet PostgreSQL 16-alpine + ClusterIP Service |
| `36-db-sealed-secret.yaml` | Sealed Secret com a senha do banco (cifrado, commitado) |
| `40-tls-certificate.yaml` | cert-manager Certificate para `yume.lab.thomasdev.xyz` (RSA PKCS#8) |
| `argocd-application.yaml` | Application do Argo CD — aplicada **uma vez à mão** (`kubectl apply -f deploy/argocd-application.yaml`); o `directory.exclude` evita recursão |

### Ordem de sync (Argo CD sync waves)

- **Wave 1** (criado primeiro): PostgreSQL + SealedSecret + Certificate
- **Wave 2** (após wave 1): Server Deployment + Services

A ordenação garante que o banco e o certificado existam antes do servidor tentar usá-los.

## Database

Banco: **PostgreSQL 16-alpine** rodando como StatefulSet de 1 réplica (`deploy/35-postgres.yaml`).
PVC de 10Gi com `storageClassName: local-path`.

### ⚠  SEM BACKUP — RISCO DE PERDA TOTAL DE DADOS

Este é um banco **single-node, sem replicação, sem backup automatizado**. O PVC contém todos os dados dos jogadores: identidades, inventários, vínculos com criaturas, atribuições de terrenos e decorações.

**A perda do PVC resulta em perda total de todos os dados de save.**

Aceitamos este risco porque:
1. O jogo está em protótipo — o volume de dados é pequeno e nenhum jogador espera permanência
2. A complexidade operacional de backups Point-in-Time (WAL-G, pgBackRest, etc.) não se justifica nesta fase
3. O StatefulSet pode ser recriado do zero; jogadores perdem apenas progresso

### Conexão

O servidor lê a URL de conexão da variável `YUME_DATABASE_URL`, que vem do Secret `yume-db` (gerado pelo SealedSecret `deploy/36-db-sealed-secret.yaml`).

```env
YUME_DATABASE_URL=postgres://yumevale:<password>@yume-postgres:5432/yumevale
```

### Credenciais

A senha é gerada aleatoriamente (32 caracteres alfanuméricos) e selada via Sealed Secrets. O ciphertext é commitado em `deploy/36-db-sealed-secret.yaml`; o plaintext **nunca** é armazenado no repositório, disco, logs ou clipboard após a selagem.

Para regenerar a senha, siga as instruções no próprio arquivo `deploy/36-db-sealed-secret.yaml`.

## Variáveis de ambiente

| Variável | Onde | Efeito |
|---|---|---|
| `YUME_SERVER_ADDR` | cliente nativo | endereço do servidor (default `127.0.0.1:5000`) |
| `YUME_CLIENT_ID` | cliente nativo | sobrescreve o client_id (default: aleatório por instância) |
| `YUME_SERVER_WS_URL` | build Docker do cliente | URL WS embutida no wasm (vazio = derivar do host) |
| `YUME_TLS_CERT` | server (env via Deployment) | caminho para o certificado PEM (produção: `/etc/yume-tls/tls.crt`) |
| `YUME_TLS_KEY` | server (env via Deployment) | caminho para a chave PKCS#8 (produção: `/etc/yume-tls/tls.key`) |
| `YUME_DATABASE_URL` | server (env via Secret) | PostgreSQL connection string |
| `RUST_LOG` | server | filtro tracing (default `info`) |

## Operação

- **Rollback**: `git revert` do commit de pin (ou `kubectl rollout undo deployment/yume-server -n yume-vale`)
- **Logs**: `kubectl logs -n yume-vale deploy/yume-server -f`
- **Estado do sync**: Argo CD UI / `argocd app get yume-vale`
- **Validação local dos manifests**: `./yume-vale.sh infra check` (rota ansible + kubectl dry-run)
- O server é single-replica de propósito (um mundo autoritativo por processo); o pod reinicia em cada deploy (`Recreate`, sem rolling)
