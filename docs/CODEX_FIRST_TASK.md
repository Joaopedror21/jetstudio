# Tarefa para o Codex — Milestone 1: interface principal

> **Emitida por:** Claude (arquitetura) · **Data:** 23/10/2026 · **Task-id:** `M1`
> **Leia antes de começar, nesta ordem:** `CLAUDE_REVIEW.md` → `FINAL_ARCHITECTURE.md` → `UI_FIDELITY_STRATEGY.md` → `COLLABORATION.md` → `IMPLEMENTATION_ROADMAP.md` (M1). Se este arquivo e eles divergirem, **pare e pergunte**: não escolha sozinho.

## 0. Regras inegociáveis desta tarefa

1. **Nada falso.** Se algo não funciona de verdade, fica desabilitado, com `DisabledReason`. Nada de dados de medidor, CPU, waveform ou preview simulados.
2. **Nenhuma medida inventada.** Nenhum número de pixel, cor ou fonte "parecido com o FL". Os tokens nascem `Unmeasured`, com placeholder neutro (§5), até o R0 medir.
3. **Nenhum asset da Image-Line.** Ícones desenhados em código, por nós; fonte livre com linha em `ATTRIBUTION.md`; nenhum arquivo da instalação do FL é lido.
4. **Sem widgets padrão do egui nas superfícies do produto** (`Button`, `Slider`, `ComboBox`, `Window`, `menu`, `ScrollArea` visível etc.). O egui serve de plataforma: input, texto, Painter, viewport, Harness.
5. **Sem `unwrap`/`expect`/`panic!`/`todo!` fora de testes; `unsafe_code = "forbid"`.**
6. **Git:** trabalhe em `codex/M1-<n>-<slug>`, num worktree próprio, com `CARGO_TARGET_DIR=target/agent-codex`. Siga `COLLABORATION.md` §3. **Nunca dê push sem autorização do usuário.**
7. **Pontos de parada obrigatórios para revisão do Claude:** ao final de **M1.1** e de **M1.2**. Não comece o M1.3 sem a revisão registrada no arquivo da tarefa.

## 1. M1.0 — Bootstrap do repositório

1. `git init` na raiz; `.gitignore` com `target/`, `reference/`, `*.local.*`, `.env`; primeiro commit contendo apenas `docs/`, que já existe.
2. Workspace (`Cargo.toml`):
   ```toml
   [workspace]
   resolver = "3"
   members = ["crates/*", "apps/*", "xtask"]
   [workspace.package]
   edition = "2024"
   rust-version = "1.95"
   [workspace.lints.rust]
   unsafe_code = "forbid"
   [workspace.lints.clippy]
   unwrap_used = "deny"
   expect_used = "deny"
   panic = "deny"
   todo = "deny"
   unimplemented = "deny"
   dbg_macro = "warn"
   [workspace.dependencies]
   egui = "=0.36.2"
   eframe = { version = "=0.36.2", default-features = false, features = ["wgpu", "accesskit"] }
   egui_kittest = { version = "=0.36.2", features = ["wgpu", "snapshot"] }
   serde = { version = "1", features = ["derive", "rc"] }
   serde_json = "1"
   thiserror = "2"
   proptest = "1"
   image = { version = "0.25", default-features = false, features = ["png"] }
   ```
   Em testes, `unwrap`/`expect` são permitidos com `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]`. Confira os nomes das features do eframe 0.36.2 no `Cargo.toml` dele antes de fixá-las (o PhotoCraft usa `["wgpu", "accesskit"]`).
3. `rust-toolchain.toml` com `channel = "1.99.0"` e os componentes `rustfmt` e `clippy`.
4. `xtask` com os subcomandos:
   - `ci`: fmt `--check`, `clippy --workspace --all-targets -D warnings`, `test --workspace`, `layers`, `assets`.
   - `layers`: lê `cargo metadata --format-version 1 --no-deps` e aplica a tabela de `FINAL_ARCHITECTURE.md` §2, rejeitando dependência ascendente, crate não registrado e `egui`/`eframe`/`winit`/`wgpu`/`rfd` abaixo de L6 (exceto `kind = "dev"`). Modelo de referência: a lógica de `soundcraft/xtask/src/main.rs:45-80`, reescrita por nós, sem copiar.
   - `assets`: todo arquivo em `assets/` tem linha em `ATTRIBUTION.md`.
   - `parity`: gera `docs/parity-checklist.md` a partir de `catalog/menus.txt` e do registro de comandos (pode ficar para o M1.2).
   - `shot`: renderização offscreen (M1.2).
