# Arquitetura final — JetStudio

> **Status:** vinculante para a implementação, a partir de 23/10/2026. Alterações só por meio de um registro de decisão (ADR) na seção 11.
> **Base:** decisões verificadas em `CLAUDE_REVIEW.md`; fatos do FL Studio vêm do manual oficial (FL Studio 2026) e são marcados **[Manual]**; padrões do ArtCraft conferidos no código são marcados **[ArtCraft-verificado]**; o restante é **[Decisão]** nossa.
> "JetStudio" e o prefixo `jet-` são nomes provisórios (ver Q3 em `CLAUDE_REVIEW.md`). O produto **não** usa os nomes "FL" nem "FL Studio", nem marcas da Image-Line.

## 1. Princípios

1. **Modelo antes da tela.** Todo estado persistente vive num documento serializável (`Project`) sem dependência de UI ou de áudio. A UI é cliente do motor. **[ArtCraft-verificado]** Nos três apps, nada abaixo da UI depende de egui (`xtask layers`).
2. **Toda ação visível é um comando** com id estável, acionável igualmente por menu, atalho, CLI, canal de controle e testes. **[ArtCraft-verificado]**
3. **Nada falso.** Uma função sem implementação real aparece **desabilitada**, com motivo (`disabled_reason`), ou não aparece. Nunca há dados simulados em medidores, CPU, waveforms ou previews.
4. **Fidelidade é dado, não intuição.** Geometria, cores e tipografia vêm de arquivos de *spec* medidos da referência, com a origem de cada valor (ver `UI_FIDELITY_STRATEGY.md`). Enquanto não há medida, o token é marcado `UNMEASURED`.
5. **Tempo real é inviolável** (M2+). O callback de áudio não bloqueia, não aloca em regime e não entra em pânico. Isso é **medido em teste**, não presumido.
6. **Nunca travar.** Sem `unwrap`/`expect`/`panic!` fora de testes; entradas (arquivos, parâmetros de comandos, canal de controle) são hostis; `unsafe` é proibido, exceto em crates de FFI de plugins futuros. **[ArtCraft-verificado]** É a política do SoundCraft.

## 2. Workspace e camadas

```text
jetstudio/
├─ Cargo.toml            workspace (resolver 3, edition 2024)
├─ rust-toolchain.toml   1.99.0 (versão instalada na máquina de dev em 23/10/2026)
├─ crates/
│  ├─ jet-time           L0  Tick, Ppq, TempoMap, TimeSig, conversões, formatos de exibição
│  ├─ jet-model          L1  Project e entidades (serde), IDs, invariantes, validação
│  ├─ jet-format         L1  formato de projeto v1, migrações, save atômico, recursos ausentes
│  ├─ jet-seq            L2  (M2) compila Project → agenda de eventos (puro, offline)
│  ├─ jet-dsp            L2  (M2) instrumentos/efeitos internos, sem dependências de I/O
│  ├─ jet-audio-graph    L3  (M2) grafo de render compartilhado por playback e bounce
│  ├─ jet-engine         L4  Session, registro de comandos, histórico, transporte, Clock
│  ├─ jet-playback       L4  (M2) CPAL, thread de áudio, filas RT
│  ├─ jet-control        L5  canal de controle JSON/TCP e CLI (M1: só inspeção e comandos)
│  ├─ jet-ui             L6  egui: shell, gerenciador de janelas, widgets, superfícies, tema
│  └─ jet-fidelity       L6  carregamento de specs e comparação de shapes/PNGs (dev/test)
├─ apps/
│  ├─ jetstudio          desktop (eframe)
│  └─ jet-cli            headless: comandos, inspeção, (M2) render
├─ xtask/                ci · layers · assets · parity · fidelity · shot
├─ catalog/menus.txt     árvore de menus da referência (só nomes), com origem por linha
├─ fidelity/spec/        specs medidos (números + origem), versionados
└─ reference/            CAPTURAS DA REFERÊNCIA — no .gitignore, nunca versionadas
```

