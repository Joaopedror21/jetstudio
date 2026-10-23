# Revisão técnica da pesquisa do Codex (Claude)

> **Data:** 23/10/2026 · **Revisor:** Claude (arquitetura e auditoria) · **Escopo:** os sete documentos de `docs/` escritos pelo Codex, conferidos contra os repositórios ArtCraft clonados e contra o manual oficial do FL Studio baixado localmente.
> **Convenção de evidência:** **[Verificado]** = conferi no arquivo/código/página citada; **[Corrigido]** = a afirmação do Codex está errada ou incompleta; **[Sem evidência]** = não encontrei a fonte; **[Decisão]** = minha recomendação, não fato.

## 1. Como esta revisão foi feita

Diferente do Codex, que só pôde ler o GitHub pelo navegador (`git ls-remote` falhou na rede dele), eu consegui clonar os repositórios. Isso me permitiu ler código, testes, `Cargo.lock` e histórico Git. Nada foi compilado nem executado: os clones são tratados como dados não confiáveis.

| Repositório | Commit inspecionado (HEAD em 23/10/2026) | Commits | Primeiro commit |
|---|---|---:|---|
| `storytold/soundcraft` | `6b84a4f0461f2467402eafe41dcc708c369fb045` | 878 | 05/10/2026 |
| `storytold/photocraft` | `28f3c18c0d11d69005d224a0a5d5edc56f1fe858` | 5.429 | 30/09/2026 |
| `storytold/filmcraft` | `fa550ce2d07bdfee6b1d7b6835e8abc8c6913d00` | 2.208 | 30/09/2026 |
| `storytold/craft-libs` (**não citado pelo Codex**) | `bfb116ccafbf48bb5494b46c6b47603dd643d106` | 188 | 09/10/2026 |
| `storytold/craftrules` | `git clone` → *Repository not found* (privado ou inexistente) | — | — |

Do manual do FL Studio, baixei 22 páginas (HTML) em 23/10/2026 e as converti em texto para busca literal. Entre elas estão `basics_interface`, `toolbar_panels`, `basics_shortcuts`, `channelrack`, `playlist`, `mixer`, `browser`, `pianoroll`, `envsettings_general`, `envsettings_audio`, `songsettings_settings` e `WhatsNew`. O changelog lista como build mais recente **26.1.7.5 (12/10/2026)**, e a página de configurações gerais cita "the release of FL Studio 2026". **[Verificado]** O manual on-line descreve, portanto, o FL Studio 2026.

## 2. Avaliação geral

A pesquisa do Codex é **boa e epistemicamente cuidadosa**. Ela separa fato, declaração do projeto, inferência e proposta. Não atribui Ghidra/REA ao ArtCraft e reconhece que contagem de menus não é paridade. Também registra os próprios limites de acesso. Conferi por amostragem os números e as afirmações centrais, e quase todos se confirmaram:

| Afirmação do Codex | Resultado |
|---|---|
| SoundCraft 482/512 itens de menu, motor sozinho 397/512 | **[Verificado]** `docs/target-app-parity.md:16`, `ROADMAP.md:18` |
| SoundCraft *pre-alpha*, ~70% de amplitude, ~40% de prontidão | **[Verificado]** `ROADMAP.md:3,19,20` |
| SoundCraft G-07: não há harness de fidelidade contra o Pro Tools | **[Verificado]** `docs/gaps.md:24,124` |
| `CommandSpec` com id, rótulo, menu, atalho, params, enabled, run | **[Verificado]** `crates/engine/src/cmd/mod.rs:50-64`. O Codex não menciona os campos `journal`, `undoable` e `view` |
| Callback CPAL marca a thread, usa `catch_unwind` e não bloqueia | **[Verificado]** `crates/playback/src/lib.rs:145,313`. Medidores usam `try_lock`, e sessões antigas e plugins retirados voltam por canal para serem liberados na thread de UI (`:19,171-173,427-435`) |
| `cargo xtask layers` lê `cargo metadata` e rejeita dependência ascendente ou de UI | **[Verificado]** `soundcraft/xtask/src/main.rs:45-80` |
| PhotoCraft 190/258 oráculos e smart filters 5/30 | **[Verificado]** `photocraft/docs/gaps.md:3,50,51` |
| FilmCraft: ticks a 254.016.000.000/s | **[Verificado]** `filmcraft/docs/architecture.md:107` |
| FL: 500 inserts, Current e Master; 10 slots de efeito por faixa | **[Verificado]** `mixer.htm` ("500 x Insert Tracks", "Each Track has 10 effects slots") |
| FL: F5, F6, F7, F8, F9, Alt+F8, Espaço, Ctrl+Espaço, L | **[Verificado]** `basics_shortcuts.htm`, `toolbar_panels.htm` |

