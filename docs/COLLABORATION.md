# Colaboração Codex ↔ Claude e estratégia de Git

> **Status:** vinculante, a partir de 23/10/2026. `AGENTS.md` (Codex) e `CLAUDE.md` (Claude Code), na raiz, devem apontar para este arquivo. Eles serão criados na primeira tarefa (`CODEX_FIRST_TASK.md`).
> **Origem:** adapta práticas **verificadas** no ArtCraft (um `CARGO_TARGET_DIR` por agente, worktree por agente, um dono por crate, gates antes do commit, critério de "pronto" do FilmCraft em `docs/agents.md` e `docs/contributing.md` §5). A divisão de papéis é decisão nossa.

## 1. Papéis

| Papel | Responsável | Faz | Não faz |
|---|---|---|---|
| **Implementação** | Codex | código, testes, integração, otimização, ferramentas (`xtask`, `tools/capture`, `measure`), geração de relatórios | alterar decisões de `FINAL_ARCHITECTURE.md` sem um ADR revisado |
| **Arquitetura e auditoria** | Claude | ADRs, revisão de cada branch que toca contratos (modelo, comandos, formato, camadas, tokens), auditoria de fidelidade (relatórios C), revisão de concorrência/RT, documentos de decisão | aprovar a própria implementação: quando o Claude implementa, o Codex revisa |
| **Referência** | Usuário | operar o FL Studio licenciado, executar capturas, responder às questões abertas, aprovar pushes e publicação | — |
| **Ambos** | — | implementar módulos específicos, **desde que em arquivos diferentes** e com posse registrada | editar o mesmo arquivo ao mesmo tempo |

## 2. Posse de arquivos (como evitar edição simultânea)

- `docs/tasks/ACTIVE.md` é a **fonte única** de quem está fazendo o quê. Cada linha registra: `task-id | agente | branch | caminhos reservados (globs) | início | estado`.
- Antes de editar, o agente **lê** `ACTIVE.md` e reserva os caminhos num commit pequeno na `main`, contendo só essa linha. Se o caminho já estiver reservado por outro agente, **não edita**: pede liberação por meio do usuário ou escolhe outra tarefa.
- **Arquivos quentes** (alta chance de conflito) não são reservados por tarefas longas. Quem precisa deles faz uma alteração mínima, num commit separado, integrada logo:
  - `Cargo.toml` raiz e `Cargo.lock`;
  - `crates/jet-engine/src/cmd/mod.rs` (registro de módulos de comandos: **uma linha por módulo**; cada área tem o próprio arquivo `cmd/<área>.rs`, como no SoundCraft);
  - `catalog/menus.txt`, `fidelity/spec/tokens.ron`;
  - `AGENTS.md`, `CLAUDE.md`, este arquivo e `FINAL_ARCHITECTURE.md` (este último só via ADR).
- Em caso de conflito no `Cargo.lock`, nunca resolver à mão: aceitar o da `main` e rodar `cargo update -w`, ou o build, para regenerar.

## 3. Git

### 3.1 Preparação (uma vez)

A pasta **ainda não é um repositório Git**. A primeira tarefa faz `git init`, `.gitignore` (inclui `target/`, `reference/`, `*.local.*`) e o primeiro commit com `docs/`. **Push para um remoto só com autorização explícita do usuário** (Q4 em `CLAUDE_REVIEW.md`).

### 3.2 Branches e worktrees

```text
main                     sempre compila e passa `cargo xtask ci`; ninguém commita trabalho em andamento aqui
codex/<task-id>-<slug>   trabalho do Codex
claude/<task-id>-<slug>  trabalho do Claude
```

```sh
# cada agente no seu worktree e com o seu target dir (padrão FilmCraft/PhotoCraft)
git worktree add ../jetstudio-codex  -b codex/M1-1-model
git worktree add ../jetstudio-claude -b claude/ADR-008-history
# no worktree:
export CARGO_TARGET_DIR=target/agent-codex     # PowerShell: $env:CARGO_TARGET_DIR="target/agent-codex"
```

### 3.3 Ciclo de uma tarefa