**Regra de camadas** (verificada por `cargo xtask layers`, nos moldes de `soundcraft/xtask/src/main.rs:45-80`): um crate só depende de camadas inferiores. Nenhum crate abaixo de L6 depende de `egui`, `eframe`, `winit`, `wgpu` ou `rfd`. `jet-time` não tem dependências internas. Crates (M2) só entram no workspace quando o milestone deles começar. Não criar crates vazios.

## 3. Modelo de dados (`jet-model`)

O modelo reflete as relações documentadas no manual, e não a organização interna da Image-Line, que é desconhecida.

```text
Project
 ├─ settings: ProjectSettings { ppq: Ppq (padrão 96 [Manual]), time_sig, tempo: Tempo (10–522 BPM [Manual]),
 │                             tempo_fine (1/1000 BPM [Manual]), swing, main_volume, main_pitch }
 ├─ channels:  IdMap<ChannelId, Channel>      ordem explícita: Vec<ChannelId>
 │     Channel { name, color, kind: Sampler{sample: Option<ResourceRef>} | (futuro) Plugin…,
 │               mute, volume, pan, mixer_target: MixerInsertIndex, group, selected? → NÃO (é view state) }
 ├─ patterns:  IdMap<PatternId, Pattern>       ordem explícita; máx. 999 [Manual]
 │     Pattern { name, color, length: Auto | Steps(1..=512) [Manual],
 │               steps: Map<ChannelId, StepSeq>, notes: Map<ChannelId, Vec<Note>> (M3) }
 │     StepSeq { cells: Vec<Step> }  Step { on: bool, props: StepProps (velocity, pan, release, pitch, mod x/y, shift) }
 │       → desligar um passo preserva props quando "Mute removed steps" estiver ativo [Manual: channelrack]
 ├─ arrangements: IdMap<ArrangementId, Arrangement>, current: ArrangementId     [Manual: playlist "Arrangements"]
 │     Arrangement { name, tracks: Vec<PlaylistTrack>, clips: IdMap<ClipId, ClipInstance>, markers }
 │     PlaylistTrack { id, name, color, height, mute, (M4) solo/group/lock }
 │     ClipInstance { id, track, start: Tick, length: Tick, offset: Tick, source: ClipSource, muted }
 │     ClipSource = Pattern(PatternId) | Audio(ResourceRef) (M4) | Automation(AutomationId) (M6)
 ├─ mixer: Mixer { master, current, inserts: Vec<Insert> (até 500 [Manual]) }   (M1: só índices e nomes; M5: completo)
 ├─ resources: IdMap<ResourceId, Resource { path_rel, path_abs_hint, hash?, status }>
 └─ ids: IdAllocator  (contadores monotônicos por tipo; IDs nunca reutilizados)
```

Regras:

- **Canal ≠ faixa de Playlist ≠ insert do Mixer.** Os vínculos são opcionais (`mixer_target`; o *Track Mode* chega no M4). Uma faixa de Playlist aceita clipes de qualquer tipo e sobreposições. **[Manual: playlist, channelrack, mixer]**
- **Patterns e canais são compartilhados entre Arrangements**: o Arrangement só contém faixas, clipes e marcadores. **[Manual: playlist.htm "Arrangements share the Channel Rack and Mixer"]**
- **Estado de visualização fica fora do `Project`**: seleção, scroll, zoom, janelas abertas e foco ficam em `ViewState` (UI) ou `SessionState` (engine, não persistido). O que o FL salva no projeto quanto a layout de janelas é **[Pendente: observar]**. Até lá, layout de janelas vai para as preferências do usuário, não para o projeto.
- **Tempo:** `Tick(i64)` relativo ao `ppq` do projeto. Trocar o PPQ é um comando explícito que reescala todos os eventos; ao **reduzir**, eventos que não caem na nova grade são realocados, como o manual descreve. **[Manual: songsettings_settings]** Nunca há `f32`/`f64` como posição armazenada. Conversões tick ↔ amostra usam aritmética racional em `jet-time`, com testes de propriedade (sem drift em 10⁶ conversões).
- Toda coleção carregada de arquivo tem limites (`MAX_CHANNELS`, `MAX_PATTERNS = 999`, `MAX_STEPS = 512`, `MAX_INSERTS = 500`…). Se o FL tiver um limite próprio para algo, esse limite vem do manual; se não houver número documentado, o limite é marcado como nosso.

