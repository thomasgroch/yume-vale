# Yume Vale — Pendências (Auditoria 27/07/2026)

Auditoria baseada no plano [`yume-vale-plan.md`](./yume-vale-plan.md), confrontado com o código fonte atual.
**Nota:** esta lista não reflete ordem de prioridade. Itens agrupados por afinidade temática.

---

## Já implementado (não listado)

Os seguintes sistemas/recursos do plano estão funcionais e **não aparecem** no backlog:

- Movimento (Tnua + Avian3d), física, pulo, corrida
- Raposa animada (GLB + rig + estados de animação), arena "Ruínas de Cristal" com layout determinístico, decorações
- 3 listeners (UDP 5000, WT 5001, WS 5002) no mesmo processo
- Snapshot interpolation (`PlayerPosition` com `add_linear_interpolation`)
- Atribuição de `PlayerColor` round-robin pelo servidor
- Touch auto-detectado (joystick virtual + botão de pulo)
- CI/CD completo (GitHub Actions → ghcr.io → k3s + Argo CD)
- Testes de integração (`integration.rs`: spawn, cores distintas, reconexão)
- Reconexão resiliente com backoff de 2s (`retry_connect_when_disconnected`)
- Teste de dedup na reconexão (`reconnect_same_id_leaves_single_player`)

---

## 1. Fundações parcialmente implementadas

### 1.1 Coleta de recursos — spawn, respawn, inventário

- [ ] **`ResourceKind` e `Inventory` existem em `game_core`, mas nenhum sistema os usa em gameplay.**
  - O que existe: `ResourceKind` (8 variantes), `Inventory` (slots, add/remove, max_stack), `InventoryChanged` event.
  - O que falta: nenhum crate `features/resources/` existe. Nenhuma entidade de recurso é spawnada no mundo. Não há lógica de coleta, spawn, respawn ou integração com o inventário. O inventário nunca é modificado por uma ação de jogo.

- [ ] **`read_keyboard_input` produz `ActionKind::Collect`, mas `gather_input` descarta o valor.**
  - A linha `let (movement, run, _action)` em `gather_input` usa `_action` — a ação é lida do teclado e imediatamente ignorada.
  - O `ClientInput` enviado pela rede não carrega action/interact.

- [ ] **`PlayerInput` possui `interact` e `action`, mas o servidor os fixa em `None`.**
  - `apply_input_to_player` no servidor grava `interact: None, action: None` no `ReplicatedPlayerInput`.
  - `process_actions` roda em `FixedUpdate` mas o gatilho `ActionStarted` nunca é disparado porque `action` é sempre `None`.

### 1.2 Transmissão de ações/interações

- [ ] **Cadeia action/interact incompleta: `ActionKind`/`PlayerInput`/`ActionStarted` existem, mas `ClientInput` (mensagem de rede) não os transporta.**
  - O que existe: `ActionKind` (10 variantes), `PlayerInput` (com `interact` e `action`), `ActionStarted` event, `process_actions` system.
  - O que falta: `ClientInput` na `game_protocol` só tem `tick, move_x, move_z, run, jump`. Não há campo `action` ou `interact`. O servidor não recebe intenções de ação do cliente, logo nunca as aplica. Toda a cadeia de "cliente aperta F → coleta um recurso → inventário é atualizado" está quebrada no primeiro elo.

### 1.3 `world.ron` — schema, carregamento, validação

- [ ] **`assets/world.ron` existe com dados de recursos, criaturas e quests, mas nenhum código Rust o carrega.**
  - O arquivo contém: 3 resources (Wood, Crystal, Berry), 2 creatures (Fluffball, Glimmerwing), 1 quest ("A Berry Good Start").
  - Nenhum crate possui um loader, parser ou validação para este RON. Não há `WorldConfig` struct, `WorldPlugin`, `AssetLoader` personalizado, ou sequer um `include_str!` + `ron::from_str`.
  - O schema implícito contém tipos de criaturas, objetivos e recompensas que não possuem estruturas Rust correspondentes. Como o arquivo não é desserializado, esses valores não são verificados.

### 1.4 Modelo de estados Loading/Menu/InGame

- [ ] **O plano prevê três estados (`Loading, Menu, InGame`); o código implementa apenas `Menu` e `Playing` (via `AppFlow`).**
  - Não há estado `Loading`. A transição Menu→Playing é imediata (botão "Jogar" → `start_connection`), sem uma fase explícita de carregamento.

---

## 2. Gameplay — não iniciado

- [ ] **Recursos: crate `features/resources/` inexistente.**
  - Sem spawn de recursos no mundo, sem coleta, sem respawn, sem HUD de inventário.