1. Reservar caminhos em `ACTIVE.md` (commit na `main`).
2. Criar branch e worktree a partir da `main` atual.
3. Fazer commits pequenos, um tema por commit, cada um compilando. As mensagens seguem `<área>: <o quê>` com o `task-id` no corpo.
4. Antes de pedir revisão: `git fetch` (se houver remoto) e `git rebase main`, e então `cargo xtask ci` verde.
5. Revisão: o outro agente revisa (§5). Os achados são corrigidos na mesma branch.
6. Integração: `git checkout main && git merge --no-ff <branch>`, com nova execução de `cargo xtask ci` na `main`. Em seguida, liberar os caminhos em `ACTIVE.md`.
7. Remover o worktree e o target dir do agente (`git worktree remove`), porque o disco enche.

### 3.4 Proibições

- `git push --force` em `main`; `git reset --hard`, `git clean -fdx` ou `git checkout -- .` em worktree que não é seu.
- `--no-verify` para pular hooks.
- Commitar `reference/`, capturas, binários do FL, `.env` ou tokens.
- Reescrever histórico já integrado à `main`.
- Commits gigantes que misturam refatoração e funcionalidade.

### 3.5 Atribuição

Os commits terminam com o trailer do agente autor (`Co-Authored-By: …`). Isso permite medir depois quem fez o quê, como o histórico do ArtCraft permite (`CLAUDE_REVIEW.md` E4).

## 4. Contrato de tarefa

Toda tarefa em `docs/tasks/<task-id>.md` contém:

1. **Objetivo** e milestone.
2. **Evidência:** página do manual ou `capture_id` de cada comportamento, e o que é `[Pendente]`.
3. **Caminhos reservados.**
4. **Comandos** (ids) criados ou alterados.
5. **Definição de pronto** (§6).
6. **Fora de escopo**, explicitamente.
7. **Riscos e perguntas.**

## 5. Revisão

| Tipo de mudança | Revisor obrigatório | Foco |
|---|---|---|
| Modelo, formato, comandos, histórico, camadas, concorrência | Claude | invariantes, compatibilidade do formato, determinismo, erros |
| UI com efeito visual | Claude (fidelidade) | relatório A/B/C antes e depois; nenhum número sem origem |
| Código escrito pelo Claude | Codex | correção, testes, aderência ao estilo |
| Ferramentas (`xtask`, captura) | qualquer um | reprodutibilidade |

A revisão produz uma lista de achados, cada um com arquivo:linha, cenário de falha e severidade, registrada no fim do arquivo da tarefa.

## 6. Definição de pronto (vale para qualquer funcionalidade)

- [ ] O comando está registrado com id, rótulo, caminho de menu, atalho (quando houver), documentação de parâmetros e `enabled` com motivo.
- [ ] Testes de engine cobrem o caso normal, o undo/redo e o caso desabilitado.
- [ ] A UI está ligada a menu, controle e atalho, e todo widget novo tem `WidgetId` registrado no LayoutProbe.
- [ ] Foi acionado pelo Harness ou pelo canal de controle, e a captura foi **olhada** (o PNG lido de fato).
- [ ] Elementos com spec `Measured` passam na camada A; os ainda não medidos estão marcados `Unmeasured`.
- [ ] `cargo xtask ci` está verde.
- [ ] Scorecard, gaps e `ACTIVE.md` foram atualizados, e o relatório de handoff foi escrito (§7).

## 7. Handoff (fim de cada sessão ou tarefa)

```text
task-id / branch / commit(s):
arquivos alterados:
comandos executados e resultado (cole a saída final do `cargo xtask ci`):
capturas/relatórios gerados (caminhos locais):
o que ficou parcial ou ausente, com motivo:
decisões tomadas que precisam de ADR:
próximo passo sugerido:
```

## 8. Documentação

- Marcações obrigatórias de evidência: **[Manual: página]**, **[Observado: capture_id]**, **[Medido: capture_id + método]**, **[ArtCraft-verificado: caminho]**, **[Hipótese]**, **[Decisão]**.
- Todo documento de progresso tem cabeçalho com `Última revisão`, data e versão-alvo, e uma tabela de revisões no fim (padrão SoundCraft).
- Percentuais só com método: "medido" (teste ou script reexecutável) ou "estimado". Nunca subir um número sem evidência que um revisor consiga reproduzir (regra do FilmCraft, `docs/agents.md`).
- Os documentos de pesquisa do Codex (`ARTCRAFT_*`, `FL_STUDIO_*`, `TECH_STACK_DECISION`, `UI_REPLICATION_METHOD`) são **históricos**: não são reescritos, só recebem notas de correção no topo.