## 4. Interface (`jet-ui`)

### 4.1 Plataforma: egui/eframe **0.36.2**, sem widgets padrão no produto

- **[Decisão]** eframe com backend `wgpu`. As versões ficam fixadas com `=0.36.2` em `egui`, `eframe` e `egui_kittest`. O upgrade para 0.37+ é uma tarefa própria, feita depois que os testes de fidelidade existirem e passarem antes e depois. O egui 0.37.0 tem 8 dias; os apps Craft continuam no 0.36.
- O egui fornece input, foco, texto (epaint 0.36 com `skrifa` e `harfrust`), tesselação, wgpu, AccessKit, viewports nativas e o `egui_kittest`. **Nenhum `egui::Button`, `Slider`, `ComboBox`, `Window` ou `menu` padrão é usado nas superfícies do produto.** Todo controle visível é um widget nosso (`jet-ui::widgets`) que pinta via `Painter`. Um teste varre o código e falha se aparecerem chamadas proibidas fora de `jet-ui/src/debug/`.
- Densidade: Playlist, Channel Rack e Piano roll desenham **só o intervalo visível** (virtualização manual). Se o orçamento de frame estourar (porta em `IMPLEMENTATION_ROADMAP.md`), a camada densa passa a usar `egui_wgpu::CallbackTrait` (pintura wgpu direta dentro do egui), sem trocar de framework.

### 4.2 Estrutura do shell (fatos do manual)

```text
MainWindow (janela nativa sem decoração do SO; o FL desenha os próprios controles de janela [Manual: "Window Control Icons"])
 ├─ Toolbar (área superior, dirigida por dados: ToolbarLayout { panels: Vec<PanelPlacement> }, presets, "Flat buttons",
 │           separadores, ocultar com Ctrl+F11 [Manual])
 │     painéis do M1: MenuBar · Title&Hint · Transport&Recording · Tempo · Time · Pattern · GlobalSnap · Shortcut icons ·
 │                    WindowControls · (vazios/desabilitados até M2: Output, CPU&Memory, MonitorVolume/Pitch)
 └─ Desktop ("FL Studio desktop" no manual)
      ├─ Browser acoplado à lateral, ou destacado [Manual: browser.htm]
      └─ WorkspaceWindows: janelas-filhas flutuantes (Channel Rack, Playlist, e depois Piano roll, Mixer, plugins)
```

### 4.3 Gerenciador de janelas (`jet-ui::workspace`)

O modelo é puro e testável sem render, inspirado no padrão transacional de `craft-ui::docking` **[ArtCraft-verificado]**, mas é implementação própria: o FL usa janelas flutuantes num desktop, não docking por abas.

```rust
struct Workspace { windows: Vec<WinState> /* ordem = z-order */, focused: Option<WinId>, browser: BrowserDock }
struct WinState { id: WinId, kind: WinKind, rect: RectLogical, visible: bool,
                  mode: Normal | Maximized{restore: RectLogical} | Detached{os_rect} (M1: Detached desabilitado) }
enum WsAction { Open(WinKind), Close(WinId), Toggle(WinKind), Focus(WinId), Move{id, to}, Resize{id, edge, to},
                Maximize(WinId), Restore(WinId), CycleNested, ArrangeDefault, CloseAll }
impl Workspace { fn apply(&mut self, a: WsAction, desktop: RectLogical) -> Result<(), WsError> } // valida antes e depois; atômico
```

