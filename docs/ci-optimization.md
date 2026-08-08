# CI — Diagnóstico e Plano de Otimização

Baseado no run [31239023857](https://github.com/thomasgroch/yume-vale/actions/runs/31239023857): `check` ok (7m37s), `wasm` falhou, `build (client/server)` cancelado manualmente aos ~24min (443 crates recompilados do zero, confirmado no log).

## ❌ 1. Docker build (client/server) recompila tudo do zero sempre — causa do travamento de 24min+

- **Problema:** `Dockerfile.server`/`Dockerfile.client` fazem `COPY . .` **antes** de `cargo build`/`trunk build`. Qualquer commit muda algum arquivo → a camada `RUN cargo build` nunca bate cache, mesmo com `cache-from/cache-to: type=gha` configurado no workflow. Nenhum cache mount de registry/target. sccache não entra no container. Resultado: **443 crates do zero em todo push**, sem exceção.
- **Solução:** padrão `cargo-chef` (separa uma camada "receita de dependências" do código-fonte) + `RUN --mount=type=cache` para `/usr/local/cargo/registry` e `/app/target` (copiando o binário pra fora do mount antes do estágio final, já que cache mounts não persistem na imagem).
- **Ganho:** builds com `Cargo.lock` inalterado só recompilam os crates do workspace que mudaram — minutos, não 24+.

## ❌ 2. Job `wasm` falha: `getrandom v0.4.3` sem feature `wasm_js`

- **Causa raiz:** `rand v0.10.2` (via `bevy_math`, atualizado recentemente) trouxe uma nova *major* do `getrandom` (0.4.x) que exige opt-in explícito para wasm32. A 0.3.x já está corretamente configurada em [`crates/game_client/Cargo.toml:29`](../crates/game_client/Cargo.toml), mas Cargo **não unifica features entre majors diferentes** (0.3 e 0.4 contam como crates distintos) — logo o mesmo problema reaparece para a 0.4.
- **Solução:** 1 linha nova na seção `[target.'cfg(target_arch = "wasm32")'.dependencies]`:
  ```toml
  getrandom_v04 = { package = "getrandom", version = "0.4", features = ["wasm_js"] }
  ```
- **Risco:** nenhum — só afeta o alvo wasm32, zero mudança de comportamento nativo.

## ⚠️ 3. Step "PostgreSQL" no CI nunca toca o Postgres (achado não solicitado, mas crítico)

- **Problema:** o step define `YUME_TEST_DATABASE_URL`, mas **nenhum código do workspace lê essa variável** (confirmado via grep). Todo teste de `game_persistence` sempre usa `sqlite://<tempdir>` (helper `with_worker`). O serviço Postgres sobe, espera healthcheck, e **os mesmos 16 testes rodam de novo contra SQLite** — tempo de CI jogado fora, sem cobertura real adicional.
- **Contexto de produção:** o `game_server` real usa **Postgres** em produção (`YUME_DATABASE_URL` em `deploy/10-server.yaml` → StatefulSet `yume-postgres`). Ou seja: hoje a suíte **nunca** exercita o banco que roda em produção.
- **Sobre o Jazz:** usado só em `apps/admin` (CRDT sync para o painel administrativo) — não tem relação com `game_persistence`/SQL do servidor de jogo. Não substitui nem sobrepõe essa camada. **O teste SQLite continua fazendo sentido** como teste rápido de contrato.
- **Decisão pendente (ver pergunta ao usuário):** consertar o step pra rodar de fato contra Postgres, ou remover o step e confiar que `sqlx::Any` abstrai bem as diferenças entre drivers.

## 🗂️ 4. Cache — estado atual

| Job | Cache hoje | Avaliação |
|---|---|---|
| `check` | sccache (GHA backend) + `rust-cache` (`cache-targets: false`) | ✅ correto |
| `wasm` | só `rust-cache` (sem sccache) | ⚠️ funcional, mas inconsistente com `check` |
| `build` (Docker) | `cache-from/to: type=gha` nas camadas Docker | ❌ inútil hoje (ver item 1) — corrigido pelo item 1 |

## ❓ Perguntas antes de agir

1. **Step Postgres:** conserta pra rodar de verdade (mais fidelidade, mais tempo de CI) ou remove (mais rápido, sem cobertura do banco de produção)?
2. **cargo-chef:** ok introduzir essa dependência de build no Dockerfile pra resolver o cache de camadas? É o padrão mais robusto para workspace multi-crate (evita listar Cargo.toml de cada crate manualmente).
3. **sccache no job `wasm`:** vale padronizar (ganho pequeno) ou não compensa a complexidade extra?
