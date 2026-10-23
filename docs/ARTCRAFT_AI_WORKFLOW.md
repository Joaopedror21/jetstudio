# Fluxo de trabalho com agentes nas aplicações Craft

**Pesquisa em 23/10/2026.** Aqui, “ArtCraft” significa a família de aplicações `storytold/*craft` solicitada para estudo. O repositório público [`storytold/artcraft`](https://github.com/storytold/artcraft) é outro produto, voltado à criação de imagens e vídeos com IA; sua arquitetura Tauri/TypeScript não deve ser atribuída ao PhotoCraft ou ao SoundCraft. As marcas `[Fato]`, `[Inferência]` e `[Proposta]` distinguem evidência pública de interpretação e recomendação.

## Limites das evidências

- **[Fato]** O [AGENTS.md do PhotoCraft](https://github.com/storytold/photocraft/blob/main/AGENTS.md) chama `../craftrules` de repositório **não público**. O endereço fornecido, [`storytold/craftrules`](https://github.com/storytold/craftrules), retornou 404 durante esta pesquisa. Só é possível descrever as regras *citadas* pelos repositórios públicos; não foi possível ler o seu conteúdo.
- **[Fato]** O [AGENTS.md do SoundCraft](https://github.com/storytold/soundcraft/blob/main/AGENTS.md) indica `plan/STATUS.md`, `plan/execution-plan.md`, observações do Pro Tools e screenshots sob `plan/protools/`. Declara que `plan/` é ignorado pelo Git. O [AGENTS.md do PhotoCraft](https://github.com/storytold/photocraft/blob/main/AGENTS.md) também menciona `plan/` e `log/` locais e ignorados. Seu conteúdo privado, prompts, decisões diárias e eventual número de agentes simultâneos não são verificáveis nesta pesquisa.
- **[Fato]** O [`CLAUDE.md` do SoundCraft](https://github.com/storytold/soundcraft/blob/main/CLAUDE.md) contém apenas a instrução para ler `AGENTS.md`. Não é evidência de que todos os commits foram feitos com Claude Code; o repositório tampouco permite quantificar contribuição de Codex por si só.
- **[Inferência]** As convenções públicas mostram um projeto preparado para desenvolvimento paralelo por agentes e para controlar o produto por agentes. Não revelam a orquestração exata que a equipe usou historicamente.

## Ciclo público e verificável

| Passo | Prática observada | Evidência |
|---|---|---|
| Orientar a sessão | PhotoCraft orienta ler arquitetura, desenvolvimento, contribuição, protocolo de controle, design da UI, roadmap e lacunas. SoundCraft começa pelo status e plano locais e, na ausência deles, pelos documentos públicos de progresso. | [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md), [SoundCraft AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md) |
| Escolher trabalho | PhotoCraft prioriza infraestrutura, lacunas ranqueadas, scorecard, cobertura de menu e log. O documento de paridade distingue presença de recursos e prontidão real. | [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md), [target-app-parity.md](https://github.com/storytold/photocraft/blob/main/docs/target-app-parity.md) |
| Implementar uma ação | Uma ação visível é uma especificação de comando do engine, com id, rótulo, menu, atalho, parâmetros, precondição, execução e testes. UI, CLI, controle e MCP despacham o mesmo id. | [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md), [SoundCraft AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md) |
| Inspecionar o resultado | `ui.inspect` lê estado, `ui.pointer` injeta gestos, `ui.screenshot` captura a janela. PhotoCraft e SoundCraft têm renderização offscreen para inspecionar sem roubar foco. | [PhotoCraft control-protocol.md](https://github.com/storytold/photocraft/blob/main/docs/control-protocol.md), [PhotoCraft development.md](https://github.com/storytold/photocraft/blob/main/docs/development.md), [SoundCraft AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md) |
| Verificar | PhotoCraft exige testes, `clippy`, teste de dependências em camadas, WASM quando aplicável, paridade, scorecard e corpus segundo o tipo de alteração. SoundCraft usa `cargo xtask ci` para formato, lint, testes, assets, camadas e WASM. | [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md), [SoundCraft AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md) |
| Registrar | PhotoCraft manda atualizar documentos de lacunas/paridade, aumentar pisos de testes e acrescentar entrada ao log de desenvolvimento para retomada após interrupções. | [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md) |

O [FilmCraft `docs/agents.md`](https://github.com/storytold/filmcraft/blob/main/docs/agents.md) torna esse ciclo ainda mais explícito: orientar pelo documento de lacunas, definir teste de aceitação, implementar, executar `cargo xtask ci`, acionar a UI com `--control`, inspecionar screenshot, atualizar paridade/gaps e registrar o trabalho. Seu critério de conclusão cobre comando, teste de engine/undo/disabled, UI/atalho, id de automação, screenshot e gates. O documento recomenda um worktree e `CARGO_TARGET_DIR` por agente e um dono por crate. Essas são instruções públicas do projeto, não observação independente de cada sessão de agentes.

### Um exemplo concreto de contrato compartilhado

No PhotoCraft, o catálogo `menu_catalog.rs` contém nomes, ordem, separadores e atalhos dos menus. Um item com id correspondente a um comando passa a funcionar; itens ainda sem implementação permanecem desabilitados. O comando pertence ao engine, enquanto estado puro de janela ou painel pertence à UI. Isso reduz divergência entre clique, atalho, CLI e MCP, e torna possível medir a cobertura do catálogo. A descrição consta do [AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md) e do [ui-design.md](https://github.com/storytold/photocraft/blob/main/docs/ui-design.md). **[Inferência]** A economia de trabalho vem do ponto único de comportamento, não de uma geração automática de toda a função.

Um PR verificável mostra o ciclo em pequena escala: [PhotoCraft PR #141](https://github.com/storytold/photocraft/pull/141) alinhou os atalhos mostrados nos menus com os efetivamente despachados, tratou foco de campos de texto, testou múltiplos pontos de foco e reduziu de 214 para zero as falhas do seu teste de atalhos. Outro, [PR #139](https://github.com/storytold/photocraft/pull/139), introduziu estado serializável de dock, gestos e testes de layout, persistência e janelas pequenas. São exemplos de trabalho revisável, não prova de equivalência geral com Photoshop.

## Agentes como *usuários* da aplicação

Há uma diferença entre usar IA para escrever o produto e permitir que uma IA opere o produto. A segunda parte está documentada diretamente:

1. [PhotoCraft control-protocol.md](https://github.com/storytold/photocraft/blob/main/docs/control-protocol.md): canal TCP de JSON por linha, autenticação, `engine.execute`, inspeção de documento e UI, entrada sintética, screenshots e raízes de leitura/escrita para automação.
2. [SoundCraft AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md): `soundcraft --control`, render offscreen `ui_shot`, CLI para comandos e bounce. Menus, atalhos, CLI, controle e MCP usam os ids comuns.
3. [SoundCraft docs/mcp.md](https://github.com/storytold/soundcraft/blob/main/docs/mcp.md): `soundcraft-cli mcp` pode operar o engine sem janela ou conectar a uma instância aberta, quando também oferece screenshots e cliques. A página mostra como registrar o servidor no **Claude Code**. Isso confirma compatibilidade documentada com Claude Code, não um registro histórico completo do desenvolvimento.
4. [PhotoCraft corpus](https://github.com/storytold/photocraft-corpus): testes com arquivos gerados por scripts no Photoshop instalado; os resultados são fixados por commit e SHA-256. São oráculos de conteúdo/renderização para casos específicos, não uma prova visual da UI inteira.

**MCP não significa engenharia reversa de binário.** Nos repositórios estudados, MCP é uma interface *do programa reimplementado* para comandos e observação. O [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md) afirma estudo do Photoshop por comportamento e aparência, uso de especificações públicas e proibição de copiar código/assets. O [SoundCraft AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md) descreve screenshots e menus observados. Há, porém, uma distinção importante: o [PhotoCraft target-app-parity.md](https://github.com/storytold/photocraft/blob/main/docs/target-app-parity.md) relata inspeção de metadados, atalhos e **strings do binário principal** do Photoshop instalado para inventariar funcionalidades. Isso é análise estática de informações do aplicativo, **sem evidência** de Ghidra, REA, IDA ou descompilação de funções. Não atribuir essas ferramentas ao projeto.

## Desenvolvimento paralelo: fatos e limites

**[Fato]** O [AGENTS.md do PhotoCraft](https://github.com/storytold/photocraft/blob/main/AGENTS.md) pede `CARGO_TARGET_DIR=target/agent-<name>` por agente para evitar lock do Cargo, delimitação de arquivos, edições cirúrgicas em módulos compartilhados, manifesto `Cargo.toml` válido a todo momento e novo módulo de comandos por área. SoundCraft prescreve práticas equivalentes no seu [AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md). A estrutura em camadas permite tarefas em crates separadas, e as portas de teste reduzem conflitos de integração.

**[Limite]** O percentual de trabalho paralelizável e as estimativas em “agent-hours” no [PhotoCraft target-app-parity.md](https://github.com/storytold/photocraft/blob/main/docs/target-app-parity.md) são estimativas dos mantenedores. Não devem ser convertidas em uma afirmação sobre tamanho exato da equipe, custo ou produtividade esperada para o FL Studio.

## Proposta de divisão Codex e Claude Code para a nossa reimplementação

Esta seção é uma **proposta**, não descrição do que a equipe Craft fez.

| Responsabilidade | Codex | Claude Code | Revisão conjunta |
|---|---|---|---|
| Evidência da referência | Manter matriz de comportamentos, capturas e testes observacionais da versão fixada. | Reproduzir protocolos da referência de forma independente em casos ambíguos. | Resolver divergências antes de codificar. |
| Fundação Rust | Crates de modelo, tempo musical, comandos e persistência; contratos e testes. | Revisão de interfaces, invariantes e riscos de concorrência; implementação de módulos separados. | Um responsável por cada manifesto e API pública; CI obrigatório. |
| Áudio | Scheduler e mixer interno isolados por crate, com testes offline. | Auditoria de callback em tempo real, alocação/locks, plugins e falhas. | Render offline, loopback, testes de latência e dropout. |
| UI | Estado serializável, design tokens, Channel Rack/Playlist/Piano Roll por superfícies distintas. | Testes de interação, atalhos, acessibilidade e revisão visual de outra superfície. | Capturas nas mesmas condições e checklist de comportamento. |
| Integração | Integrar pequenos PRs e medir cobertura de comandos/fluxos. | Revisar casos adversos e lacunas documentadas. | Declarar pronto apenas com evidência executável e limites registrados. |

### Contrato de tarefa recomendado

Cada tarefa deve declarar: (1) versão e comportamento de referência, (2) entradas e saídas observáveis, (3) arquivos/crate sob responsabilidade do agente, (4) id de comando, se houver, (5) definição de concluído com teste do engine e, para UI, comparação de estado/captura, (6) limites conhecidos, (7) URL oficial ou evidência interna autorizada. A integração só ocorre após revisão da função concreta, nunca só porque um item de menu passou de cinza a ativo.

### Handoff recomendado

Manter `STATUS.md` público do projeto, mapa de crates, catálogo de comandos, matriz de paridade por fluxo, log de decisões com fonte e conjunto de fixtures próprios. Um agente encerra com: hash de commit, arquivos alterados, testes executados, resultados, screenshots/áudio de prova, lacunas e próximo passo. Este processo adapta as práticas visíveis no [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md), mas a distribuição específica Codex/Claude é nossa.

## Perguntas ainda não respondidas por fontes públicas

- Prompts e sequência real dos agentes fundadores; conteúdo do `craftrules` e dos `plan/` ignorados.
- Processo original de medição de pixels, fontes e geometrias da UI proprietária; o código mostra *como implementam e testam*, não todos os passos pelos quais chegaram aos valores.
- Uso histórico exato de Codex, Claude Code e outras ferramentas por commit.
- Qualquer suposto uso de Ghidra ou REA: não confirmado por evidência pública primária consultada; a inspeção estática de strings/metadados do Photoshop pelo PhotoCraft é descrita separadamente acima.