- **Encaixe (snap) ao arrastar** para bordas de outras janelas e do desktop, com **Alt** desativando. **[Manual: basics_interface]** A distância de encaixe é **[Pendente: medir]** e fica num token `UNMEASURED` até lá.
- Atalhos de janela **[Manual: basics_shortcuts]**: F5 Playlist, F6 Channel Rack, F7 Piano roll (desabilitado até o M3), F8 Plugin Picker (desabilitado), F9 Mixer (desabilitado até o M5), Alt+F8 Browser, F12 fechar todas, Tab alternar janelas aninhadas, Enter maximizar/minimizar a Playlist, Esc fechar janela, Ctrl+Shift+H reorganizar. Toggle significa: se a janela está oculta, mostra e foca; se está visível e focada, oculta. O comportamento exato quando ela está visível mas sem foco é **[Pendente: observar]**.
- Arrastar conteúdo sobre os botões de janela da toolbar traz a janela para a frente. **[Manual: toolbar_panels]**
- Ações de workspace passam pelo mesmo registro de comandos (ids `view.*`), são inspecionáveis e acionáveis pelo canal de controle, **mas não entram no histórico de undo do projeto**.

### 4.4 Biblioteca de widgets (`jet-ui::widgets`)

Cada widget separa **estado/comportamento** (máquina de estados testável) de **pintura** (função pura de estado + tokens → shapes). Todo widget registra `(WidgetId, rect, role, value?, enabled)` no **LayoutProbe** a cada frame. Isso alimenta `ui.inspect` e os testes de spec.

| Widget | Comportamento obrigatório (fonte) |
|---|---|
| `Knob`, `Slider` | arrastar vertical (ou horizontal no slider); Ctrl = fino; Shift = sem snap em valores especiais; Alt+clique ou clique do meio = reset; botão direito = menu com "Type in value…" e "Set"; valor e descrição na Hint Bar durante o hover e o arraste **[Manual: basics_interface, toolbar_panels]** |
| `Lcd` (tempo, posição, números) | arrastar = alterar; clique na borda superior/inferior = ±1 passo, com cursor específico; clicar e segurar e digitar = entrada numérica **[Manual: basics_interface]** |
| `ToolbarButton`, `ToggleButton` | estados normal/hover/pressed/on/disabled; botão direito com ação alternativa onde o manual documenta (ex.: clique direito em PAT abre o Channel Rack e em SONG abre a Playlist) **[Manual: toolbar_panels]** |
| `StepButton` | esquerdo liga, direito desliga; arrastar pinta **[Manual: channelrack; pintura por arraste: Pendente-observar]** |
| `Menu`, `ContextMenu` | itens com checkbox aceitam clique direito sem fechar **[Manual: basics_interface]**; navegação por teclado; itens sem comando ficam desabilitados |
| `TitleBar` de janela-filha | menu no canto superior esquerdo, título e controles; mover e redimensionar pelas bordas |
| `HintBar` | texto do widget sob o cursor, ou valor durante a edição |

### 4.5 Tema e tokens

`jet-ui::theme::Tokens` é **gerado** de `fidelity/spec/tokens.ron` por `build.rs` (ou carregado em debug com hot-reload). Cada token tem `value`, `source` (`capture_id` + método) e `status` (`Measured | Unmeasured`). Valores `Unmeasured` recebem um placeholder neutro **declarado como tal**, e o scorecard os conta. Widgets nunca usam cores literais; um lint de teste procura `Color32::from_rgb` fora de `theme`.

Fontes: uma fonte **livre** embutida, com licença e linha em `ATTRIBUTION.md`, escolhida pelas métricas medidas. A fonte original do FL é **[Pendente: identificar]**. Ícones: **desenhados em código** (paths vetoriais próprios), com silhueta e função equivalentes, **nunca decalcados** de capturas. Logotipos da Image-Line nunca aparecem.

### 4.6 Atalhos