Os problemas encontrados estão abaixo, ordenados pelo impacto no projeto.

## 3. Erros e omissões factuais

### E1 — Omissão grave: `craft-libs` é público e é a peça de UI mais relevante [Corrigido]

O Codex cita `craft-ui` apenas como "fixado em uma revisão Git" e trata o código de UI compartilhado como inacessível. Na verdade, `storytold/craft-libs` é **público**, licenciado MIT OR Apache-2.0, e contém:

- `craft-ui::docking`: modelo transacional de layout (`Layout`, `Node::{Tabs,Split,Stack}`, `Floating`, `Action`). Cada ação valida o layout antes e depois de aplicar e é atômica em caso de erro. Há um teste com 800 movimentos determinísticos e testes de ponteiro reais (`docs/ui-docking.md`). Uma limitação importa para nós: grupos flutuantes são *egui areas* em uma única viewport, e janelas nativas destacadas ficam a cargo do app.
- `craft-ui::theme::WidgetPalette`, com uma comparação de oito apps Craft (`docs/theme-infrastructure.md`). Ela mostra que cada app mantém a própria paleta e só compartilha a operação mecânica.
- **`crates/ui/tests/visual_regression.rs`**: comparações offscreen "contra *reference painters* independentes". O teste usa geometria e cores literais como baseline, compara malhas tesseladas e pixels no mesmo renderer e aplica **mutações deliberadas que provam que regressões de cor, espaçamento e traço são detectadas**. Animação e blink do cursor ficam desligados. É a técnica mais transferível para nossa estratégia de fidelidade (ver `UI_FIDELITY_STRATEGY.md`).
- `craft-settings` (config com migração e recuperação) e `craft-atomic` (escrita atômica de arquivos), úteis para preferências e para salvar projetos.

### E2 — Os valores visuais do ArtCraft **não** foram medidos da referência [Corrigido / reforço]

