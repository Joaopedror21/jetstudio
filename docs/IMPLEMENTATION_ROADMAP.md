# Roadmap de implementação

> **Status:** vinculante, a partir de 23/10/2026. Substitui a ordem de marcos de `FL_STUDIO_ROADMAP.md` (Codex), que continua sendo referência de escopo e de portas de saída por área. As mudanças de ordem estão justificadas em `CLAUDE_REVIEW.md` D2 e D3.
> Prazos não são estimados em horas: o ritmo real dos agentes neste projeto ainda não foi medido.

## Visão geral

| Marco | Conteúdo | Depende de | Corresponde no roadmap do Codex |
|---|---|---|---|
| **R0** (contínuo) | Perfil de referência, ferramenta de captura, capturas dos cenários do M1, medições | instalação do FL (Q1) | M0 |
| **M1** | Interface principal: shell, toolbar, Browser, Channel Rack, Playlist básica, transporte silencioso, gerenciador de janelas | — (as medidas chegam via R0) | parte de M1 + parte de M3 |
| **M2** | Engine de áudio: CPAL, sampler interno, sequenciador de passos e Pattern Clips, Master, render offline | M1 | restante de M1 + parte de M4 |
| **M3** | Piano roll | M2 | M2 |
| **M4** | Playlist completa: Audio Clips, Picker, Make unique, Track Mode, marcadores, múltiplos Arrangements | M2 | M3 |
| **M5** | Mixer: inserts, sends, slots de efeito, efeitos internos, PDC, medidores | M2 | M4 |
| **M6** | Browser completo, MIDI de hardware, automação | M4, M5 | M5 |
| **M7** | Hospedagem de plugins (CLAP, depois VST3) | M5 | M6 |
| **M8** | Profundidade, desempenho de projetos grandes, interoperabilidade, outras plataformas | — | M7 |

## M1 — Interface principal (detalhado)

**Objetivo.** Uma janela que se comporta como o shell do FL Studio no perfil `win-100`, com modelo, comandos, histórico e arquivo **reais**, e sem som. Tudo o que não é real aparece desabilitado.

### Incrementos (cada um integrado à `main` separadamente)

| # | Incremento | Entregáveis | Porta de saída |
|---|---|---|---|
| M1.0 | Bootstrap | `git init`, workspace, `rust-toolchain.toml`, `xtask` (ci, layers, assets, parity), `AGENTS.md`, `CLAUDE.md`, `.gitignore`, `docs/tasks/ACTIVE.md`, `ATTRIBUTION.md` | `cargo xtask ci` verde num workspace com `jet-time` mínimo; o gate de layers rejeita uma dependência ascendente plantada num teste |
| M1.1 | Núcleo headless | `jet-time`, `jet-model`, `jet-format`, `jet-engine` (comandos, histórico com `toggle_last`/`undo_step`, transporte com `WallClock`), `jet-cli` (`run --cmd`, `inspect`) | testes de propriedade de tempo; round-trip do formato; cada comando com teste normal, de undo e de desabilitado; um roteiro CLI cria canais e patterns, liga passos, salva, reabre e reproduz o mesmo `inspect` |
| M1.2 | Shell e janelas | `jet-ui`: MainWindow sem decoração do SO, toolbar dirigida por dados (com espaços reservados), MenuBar com `catalog/menus.txt` (itens sem comando desabilitados), Hint Bar, `Workspace` (flutuantes, z-order, foco, mover, redimensionar, maximizar, fechar, encaixe com Alt para desligar), atalhos de janela, LayoutProbe, `ui.inspect`, `xtask shot` | testes de modelo do `Workspace` (sequência aleatória de 1.000 ações mantém as invariantes); testes de Harness com ponteiro real para mover, encaixar e maximizar; captura offscreen lida e revisada |
| M1.3 | Widgets primitivos | `Knob`, `Slider`, `Lcd`, `ToolbarButton`, `ToggleButton`, `StepButton`, `Menu`/`ContextMenu`, todos com máquinas de estados e conectados à Hint Bar | cada convenção de `FINAL_ARCHITECTURE.md` §4.4 tem um teste de evento real; testes de mutação na camada A para o primeiro widget medido |
| M1.4 | Transporte e toolbar | PAT/SONG (L; clique direito abre janela), Play/Stop (Espaço), Pause (Ctrl+Espaço), Tempo LCD (10–522, fino 1/1000), Time panel (B:S:T e M:S:CS), Pattern panel (+/-, F4, F2), Global Snap (valores do manual) | o estado do transporte e a posição musical correta ao longo do tempo (teste com relógio falso injetado); o Time panel formata conforme o PPQ e a fórmula de compasso do projeto |
| M1.5 | Channel Rack | lista de canais, passos (esquerdo liga, direito desliga), mute, knobs de volume/pan, LCD de destino no mixer (só valor no modelo), seleção, renomear, reordenar, comprimento do pattern, swing (valor no modelo), alternar pattern sem perder dados | alternar patterns preserva os passos por canal; um gesto contínuo vira um passo de undo; salvar e reabrir preserva tudo |
| M1.6 | Playlist básica | régua, faixas (nome, mute), Pattern Clips (inserir, mover, redimensionar, apagar), snap global e local "Main", zoom e scroll com virtualização, playhead em SONG, um Arrangement | duas instâncias do mesmo pattern apontam para a mesma fonte; snap e Alt (sem snap) testados; porta de desempenho (abaixo) |
| M1.7 | Browser | acoplado à lateral, árvore de pastas **adicionadas pelo usuário** (somente leitura), filtro, arraste de arquivo de áudio para o Channel Rack (cria um canal Sampler com `ResourceRef`) | um arquivo ausente gera erro legível; o arraste cria uma entidade real; preview de áudio **desabilitado** até o M2 |
| M1.8 | Passe de fidelidade | specs medidas do R0 para toolbar, janelas, Channel Rack, Playlist e Browser; relatórios C; correções; scorecard | scorecard gerado; nenhuma superfície declarada "aprovada em C" sem revisão do Claude |