Há uma tabela única, `ShortcutMap`, com `(contexto, acorde) → command_id`. A resolução segue esta ordem: campo de texto focado → janela focada → global. Com "Typing keyboard to piano" (Ctrl+T) ativo, atalhos de tecla única são suprimidos **[Manual: basics_shortcuts]**. O mesmo mapa gera o texto de atalho exibido nos menus (o PhotoCraft PR #141 corrigiu 214 divergências entre atalhos exibidos e despachados **[ArtCraft-verificado via doc do Codex]**). Um teste garante que todo atalho exibido despacha o comando exibido, em quatro estados de foco.

## 5. Motor (`jet-engine`)

### 5.1 Comandos

```rust
pub struct CommandSpec {
    pub id: &'static str,              // "channel.step.toggle", "transport.play", "view.playlist.toggle"
    pub label: &'static str,
    pub menu: &'static [&'static str], // caminho no catálogo de referência; vazio = fora de menus
    pub shortcut: Option<&'static str>,
    pub context: ShortcutContext,       // Global | Window(WinKind) | TextField
    pub params: &'static str,           // documentação para humanos/agentes; validação estrita
    pub enabled: fn(&Session) -> Result<(), DisabledReason>,
    pub run: fn(&mut Session, &Params) -> Result<Outcome, CmdError>,
    pub undo: UndoPolicy,               // None | Step | Coalesce(key)
}
```

- Validação **estrita** de parâmetros: campo desconhecido, tipo errado ou valor fora da faixa geram erro explícito, nunca um default silencioso. O PhotoCraft PR #2656 corrigiu exatamente esse defeito **[ArtCraft-verificado via doc do Codex]**.
- `Session::execute` captura pânicos, restaura o documento e registra a falha. **[ArtCraft-verificado]** (`Engine::execute` no SoundCraft).
- Comandos de UI pura (janelas e diálogos) ficam num registro paralelo em `jet-ui`, com o mesmo espaço de ids e o mesmo despacho.
- O catálogo `catalog/menus.txt` lista os itens da referência com marcação de origem por linha (`[manual]`, `[observado:<capture_id>]`). Item sem comando correspondente é exibido **desabilitado**. `cargo xtask parity` gera `docs/parity-checklist.md`. **Presença no menu não é paridade** (ver o scorecard em `UI_FIDELITY_STRATEGY.md`).

### 5.2 Histórico (Undo/Redo) com a semântica do FL

- Snapshots `Arc<Project>` com *structural sharing* (`Arc` por coleção grande; `Arc::make_mut`). **[ArtCraft-verificado]** É o padrão de SoundCraft, PhotoCraft e FilmCraft. Os projetos de DAW são pequenos comparados a pixels, e o áudio decodificado fica fora do `Project`, num `ResourcePool`.
- Coalescência por chave para gestos contínuos: um arraste de knob ou uma pintura de passos vira **um** passo.
- **Ctrl+Z = "Undo/Redo last edit"**: alterna entre desfazer e refazer a **última** edição. **Ctrl+Alt+Z = "Undo step-by-step"**: desfaz recuando no histórico. **[Manual: basics_shortcuts]** O comportamento exato de redo passo a passo e a interação com edições novas são **[Pendente: observar]**. A API do histórico expõe `undo_step`, `redo_step` e `toggle_last`, e o mapeamento de teclas é decidido pela observação.
- Limite de histórico em número de passos **e** em bytes estimados, configurável.

### 5.3 Transporte e relógio

```rust
pub enum PlayMode { Pattern, Song }                      // PAT/SONG, tecla L [Manual]
pub enum TransportState { Stopped, Playing, Paused }      // Espaço = play/stop; Ctrl+Espaço = pause [Manual]
pub trait Clock: Send { fn now_frames(&self) -> u64; fn rate(&self) -> u32; }
```

- O transporte é um estado do engine. A posição musical deriva de `Clock` + `TempoMap`.
- **M1:** `WallClock` (monotônico do SO) avança a posição; **não há saída de áudio**. **M2:** `DeviceClock` (frames entregues pelo callback) se torna a fonte, e o `WallClock` sai do caminho de playback.
- A semântica de Stop (voltar ao início ou à posição de *seek*, "Remember seek time") segue o manual **[Manual: toolbar_panels]**. O detalhe fino fica **[Pendente: observar]**.
- A UI lê a posição a cada frame por um `TransportSnapshot` barato (atômicos). A UI nunca controla o relógio do áudio.

### 5.4 Costura para o áudio (preparada no M1, usada no M2)

```text
UI thread ── execute(cmd) ──► Session (Arc<Project> rev N)
                                  │ on_change(rev)
                                  ▼
                   SnapshotPublisher (trait)  ── M1: NullPublisher (nenhum consumidor)
                                  │            ── M2: compila Project → RenderPlan (jet-seq + jet-audio-graph) num worker
                                  ▼
                   rtrb (pré-alocado) ──► thread de áudio: troca o plano entre blocos
                   ◄── rtrb de retorno: planos/plugins antigos liberados na thread de UI
```

- O padrão de devolver objetos antigos para fora da thread de áudio vem do SoundCraft **[ArtCraft-verificado: playback/src/lib.rs:19,171]**, mas aqui com **filas pré-alocadas**, porque o `std::mpsc` pode alocar no `send` (`CLAUDE_REVIEW.md` D5).
- Parâmetros contínuos (volume, pan) vão por uma fila de mudanças com offset amostral, sem recompilar o plano.
- Testes do M2: `assert_no_alloc` em volta do callback em modo de teste; render offline determinístico comparado ao playback para o mesmo material; medição de duração do callback versus o prazo, e de underruns.

## 6. Persistência (`jet-format`)

- Formato v1: diretório ou arquivo `.jetproj` com JSON UTF-8 (`format_version`, `app_version`, `project`) e referências de recursos relativas, com hint absoluto. JSON foi escolhido para permitir diffs e depuração no início. Um formato binário só entra se houver medida de desempenho que justifique.
- Save atômico: escrita em arquivo temporário no mesmo diretório, fsync e rename (o padrão do `craft-atomic`; implementar ou depender dele, MIT/Apache, após revisão).
- Load: validação de limites, IDs órfãos e versão; migrações `vN → vN+1` testadas; recurso ausente gera aviso legível, nunca corrupção. Round-trip `save → load` tem igualdade semântica coberta por teste de propriedade.
- `.flp`: **fora do escopo** até haver estudo de interoperabilidade autorizado. O manual não publica o esquema **[Manual: fformats_project]**.

## 7. Automação e controle (`jet-control`)

- **M1:** JSON por linha sobre TCP **em loopback**, com token no primeiro frame (padrão do PhotoCraft **[ArtCraft-verificado via doc do Codex]**). Métodos: `engine.execute`, `engine.commands`, `session.inspect`, `ui.inspect` (árvore do LayoutProbe), `ui.click`, `ui.drag`, `ui.key`, `ui.scroll`, `ui.screenshot`. Parâmetros validados de forma estrita.
- **Offscreen:** `cargo xtask shot -- --script s.jsonl --size WxH --ppp 1.0 --out dir/` usa `egui_kittest::Harness` com renderer wgpu, como o `ui_shot` do SoundCraft **[ArtCraft-verificado]**.
- **MCP:** depois do M1 (envoltório sobre o mesmo protocolo).

## 8. Testes e qualidade

| Gate (`cargo xtask ci`) | Conteúdo |
|---|---|
| fmt + clippy `-D warnings` | lints `unwrap_used`, `expect_used`, `panic` etc. negados nas raízes de produção |
| testes | unidade, propriedade (`proptest`) para `jet-time` e para o round-trip do formato |
| layers | dependências entre camadas e proibição de UI abaixo de L6 |
| assets | todo asset empacotado tem linha em `ATTRIBUTION.md` |
| ui-spec | testes de spec e de shapes (`UI_FIDELITY_STRATEGY.md` §5, camada A) |
| ui-baseline | regressão de PNG interna (camada B), por SO |
| parity | regenera `docs/parity-checklist.md`; falha se o piso cair |
| perf (M1: informativo; M2: gate) | tempo de frame de cenários fixos; métricas do callback |

A paridade externa (camada C) roda **localmente**, porque depende de `reference/`, que não é versionado. Ela gera relatórios, mas não é um gate de CI.

## 9. Bibliotecas: decisão e justificativa

| Necessidade | Escolha | Por quê, para o **nosso** caso | Quando |
|---|---|---|---|
| Linguagem | Rust 2024, toolchain 1.99.0 | controle de alocação e threads no áudio; testes headless; o ArtCraft mostra viabilidade em DAW. Não garante tempo real sozinho. | M1 |
| UI | egui/eframe/egui_kittest =0.36.2 (wgpu) | ver §4.1 e `CLAUDE_REVIEW.md` D1 | M1 |
| Serialização | serde, serde_json | formato v1 legível e diffável | M1 |
| Erros | thiserror | padrão do ArtCraft | M1 |
| Propriedades | proptest | conversões de tempo e round-trip | M1 |
| Imagens (testes/relatórios) | image (png) | PNG de baselines e diffs | M1 |
| Áudio I/O | cpal 0.18.2 | WASAPI por padrão; ASIO só como feature opcional, avaliado com licença/SDK no M2 | M2 |
| Filas RT | rtrb 0.4 | SPSC sem alocação; uma fila por produtor, agregação explícita | M2 |
| Teste de alocação | assert_no_alloc 1.1.2 | prova a regra "não alocar no callback" | M2 |
| Decodificação | symphonia 0.6.1 | fora da thread de áudio; streaming e cache são responsabilidade nossa | M2 |
| MIDI | midir 0.11.1 + parser próprio | o midir não interpreta mensagens | M6 |
| CLAP / VST3 | avaliar clack-host 0.2 / vst3 0.3 em crates isolados de FFI | hospedar é um programa de trabalho próprio | M7 |
| **Proibidos** | código GPL/LGPL/AGPL (ex.: LMMS, Ardour); assets da Image-Line | clean-room e licença | sempre |

## 10. Concorrência e threads (visão final)

| Thread | Faz | Nunca faz |
|---|---|---|
| UI (eframe) | input, comandos, render, liberação de objetos retirados | bloquear esperando a thread de áudio |
| Workers | compilar RenderPlan, decodificar amostras, scan de plugins, save | tocar no `Project` sem passar pelo engine |
| Áudio (M2) | consumir plano e eventos, renderizar, publicar medidores (atômicos ou `try_lock`) | alocar, bloquear, fazer I/O, chamar log formatado |
| Controle | ler o socket e encaminhar à UI por fila | executar comandos fora da thread de UI |

## 11. Registro de decisões (ADR)

| # | Data | Decisão | Alternativa rejeitada | Motivo |
|---|---|---|---|---|
| ADR-001 | 23/10/2026 | egui 0.36.2 como plataforma de UI, com widgets 100% próprios | winit+wgpu com framework próprio | o custo de texto, input, acessibilidade e harness não compra fidelidade; evidência dos 4 apps Craft |
| ADR-002 | 23/10/2026 | M1 = shell sem áudio, com transporte silencioso real | fatia vertical com áudio | escopo da missão; risco do primeiro entregável |
| ADR-003 | 23/10/2026 | Ticks inteiros relativos ao PPQ do projeto; troca de PPQ reescala | resolução canônica fixa alta | reproduz o efeito observável documentado ao reduzir o PPQ |
| ADR-004 | 23/10/2026 | `Arrangement` como entidade desde o M1 (só 1 ativo na UI do M1) | faixas direto no Project | o manual documenta múltiplos arranjos que compartilham canais e mixer |
| ADR-005 | 23/10/2026 | Histórico com `toggle_last` + `undo_step` | undo/redo linear apenas | semântica de Ctrl+Z documentada no manual |
| ADR-006 | 23/10/2026 | Filas pré-alocadas na fronteira RT | `std::mpsc` (como o SoundCraft) | `send` pode alocar |
| ADR-007 | 23/10/2026 | Gerenciador de janelas próprio (flutuantes, encaixe, maximizar no desktop) | `craft-ui::docking` | o modelo do FL é flutuante, não docking por abas; reaproveitamos o padrão transacional |