- [ ] **Criaturas: crate `features/creatures/` inexistente.**
  - `CreatureId` existe em `game_core`, mas não há `CreatureKind`, componentes, sistemas, modelos ou comportamento de criaturas. Sem criaturas no mundo, cuidado, alimentação ou crescimento.

- [ ] **Housing: crate `features/housing/` inexistente.**
  - Sem terreno, construção, decoração.

- [ ] **Quests: crate `features/quests/` inexistente.**
  - O quest do `world.ron` ("A Berry Good Start") não é lido nem processado. Sem sistema de missões, progressão ou recompensas.

- [ ] **Social/chat/emotes/grupos/colaboração: crate `features/social/` inexistente.**
  - Sem chat, emotes, formação de grupo, ou qualquer interação social além da presença compartilhada.

---

## 3. Networking / Produção — lacunas

- [ ] **`ServerConfig.max_players` está definido mas nunca é verificado.**
  - O campo existe em `ServerConfig` e recebe o valor `MAX_PLAYERS` (16) por padrão, mas nenhum sistema impede a conexão do 17º jogador. `on_client_connected` cria o jogador incondicionalmente.

- [ ] **`SIGHT_RANGE` (30.0) está definido em `game_core::constants` mas nunca usado.**
  - Nenhum sistema de interest management filtra entidades replicadas por distância. Todas as entidades são replicadas para todos os clientes via `NetworkTarget::All`.

- [ ] **`INTERACT_COOLDOWN_S` (0.5) está definido em `game_core::constants` mas nunca usado.**
  - Nenhum rate limiting de ações existe no servidor ou cliente.

- [ ] **Protocol ID existe e é enviado, mas não há teste ou tratamento de incompatibilidade.**
  - `PROTOCOL_ID` é enviado via `Authentication::Manual`. Mas não há teste de rejeição de protocolo versão diferente, nem mensagem de erro amigável para o cliente. A integração futura mencionada no plano (`reconexão e incompatibilidade de protocolo`) não foi implementada.

- [ ] **WebTransport em produção não implementado.**
  - O deploy (`deploy/`) só expõe WS (porta 5002) via Ingress Traefik. WT (5001) só funciona em dev local com certificado auto-assinado.

- [ ] **Fallback automático WT→WS ausente.**
  - O cliente escolhe WT ou WS estaticamente com base no ambiente (`?transport=ws` manual ou host não-local). Não há tentativa WT com fallback automático para WS em caso de falha.

---

## 4. Testes / Qualidade — lacunas

- [ ] **Testes de persistência: nenhum.**
  - Não há código de persistência (nem SQLite nem PostgreSQL) para testar.

- [ ] **Testes de validação de assets/configs: nenhum.**
  - `world.ron` não é testado quanto a schema, valores válidos ou consistência.

- [ ] **Testes de incompatibilidade de protocolo: nenhum.**
  - Não há teste que conecte um cliente com `PROTOCOL_ID` diferente e verifique rejeição.

- [ ] **Teste de integração do fluxo de input: incompleto.**
  - O plano afirma que `integration.rs` cobre aplicação de input, mas esse arquivo testa conexão, replicação, cores, interpolação e deduplicação de reconexão. `apply_input_to_player` possui testes unitários; falta verificar o envio e a aplicação de `ClientInput` entre cliente e servidor no teste em memória.

- [ ] **Simulação com checksum determinístico: não implementada.**
  - O plano menciona "Seed + estado inicial + inputs → checksum esperado" como desejável. Os layouts de arena/decoração são determinísticos, mas não há teste que reproduza uma simulação a partir de estado e inputs e valide seu checksum.

- [ ] **Testes de aplicação do limite `max_players`: nenhum.**
  - Não há teste que tente conectar 17 clientes e verifique bloqueio.

- [ ] **Testes de rate limiting: nenhum.**
  - Não há cooldown nem teste de limitação de ações/interações.

---

## 5. Opcional / Roadmap futuro

- [ ] **`cargo xtask`** — O plano menciona como opção futura se `yume-vale.sh` crescer. Hoje o shell script atende.
- [ ] **Client-side prediction** — O plano afirma explicitamente que não é requerido pela arquitetura atual (`sem prediction`). Opcional para redução de latência percebida.
- [ ] **Persistência SQLite → PostgreSQL** — Planejado para acompanhar as features de gameplay. Nenhum código de banco existe.

---

## Notas

- A auditoria considera apenas o que está descrito no `yume-vale-plan.md` e o que existe no repositório em 27/07/2026.
- Itens marcados como "não iniciado" podem ter tipos/base no `game_core` (ex.: `ResourceKind`, `CreatureId`) mas carecem de qualquer sistema, plugin, entidade ou integração gameplay.
- A quebra na cadeia de ação (cliente → protocolo → servidor → efeito) é a lacuna funcional mais crítica para as features de gameplay futuras.