### Porta de desempenho do M1 (decisão nossa, medida na máquina de dev, que deve ser registrada)

O cenário sintético é: Channel Rack com 64 canais × 64 passos visíveis, mais Playlist com 100 faixas e 2.000 clipes (500 visíveis), a 1920×1080 e 1×, em build release. A meta é **tempo de CPU por frame p95 ≤ 8 ms e p99 ≤ 16 ms** durante scroll e zoom contínuos. Se a meta falhar, ativa-se a rota de *paint callback* wgpu para a camada densa (`FINAL_ARCHITECTURE.md` §4.1). A troca de framework não é uma opção.

### Fora do M1 (aparece desabilitado)

Áudio, preview, medidores, CPU e Output panels; Piano roll (F7); Mixer (F9); Plugin Picker (F8); janelas Detached; gravação; Audio e Automation Clips; múltiplos Arrangements na UI; MCP; `.flp`.

## R0 — Trilha de referência (em paralelo)

1. O usuário confirma Q1 e Q2 (`CLAUDE_REVIEW.md` §7).
2. O Codex implementa `tools/capture` e `cargo xtask measure` (`UI_FIDELITY_STRATEGY.md` §3.2 e §4.1), sem precisar do FL para desenvolver: testa com capturas do próprio JetStudio.
3. O usuário configura o perfil `win-100` e executa os roteiros de `fidelity/scenarios/` do M1.
4. O Codex mede e preenche `fidelity/spec/*.ron`, e o Claude audita amostras: cada medida é reconferida com edge-snap numa segunda captura do mesmo estado.
5. Transcrição dos menus de topo para `catalog/menus.txt`, com origem por linha.

## M2 em diante: portas de saída principais

- **M2:** mesmos passos geram os mesmos eventos e timestamps offline; o playback e o bounce do mesmo material são idênticos quando não há plugins externos; `assert_no_alloc` passa no callback; duração do callback e underruns medidos e registrados por buffer e taxa; troca de dispositivo e de buffer sem travar.
- **M3–M8:** as portas de `FL_STUDIO_ROADMAP.md` (Codex) para cada área continuam válidas, acrescidas, em toda superfície, das camadas A, B e C de `UI_FIDELITY_STRATEGY.md`.

## Regras transversais

- Uma superfície nova só começa depois que o R0 tiver os cenários dela **roteirizados**. A captura pode vir depois, mas o roteiro e os comportamentos a observar vêm antes.
- Cada incremento termina com um handoff (`COLLABORATION.md` §7) e uma revisão do Claude.
- Nenhum marco é "concluído" com testes vermelhos ou ignorados sem um registro em gaps.