5. `AGENTS.md` e `CLAUDE.md` na raiz: resumo curto apontando para `docs/COLLABORATION.md` e para os gates. `docs/tasks/ACTIVE.md` com a linha desta tarefa. `ATTRIBUTION.md` vazio, só com o cabeçalho.
6. **Porta:** `cargo xtask ci` verde; um teste do `xtask` prova que `layers` rejeita uma dependência ascendente montada em memória.

## 2. M1.1 — Núcleo headless

### `jet-time` (L0, sem dependências internas)
- `Tick(i64)`, `Ppq(u32)` (padrão 96 [Manual: songsettings_settings]), `Tempo` em milésimos de BPM (inteiro; faixa 10.000–522.000 [Manual: toolbar_panels]), `TimeSig { num, den }`, `steps_per_beat`.
- `TempoMap` (M1: tempo constante; a API já aceita múltiplos pontos).
- Conversões `tick ↔ frames(rate)` e `tick ↔ Duration` com aritmética racional (`i128` intermediário e arredondamento definido e documentado).
- Formatação dos dois modos do Time panel: `Bar:Step/Beat:Tick` e `Min:Sec:Centisec` [Manual: toolbar_panels].
- **Testes:** proptest de ida e volta sem drift; limites (`i64` saturante, PPQ 1, tempo mínimo e máximo).

### `jet-model` (L1)
- Entidades exatamente como em `FINAL_ARCHITECTURE.md` §3, com `Arrangement` desde já e um único ativo na UI.
- `IdAllocator` monotônico; IDs nunca reutilizados.
- `validate(&Project) -> Result<(), ModelError>`: referências órfãs, limites (`MAX_PATTERNS = 999`, `MAX_STEPS = 512`, `MAX_INSERTS = 500`, todos [Manual]) e ordem consistente.
- Comando de troca de PPQ que reescala; ao reduzir, arredonda para a nova grade (com teste).

### `jet-format` (L1)
- `.jetproj` JSON: `{ "format_version": 1, "app_version", "project": … }`.
- `save_atomic(path)`: temporário no mesmo diretório, `sync_all` e `rename`. Em Windows, trate o rename sobre um arquivo existente e teste isso.
- `load(path)`: limite de tamanho do arquivo, `validate`, erro legível para recurso ausente (`Resource.status = Missing`, sem falhar o projeto inteiro).
- **Testes:** proptest de `Project` arbitrário válido em `save → load` com igualdade; arquivos malformados ou truncados geram `Err` e nunca pânico.

### `jet-engine` (L4)
- `Session { project: Arc<Project>, history, transport, session_state }`.
- `CommandSpec` e registro conforme `FINAL_ARCHITECTURE.md` §5.1; um arquivo por área em `src/cmd/<área>.rs`; `cmd/mod.rs` só registra módulos, uma linha cada.
- `execute(id, params)`: valida estritamente (campo desconhecido gera erro), confere `enabled`, captura pânico com `catch_unwind` e restaura, e grava no histórico conforme `UndoPolicy`.
- Histórico: `undo_step`, `redo_step`, `toggle_last` (Ctrl+Z = "Undo/Redo last edit"; Ctrl+Alt+Z = "Undo step-by-step" [Manual: basics_shortcuts]). Coalescência por chave. Limite de passos.
- Transporte: `PlayMode::{Pattern, Song}`, `TransportState`, `Clock` trait, `WallClock` e `ManualClock` (para testes). A posição musical deriva do relógio e do `TempoMap`. **Sem áudio.**
- `SnapshotPublisher` trait + `NullPublisher` (costura para o M2; nenhum consumidor no M1).

**Comandos do M1** (ids definitivos; atalhos marcados [Manual] vêm de `basics_shortcuts.htm`/`toolbar_panels.htm`, os demais ficam sem atalho até a observação):

