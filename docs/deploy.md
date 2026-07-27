# Deploy — Yume Vale

Produção: **https://yume.lab.thomasdev.xyz** — k3s + Argo CD (GitOps). Todo push em `main` vai ao ar automaticamente.

## Pipeline

```
push em main
  └─ GitHub Actions (.github/workflows/images.yml)
       ├─ build: Dockerfile.server → ghcr.io/thomasgroch/yume-vale-server:{latest,sha-<commit>}
       └─ build: Dockerfile.client → ghcr.io/thomasgroch/yume-vale-client:{latest,sha-<commit>}
  └─ job rollout: sed pin sha-<commit> em deploy/10-server.yaml e deploy/20-client.yaml
       └─ commit "deploy: pin images ..." [skip ci] → push
  └─ Argo CD (Application yume-vale, path deploy/, automated+prune+selfHeal)
       └─ rollout dos pods no namespace yume-vale
```

O pin do sha é necessário porque `:latest` nunca muda o manifest — sem diff o Argo CD não faz rollout. O `[skip ci]` evita loop do workflow.

## Imagens

| Imagem | Base final | Conteúdo |
|---|---|---|
| `yume-vale-server` | distroless-ish (binário server) | game server headless, 3 listeners |
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
| 5001 | UDP/QUIC | WebTransport (dev local apenas) | não exposta em produção |
| 5002 | TCP | WebSocket (browser) | ClusterIP `yume-server` → Ingress `/ws` |

Ingress (Traefik + cert-manager, issuer `letsencrypt-prod`, secret `yume-tls`):

- `yume.lab.thomasdev.xyz/` → `yume-client:80` (nginx, wasm estático)
- `yume.lab.thomasdev.xyz/ws` → `yume-server:5002` (o WS aceita o path `/ws` como está — **não** adicionar middleware de strip)

## Manifests (`deploy/`)

| Arquivo | Conteúdo |
|---|---|
| `00-namespace.yaml` | namespace `yume-vale` |
| `10-server.yaml` | Deployment server (1 réplica, `Recreate`, probes TCP no WS) + Service ClusterIP (WS) + Service LoadBalancer (UDP) |
| `20-client.yaml` | Deployment client (nginx) + Service |
| `30-ingress.yaml` | Ingress `/` e `/ws` com TLS |
| `argocd-application.yaml` | Application do Argo CD — aplicada **uma vez à mão** (`kubectl apply -f deploy/argocd-application.yaml`); o `directory.exclude` evita recursão |

## Variáveis de ambiente

| Variável | Onde | Efeito |
|---|---|---|
| `YUME_SERVER_ADDR` | cliente nativo | endereço do servidor (default `127.0.0.1:5000`) |
| `YUME_CLIENT_ID` | cliente nativo | sobrescreve o client_id (default: aleatório por instância) |
| `YUME_SERVER_WS_URL` | build Docker do cliente | URL WS embutida no wasm (vazio = derivar do host) |
| `RUST_LOG` | server | filtro tracing (default `info`) |

## Operação

- **Rollback**: `git revert` do commit de pin (ou `kubectl rollout undo deployment/yume-server -n yume-vale`)
- **Logs**: `kubectl logs -n yume-vale deploy/yume-server -f`
- **Estado do sync**: Argo CD UI / `argocd app get yume-vale`
- O server é single-replica de propósito (um mundo autoritativo por processo); o pod reinicia em cada deploy (`Recreate`, sem rolling)