O Codex diz que "o procedimento original de medição não está documentado". A evidência é mais forte, e é contrária à hipótese de medição: `soundcraft/crates/ui-egui/src/theme.rs:1` diz literalmente *"Colours were chosen by eye to give a dark, studio-style look; they are ours."* Já o PhotoCraft declara o tema "Studio" como "modelled on the look of modern pro editors", e só os temas "Pro (Light Gray/Light)" citam os níveis de brilho do Photoshop (#B8B8B8, #F0F0F0) (`photocraft/crates/ui-egui/src/theme.rs:1-18`).

**Consequência para nós:** o ArtCraft demonstra **semelhança**, não fidelidade medida. Nosso requisito, fidelidade visual extremamente alta, **vai além do que o ArtCraft comprovou**. Não podemos dizer que "seguir o método ArtCraft" nos leva a 1:1. O harness de fidelidade contra a referência terá de ser construído por nós. O próprio SoundCraft registra essa falta como lacuna G-07.

### E3 — Versão do egui errada [Corrigido]

O Codex afirma "egui/eframe 0.37" no SoundCraft. Os três apps fixam **egui 0.36** (`soundcraft/Cargo.toml`, `photocraft/Cargo.toml:58`, `filmcraft/Cargo.toml:81`). O SoundCraft justifica num comentário: *"egui 0.37 needs craft-libs#199 (craft-ui) first; pin 0.36 until it lands."* Segundo o `Cargo.lock`, a versão resolvida é epaint/egui-wgpu/egui-winit **0.36.2**, com wgpu 30.0.1 e winit 0.30.13. No crates.io, o egui 0.37.0 saiu em 15/10/2026, há apenas 8 dias.

### E4 — A participação de agentes **é** parcialmente observável [Corrigido]

O Codex afirma que "contagem, prompts e mix real de Codex/Claude não são públicos". Os prompts de fato não são públicos, mas os *trailers* `Co-Authored-By` no histórico dão um **limite inferior**:

| Repo | Commits | Trailers Claude (Opus/Sonnet/Haiku 5.5) | Trailers Codex |
|---|---:|---:|---:|
| SoundCraft | 878 | ≥265 | 0 |
| PhotoCraft | 5.429 | ≥3.400 | não contado |
| FilmCraft | 2.208 | ≥985 | 16 |

Também há PRs integrados vindos de branches `codex/...` (ex.: SoundCraft PR #583). Ressalvas: trailers são opcionais, então os números são mínimos. Eles não dizem quem escreveu o quê, nem que o trabalho de cada agente é igual.

O ritmo também é informativo. No SoundCraft, fundação, UI, CLI/MCP e gates de `xtask` entraram em commits espaçados por minutos no primeiro dia (05/10/2026, de `755f02f` a `63bf6c5`). Isso indica código gerado em sessões longas de agente e integrado depois. **[Inferência]**

### E5 — Faltou a técnica de teste mais útil: asserções sobre as *shapes* emitidas [Corrigido]

O SoundCraft não testa a UI só por screenshot. `crates/ui-egui/tests/mix_strip_layout.rs` cria o app real num `egui_kittest::Harness`, executa frames e **inspeciona `FullOutput.shapes`**: posição de textos, cores e contraste WCAG calculado. São asserções determinísticas de **geometria e cor sem pixels**, imunes ao ruído de rasterização. O `ui_shot` (`examples/ui_shot.rs`) usa o `Harness` com renderer **wgpu**, `pixels_per_point = 2.0`, tamanho padrão de 1600×1000 e script JSONL de requisições de controle. O protocolo de controle expõe `ui.click`, `ui.drag`, `ui.inspect`, `ui.key`, `ui.menu.invoke`, `ui.menu.list`, `ui.move`, `ui.screenshot`, `ui.scroll`, `ui.set`, `ui.text` e `ui.zoom` (`control.rs`).

### E6 — Versão-alvo não fixada [Corrigido]

O SoundCraft fixa explicitamente a versão-alvo, **Pro Tools Ultimate 2026.4.1**, no cabeçalho de cada documento de paridade. O Codex deixa a versão do FL Studio em aberto. Proponho fixar **FL Studio 2026, Windows x64**, registrando o build exato a partir de *Help › About* na máquina de referência (o mais recente publicado é 26.1.7.5). Ver Q1 na seção 7.

### E7 — Afirmações sobre o FL Studio sem evidência no manual [Sem evidência]

- **"O valor inicial descrito pelo manual é 16 passos"** (`FL_STUDIO_RESEARCH.md` §3). Não encontrei isso em `channelrack.htm` nem em `songsettings_settings.htm`. O manual diz que *Pattern Length* vai de 1 a 512 passos ou "Auto". O padrão de 16 passos é plausível, mas precisa virar **[Pendente – observar]**.
- **Ctrl+Z.** O Codex trata Undo/Redo de forma genérica. O manual documenta **Ctrl+Z = "Undo/Redo last edit"** (alterna entre desfazer e refazer a última edição) e **Ctrl+Alt+Z = "Undo step-by-step"**. Essa semântica é diferente da usual e afeta o desenho do histórico. **[Verificado em `basics_shortcuts.htm`]**
- **O manual se contradiz sobre F10.** `basics_interface.htm` diz que F10 abre *Theme settings*, e `basics_shortcuts.htm` diz "Show/hide MIDI settings". Só a observação resolve. **[Pendente]**

### E8 — Fatos do FL Studio ausentes e necessários ao primeiro milestone [Omissão]

Todos estes foram **[Verificados]** no manual e não aparecem na pesquisa do Codex:

1. **Arrangements.** A Playlist pode ter várias *Arrangements* (versões do arranjo), e todas **compartilham Channel Rack e Mixer** (`playlist.htm:31,40`). O modelo de dados precisa de `Arrangement` acima de `PlaylistTrack` desde o início.
2. **Modelo de janelas.** As janelas "se encaixam" quando arrastadas perto umas das outras, e Alt desativa o encaixe (`basics_interface.htm`). Playlist, Channel Rack, Piano roll, Mixer e plugins podem ser **Detached** do *FL Studio desktop*, inclusive para outro monitor. O Browser pode ficar destacado ou acoplado na lateral. Há *Tab* para "cycle nested windows", *Ctrl+Shift+H* para "(Re)arrange windows", *F12* para "Close all windows" e *Enter* para "Toggle Max/Min Playlist". Esse é um gerenciador de janelas próprio e é o coração do milestone 1.
3. **A toolbar é configurável.** Painéis e ícones podem ser reordenados e ocultados, há presets e modo "Flat buttons", e Ctrl+F11 oculta tudo (`toolbar_panels.htm`). A barra superior tem de ser **dirigida por dados**, não um layout fixo no código. Painéis documentados: Menu Bar (FILE, EDIT, ADD, PATTERNS, VIEW, OPTIONS, TOOLS, HELP), Title & Hint Bar, Window Control Icons, Global Snap, Add, Multilink, Time, Pattern, Output, CPU & Memory, Chord, Online, Communications, Shortcut Icons, Transport & Recording, Monitor Volume & Pitch.
4. **Convenções de controles.** Knobs e sliders: arrastar com o botão esquerdo, Ctrl para ajuste fino, Shift para evitar o *snap* em valores-padrão, Alt+clique ou clique do meio para resetar, menu do botão direito com "Type in value" e "Set". Displays LCD: arrastar, clicar na borda superior ou inferior para incrementar ou decrementar, clicar e digitar. Itens com checkbox de menu aceitam clique direito para testar sem fechar o menu. Tudo em `basics_interface.htm`. São **widgets primitivos** do milestone 1.
5. **A Hint Bar** mostra a descrição e o valor do controle sob o cursor, além de ícones de capacidade. O conteúdo dos hints faz parte da fidelidade comportamental.
6. **Variáveis de render que alteram capturas:** Main GUI scaling ("System" segue o SO), scaling separado para pop-ups, toolbars e cursor, nível de Animations ("Don't distract me" a "Entertain me"), Transparent windows, Thick lines (≥200%) e High visibility (`envsettings_general.htm:7-29`). Todas precisam ser fixadas no protocolo de captura.
7. **Tempo e resolução.** O tempo vai de 10 a 522 BPM, com *fine tune* em 1/1000 BPM. O Time panel alterna entre Bar:Beat/Step:Tick e Min:Sec:Centisec. O PPQ é **por projeto**, padrão 96, e **reduzir o PPQ realoca eventos** que caem fora da nova grade. Há no máximo 999 patterns.

## 4. Decisões questionáveis

### D1 — `TECH_STACK_DECISION.md`: UI final sobre `winit + wgpu` com desenho próprio, egui só para ferramentas [Discordo]

O argumento do Codex é que "widgets padrão do egui não comprovam paridade". É verdade, mas é o argumento errado. **A fidelidade é decidida pela nossa geometria e pelo nosso código de pintura, não pelo framework.** O egui, usado como *plataforma* (input, foco, texto shapado, tesselação, backend wgpu, AccessKit, multi-viewport e o harness `egui_kittest`), não impõe aparência: `Painter` desenha qualquer coisa. Reescrever tudo isso sobre winit+wgpu custaria meses (texto com shaping, IME, hit testing, acessibilidade, harness de teste) sem ganho mensurável de fidelidade. Os quatro apps Craft que estudei usam egui. O epaint 0.36 já usa `skrifa` e `harfrust` para fontes e shaping (`soundcraft/Cargo.lock`).

**[Decisão]** O produto usa egui/eframe 0.36.2 como plataforma, e **nenhum widget padrão do egui aparece nas superfícies do produto**: todos os controles visíveis vêm da nossa biblioteca de widgets. Viewports nativas do egui cobrem as janelas *Detached*. Se o profiling de Playlist ou Piano roll densos estourar o orçamento de frame, a saída é um *paint callback* wgpu dentro do egui, não trocar de framework. O experimento de dois renderers proposto pelo Codex fica cancelado: ele dobraria o custo para decidir algo que as evidências já indicam. Em troca, o M1 terá uma porta de desempenho mensurável (ver `IMPLEMENTATION_ROADMAP.md`). Detalhes em `FINAL_ARCHITECTURE.md` §4.

### D2 — `FL_STUDIO_ROADMAP.md`: M1 como fatia vertical *musical* com áudio [Ajustado]

A missão define o primeiro milestone como **interface principal**: janela, barra superior, Browser, Channel Rack, Playlist, transporte e janelas. O M1 do Codex mistura sampler, mix, render offline e saída de dispositivo, o que dobra o risco do primeiro entregável. **[Decisão]** O M1 passa a ser o *shell* com modelo e comandos **reais**, sem áudio. O áudio vira o M2. Para não haver funcionalidade falsa:

- o transporte do M1 é real (estado PAT/SONG, play/stop/pause, posição musical avançando por um `Clock` de parede), mas **não produz som**, e o scorecard o marca como "parcial – sem áudio";
- tudo o que depende de áudio (preview no Browser, medidores, CPU, Output panel) aparece **desabilitado ou vazio**, nunca simulado com dados inventados;
- o motor expõe as costuras (`Clock`, publicação de snapshots do projeto) que o M2 vai ocupar.

### D3 — `FL_STUDIO_ROADMAP.md`: M0 bloqueante antes de qualquer código [Ajustado]

M0 depende de uma instalação licenciada e de capturas, que ainda não temos. **[Decisão]** O M1 começa em paralelo pelas partes que **não dependem de medidas**: workspace, modelo, comandos, histórico, formato, gerenciador de janelas e harness de inspeção. Os tokens visuais ficam marcados como `UNMEASURED` até chegarem as capturas, e nenhuma medida é inventada.

### D4 — `ARTCRAFT_AI_WORKFLOW.md`: divisão de papéis [Substituído]

A tabela do Codex dá "manter matriz de comportamentos e capturas" ao Codex. **[Decisão]** As capturas exigem a máquina com o FL Studio, que é do usuário. A coleta é de responsabilidade do usuário, ajudado por scripts. A medição a partir das capturas é feita pelo Codex com ferramenta (*edge-snap*) e auditada pelo Claude. Ver `COLLABORATION.md`.

### D5 — `TECH_STACK_DECISION.md` §3: canais `std::sync::mpsc` para o áudio [Alerta]

O SoundCraft devolve snapshots e plugins pela thread de áudio com `Sender::send` de `std::sync::mpsc` (`playback/src/lib.rs:218,270`). O canal ilimitado do `std` pode **alocar** durante o `send`, o que contraria a regra "não alocar no callback". **[Decisão]** No M2 usaremos filas pré-alocadas (`rtrb` 0.4) para entrada e retorno, e testes com `assert_no_alloc` no caminho do callback. O padrão "liberar fora da thread de áudio" do SoundCraft é bom; o transporte usado por ele, não.

### D6 — Modelo de áudio do SoundCraft como referência [Alerta]

`soundcraft/docs/architecture.md` admite: só o dispositivo **padrão**, blocos fixos de 512 amostras com reamostragem, sem seleção de buffer, sem ASIO, mídia decodificada inteira na memória e gravação em RAM até o stop. Serve como referência de **organização** (camadas, `MixEngine` comum a tempo real e offline), não de **engine profissional**. O FL Studio expõe seleção de dispositivo e buffer, ASIO e contador de underruns (`envsettings_audio.htm`).

## 5. Riscos que a pesquisa subestima

| Risco | Por quê | Mitigação |
|---|---|---|
| **Propriedade intelectual da aparência** | Copiar a aparência de um produto comercial com "fidelidade extremamente alta" pode levantar questões de *trade dress* e de direito autoral sobre elementos gráficos (ícones, logotipo), dependendo da jurisdição. O ArtCraft faz algo análogo, mas isso não é garantia jurídica. | Ícones e logotipos: desenhos **originais**, nunca vetores decalcados de screenshots. Nome e marca próprios ("JetStudio"), sem "FL" nem a fruta. Screenshots da referência só internamente. **Consultar advogado antes de distribuir.** Isto não é aconselhamento jurídico. |
| **Texto** | Renderização de fonte é o maior ruído em diff de pixels, e a fonte de UI do FL Studio **não está identificada** em nenhuma fonte consultada. | Métricas de texto (baseline, altura de maiúsculas, avanço) em vez de diff de pixels; fonte livre com métricas próximas. |
| **Configurabilidade do FL** | Toolbar, temas, scaling e layouts de janela são configuráveis: "a interface" não é uma só. | Fixar um **perfil de referência** (toolbar preset padrão, tema padrão, 100%) e só depois cobrir variações. |
| **Ritmo do ArtCraft como expectativa** | 878 commits em 18 dias no SoundCraft ainda resultaram em "pre-alpha, ~40% de prontidão" e "UI ~55%" (autoavaliados). | Medir por fluxo e por dimensão; nada de porcentagem única. |

## 6. O que preservei

Não reescrevi nenhum documento do Codex. Acrescentei apenas uma linha de aviso no topo de `ARTCRAFT_RESEARCH.md`, `TECH_STACK_DECISION.md`, `FL_STUDIO_RESEARCH.md` e `FL_STUDIO_ROADMAP.md` apontando para esta revisão. As decisões vinculantes passam a estar em `FINAL_ARCHITECTURE.md`, `UI_FIDELITY_STRATEGY.md`, `IMPLEMENTATION_ROADMAP.md`, `COLLABORATION.md` e `CODEX_FIRST_TASK.md`.

## 7. Questões em aberto (precisam do usuário)

| # | Questão | Por que bloqueia |
|---|---|---|
| Q1 | Há uma instalação **licenciada** do FL Studio 2026 em Windows disponível para capturas? Qual edição e qual build (Help › About)? | Sem ela não há fidelidade medida; o M1.7 fica bloqueado. |
| Q2 | Resolução e escala dos monitores da máquina de referência. | Define as configurações de captura (1×, e 1,5× ou 2×). |
| Q3 | Nome do produto e licença do nosso código (MIT/Apache como o ArtCraft? fechado?). | Afeta `LICENSE`, assets, dependências GPL proibidas e o nome dos crates (`jet-*` é provisório). |
| Q4 | O repositório terá um remoto (GitHub privado)? | Estratégia de branches/PR; hoje a pasta **nem é um repositório Git**. |
| Q5 | Plataformas além do Windows estão no escopo do M1? | Hoje recomendo só Windows. |

## 8. Referências usadas nesta revisão

- Clones locais (scratchpad da sessão) dos commits listados na seção 1.
- Manual: `https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/<página>.htm`, páginas listadas na seção 1, baixadas em 23/10/2026.
- crates.io API, 23/10/2026: egui/eframe/egui_kittest 0.37.0 (15/10/2026) e 0.36.2 (08/09/2026); cpal 0.18.2 (16/08/2026); rtrb 0.4.0; midir 0.11.1; symphonia 0.6.1; clack-host 0.2.0; vst3 0.3.0; assert_no_alloc 1.1.2.