| Área | Ids |
|---|---|
| file | `file.new`, `file.open {path}` (Ctrl+O), `file.save` (Ctrl+S), `file.save_as {path}` (Ctrl+Shift+S). **Atenção:** no FL, Ctrl+N é "Save new version" [Manual], não "New" |
| edit | `edit.undo_toggle` (Ctrl+Z), `edit.undo_step` (Ctrl+Alt+Z), `edit.redo_step` (atalho **[Pendente]**) |
| transport | `transport.play_stop` (Espaço), `transport.pause` (Ctrl+Espaço), `transport.mode.toggle` (L), `transport.tempo.set {milli_bpm}`, `transport.position.set {tick}` |
| pattern | `pattern.select {id}`, `pattern.next` (+), `pattern.prev` (-), `pattern.next_empty` (F4), `pattern.first_empty` (Shift+F4), `pattern.rename {id,name}` (F2 abre o diálogo na UI), `pattern.clone {id}` (Alt+C), `pattern.delete {id}`, `pattern.length.set {id, Auto\|Steps(n)}` |
| channel | `channel.add {kind, name?, resource?}`, `channel.remove`, `channel.rename`, `channel.move {id,to}`, `channel.mute.set`, `channel.volume.set`, `channel.pan.set`, `channel.mixer_target.set`, `channel.color.set` |
| step | `step.set {pattern,channel,index,on}`, `step.paint {pattern,channel,from,to,on}` (coalescido) |
| project | `project.swing.set`, `project.ppq.set`, `project.time_sig.set` |
| playlist | `playlist.track.add`, `playlist.track.rename`, `playlist.track.mute.set`, `playlist.clip.add {track,source,start}`, `playlist.clip.move`, `playlist.clip.resize`, `playlist.clip.delete`, `snap.global.set {value}` (valores do Global Snap [Manual: toolbar_panels]) |

### `jet-cli` (app)
- `jet-cli run [--project p] --cmd 'id={json}' … [--save out.jetproj]` e `jet-cli inspect p.jetproj` (JSON canônico, ordenado).
- **Porta do M1.1:** um teste de integração cria dois canais e dois patterns, liga passos diferentes em cada um, alterna patterns, salva, reabre e compara o `inspect`; desfaz e refaz, e confere de novo. → **PARE e peça revisão do Claude.**

## 3. M1.2 — Shell, menus e gerenciador de janelas

### Estrutura de `jet-ui`
```text
src/
  app.rs            JetApp: Session + ViewState + Workspace + ShortcutMap; logic() e ui()
  probe.rs          LayoutProbe: WidgetId → (rect, role, enabled, value) por frame
  theme.rs          Tokens (de fidelity/spec/tokens.ron; status Measured|Unmeasured)
  icons.rs          ícones desenhados em código (paths próprios)
  fonts.rs          fonte livre embutida
  shortcuts.rs      ShortcutMap e resolução por contexto
  workspace/        modelo puro (mod.rs) + render (view.rs)
  toolbar/          ToolbarLayout (dados) + painéis
  menus/            catálogo → árvore de menu; itens sem comando desabilitados
  widgets/          (M1.3)
  surfaces/         channel_rack.rs, playlist.rs, browser.rs (M1.5–M1.7)
  control.rs        ui.inspect/click/drag/key/scroll/screenshot
  debug/            único lugar onde widgets padrão do egui são permitidos (painel de debug, fora do produto)
```

### Requisitos
1. **MainWindow:** `eframe` com `decorations(false)`. Os controles de janela (minimizar, restaurar, fechar) são desenhados por nós [Manual: "Window Control Icons"]; arrastar uma área vazia da toolbar move a janela, e as bordas redimensionam.
2. **Toolbar dirigida por dados:** `ToolbarLayout { panels: Vec<PanelPlacement { kind, visible }> }`, carregado de um preset embutido. Ordem inicial dos painéis: **[Pendente: R0]**. Até lá, use a ordem em que o manual os lista e marque o preset como `Unmeasured`. Ctrl+F11 oculta a toolbar [Manual].
3. **MenuBar:** os menus de topo são **FILE, EDIT, ADD, PATTERNS, VIEW, OPTIONS, TOOLS, HELP** [Manual: toolbar_panels]. Os itens vêm de `catalog/menus.txt`. Comece só com os itens que o manual documenta (marcados `[manual]`) e deixe o restante para a transcrição do R0. Item sem comando: desabilitado. Itens com checkbox aceitam clique direito sem fechar o menu [Manual: basics_interface].
4. **Hint Bar:** mostra o `hint` do widget sob o cursor, e o valor dele durante a edição.
5. **Workspace** (`FINAL_ARCHITECTURE.md` §4.3): `apply(WsAction)` atômico e validado; encaixe nas bordas de outras janelas e do desktop, com Alt desligando; a distância de encaixe é um token `Unmeasured`. Atalhos: F5, F6, Alt+F8, F12, Tab, Enter (Playlist), Esc e Ctrl+Shift+H [Manual]. F7, F8 e F9 ficam registrados e **desabilitados**, com motivo "chega no M3/M5/M7". "Detached" aparece desabilitado.
6. **LayoutProbe + `ui.inspect`:** todo elemento interativo tem `WidgetId` estável, no formato `"<superfície>.<elemento>[.<índice>]"`, e esse id é usado pela spec (`UI_FIDELITY_STRATEGY.md` §4.2).
7. **`cargo xtask shot -- --script s.jsonl --size 1920x1080 --ppp 1 --out dir/`** via `egui_kittest::Harness` + wgpu. Linhas do script: requisições de controle, `{"shot": "nome.png"}`, `{"steps": n}`. Animações desligadas.
8. **Testes do M1.2:**
   - modelo do Workspace: proptest com 1.000 ações aleatórias, invariantes (ids únicos, foco válido, janelas dentro do desktop, restaurar depois de maximizar devolve o retângulo anterior);
   - Harness com ponteiro real: arrastar a janela A para perto de B encaixa, com Alt não encaixa, maximizar e restaurar, a ordem de z após clique;
   - atalhos: todo atalho exibido em menu despacha o comando exibido, em quatro estados de foco (nenhum, janela, campo de texto, Typing-to-piano ligado);
   - um teste falha se `egui::Button`, `egui::Slider`, `egui::ComboBox`, `egui::Window` ou `ui.menu_button` aparecerem fora de `src/debug/`.
9. **Porta do M1.2:** capturas offscreen de `shell/startup` e `shell/windows` **lidas por você** (abra os PNGs) e anexadas ao handoff por caminho local. → **PARE e peça revisão do Claude.**

## 4. M1.3 a M1.8

Siga `IMPLEMENTATION_ROADMAP.md`, com as convenções de controle de `FINAL_ARCHITECTURE.md` §4.4. Pontos de atenção:

- **M1.4:** o Play só mexe no transporte. Para o teste de posição, use `ManualClock`. O clique direito em PAT abre o Channel Rack e em SONG abre a Playlist [Manual].
- **M1.5:** clique esquerdo liga o passo e o direito desliga [Manual]; a pintura por arraste é **[Pendente]**. Implemente-a atrás de uma flag até o R0 confirmar. O swing só muda o valor no modelo e na UI; o efeito audível é do M2, e o hint precisa dizer isso só se o FL tiver hint equivalente (caso contrário, registre no scorecard).
- **M1.6:** desenhe só o intervalo visível; implemente a porta de desempenho e registre a máquina (CPU, GPU, SO) no handoff.
- **M1.7:** o Browser lista **somente** pastas adicionadas pelo usuário [Manual: browser]. Nada de conteúdo on-line, nem das abas Plugin Database ou Sounds (desabilitadas).
- **M1.8:** depende das capturas do usuário. Antes disso, implemente `tools/capture` e `cargo xtask measure` (`UI_FIDELITY_STRATEGY.md` §3.2 e §4.1) e teste-os com capturas do próprio JetStudio.

## 5. Placeholder de tokens não medidos

Enquanto o token for `Unmeasured`, use **tons de cinza neutros**, escolhidos só para manter contraste legível (WCAG AA calculado em teste, como o SoundCraft faz em `mix_strip_layout.rs`), e **declare no código** `// UNMEASURED: placeholder, not derived from the reference`. Não tente "chegar perto do FL" de memória. Isso seria uma medida inventada.

## 6. Entrega

- Um branch e um merge por incremento, cada um com o handoff de `COLLABORATION.md` §7, incluindo a saída final de `cargo xtask ci`.
- Atualize `docs/tasks/ACTIVE.md`, `docs/parity-checklist.md` (gerado) e `docs/ui-scorecard.md` (gerado a partir do M1.8).
- Liste em `docs/tasks/M1.md` cada **[Pendente]** encontrado, com o cenário de R0 que o resolveria.

## 7. Referências desta tarefa

- Manual (FL Studio 2026): `basics_interface.htm`, `toolbar_panels.htm`, `basics_shortcuts.htm`, `channelrack.htm`, `playlist.htm`, `browser.htm`, `mixer.htm`, `songsettings_settings.htm`, `envsettings_general.htm`, em `https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/`.
- Padrões do ArtCraft (estudar; **não copiar código**, embora a licença seja MIT/Apache, porque queremos o nosso desenho): `soundcraft/xtask/src/main.rs` (layers), `soundcraft/crates/engine/src/cmd/mod.rs` (CommandSpec), `soundcraft/crates/ui-egui/examples/ui_shot.rs` (Harness offscreen), `soundcraft/crates/ui-egui/tests/mix_strip_layout.rs` (asserções em shapes), `craft-libs/crates/ui/tests/visual_regression.rs` (reference painters e mutações), `craft-libs/docs/ui-docking.md` (layout transacional).
