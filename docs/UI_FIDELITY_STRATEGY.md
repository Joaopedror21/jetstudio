# Estratégia de fidelidade visual e comportamental

> **Status:** vinculante, a partir de 23/10/2026. Complementa `UI_REPLICATION_METHOD.md` (Codex), que continua válido como discussão de método. Onde os dois divergem, prevalece este documento.
> **Regra zero:** **nenhum número de geometria, cor ou tipografia do FL Studio aparece neste documento nem no código sem uma medida rastreável** de uma captura da referência. Os números de exemplo aqui são **tolerâncias e procedimentos**, não medidas do FL.

## 1. Ponto de partida honesto

- O ArtCraft **não** comprovou fidelidade medida contra a referência. No SoundCraft, as cores foram "chosen by eye" (`theme.rs:1`), e a ausência de um harness contra o Pro Tools está registrada como lacuna G-07. Ver `CLAUDE_REVIEW.md` E2.
- O que o ArtCraft oferece de reutilizável é **infraestrutura de verificação do próprio app**: Harness offscreen (`egui_kittest` + wgpu), asserções sobre `FullOutput.shapes` (SoundCraft `mix_strip_layout.rs`), *reference painters* independentes com testes de mutação (`craft-libs/crates/ui/tests/visual_regression.rs`) e inspeção por protocolo de controle.
- O que falta construir, e construiremos, é: (1) um protocolo de captura da referência, (2) uma spec medida e rastreável, (3) a comparação spec ↔ app e captura ↔ app.

## 2. Perfil de referência (o que fixar antes da primeira captura)

Uma captura só é comparável com outra feita no **mesmo perfil**. O perfil é um arquivo versionado, `fidelity/profiles/<id>.toml`, sem imagens.

| Variável | Valor do perfil `win-100` (proposto) | Fonte da variável |
|---|---|---|
| SO | Windows 11 x64, build registrado | — |
| FL Studio | 2026, build exato de *Help › About* (o publicado mais recente é 26.1.7.5, de 12/10/2026) | `WhatsNew.htm` |
| Idioma | inglês | — |
| Escala do SO | 100% | — |
| Main GUI scaling | "System" | `envsettings_general.htm:8` |
| Pop-ups / Toolbars / Cursor scaling | "Main" | `envsettings_general.htm:14-15,189` |
| Animations | **"Don't distract me"** (desliga animações, para capturas determinísticas) | `envsettings_general.htm:16` |
| Transparent windows / menus | Off | `envsettings_general.htm:17-19,29` |
| Thick lines / High visibility | Off | `envsettings_general.htm:25-26` |
| Tema | tema padrão de fábrica (F10 ou Options › Theme settings) | `basics_interface.htm` |
| Toolbar | preset de fábrica padrão; "Flat buttons" e separadores no padrão | `toolbar_panels.htm` |
| Janela principal | maximizada em monitor 1920×1080, e também uma janela restaurada de tamanho registrado | — |
| Perfil de usuário | instalação limpa ou preferências resetadas; nenhuma pasta pessoal no Browser além das fixtures | `browser.htm` |
| Áudio | dispositivo e buffer registrados (para capturas de CPU e medidores, que ficam fora do M1) | `envsettings_audio.htm` |

Um segundo perfil, `win-150` (SO a 150%), entra depois que o `win-100` estiver estável, para validar as regras de escala.

## 3. Coleta de referências

### 3.1 Quem coleta e onde ficam

- **Quem:** o usuário, na máquina com o FL Studio licenciado, usando os scripts que o Codex vai escrever (`tools/capture/`). Agentes não operam a instalação de referência sem o usuário presente.
- **Onde:** `reference/` na raiz, **no `.gitignore`**. Opcionalmente, um repositório privado separado, `jetstudio-reference`, fixado por hash, como o corpus do PhotoCraft. **Nunca no repositório do produto.**
- **O que pode ser versionado:** números medidos (retângulos, cores, métricas de texto), sempre com `capture_id` e SHA-256 da captura de origem, e transcrições de menus (nomes, ordem, separadores, estados).
- **O que nunca é versionado nem distribuído:** as capturas, recortes de ícones e qualquer arquivo do diretório de instalação.

### 3.2 Requisitos da ferramenta de captura (`tools/capture`, a implementar)

1. Capturar **somente a janela** do FL Studio por handle, em pixels físicos, PNG sem perda. O cursor do mouse fica **excluído**.
2. Gravar um *sidecar* `<capture_id>.json` com: perfil, build do FL, resolução, DPI do monitor, retângulo da janela, data e hora, SHA-256, descrição do estado e sequência de passos que levou a ele.
3. O `capture_id` é estável: `<perfil>/<superfície>/<cenário>/<estado>`, por exemplo `win-100/channel-rack/two-channels/step3-hover`.
4. A biblioteca de captura no Windows (Windows.Graphics.Capture, PrintWindow ou um crate como `xcap`) é escolha de implementação. O requisito é a saída acima, validada comparando duas capturas consecutivas do mesmo estado estático: elas devem ser **idênticas bit a bit**. Se não forem, existe ruído (animação, relógio, medidor) a ser eliminado no perfil antes de qualquer medição.

### 3.3 Catálogo de cenários do M1

Cada cenário é um roteiro de passos reproduzível (`fidelity/scenarios/<id>.md`), começando de um estado inicial definido.

| Cenário | Estados a capturar |
|---|---|
| `shell/startup` | primeira tela após abrir um projeto vazio da fixture |
| `shell/toolbar` | toolbar completa; hover em cada botão de transporte; PAT e SONG; tempo em edição; menu FILE aberto (e cada menu de topo) |
| `shell/windows` | Channel Rack e Playlist flutuantes; Playlist maximizada; duas janelas sobrepostas (foco e z-order); janela sendo arrastada perto de outra (encaixe); Browser oculto e visível |
| `browser/tree` | Browser com uma pasta de fixture expandida; item hover e selecionado; arraste em andamento |
| `channel-rack/basic` | 1, 2 e 4 canais; passos ligados e desligados; hover em passo; canal selecionado; knobs em 0%, 50% e 100%; menu do canal |
| `playlist/basic` | régua; 3 faixas; um Pattern Clip; clipe selecionado; clipe em arraste; zoom mínimo, padrão e máximo |
| `transport/play` | PAT tocando (playhead), SONG tocando, Stop. Capturas com o playhead parado em posição conhecida, usando Pause |

As fixtures de projeto são **criadas pela equipe** dentro do FL, com conteúdo trivial e original (padrões de passos), e sem samples ou presets da Image-Line nas capturas que serão medidas, quando isso for evitável.

## 4. Medição: da captura para a spec

### 4.1 Ferramenta `cargo xtask measure` (a implementar)

- **Edge-snap:** recebe uma captura e um retângulo aproximado (de uma pessoa ou de um agente com visão) e ajusta cada borda à transição de cor mais próxima dentro de ±N px. Devolve o retângulo exato e a força da borda. **Nenhum retângulo entra na spec sem passar por edge-snap ou por medição manual registrada como `method: manual`.**
- **Amostragem de cor:** mediana numa região plana interna (afastada pelo menos 2 px de bordas e texto), mais o desvio. Se o desvio passar do limiar, a região não é plana e a amostra é recusada.
- **Métricas de texto:** baseline, altura de maiúsculas e de x, avanço da string e alinhamento, extraídos por perfil de projeção do recorte de texto.
- **Conversão de unidades:** tudo é armazenado em **pontos lógicos a 1×** e também em pixels físicos, com a escala do perfil registrada.

### 4.2 Formato da spec (`fidelity/spec/<superfície>.ron`)

```ron
(
  surface: "channel-rack",
  profile: "win-100",
  elements: [
    ( id: "channel-rack.titlebar", parent: "channel-rack.window",
      rect: (x: 0, y: 0, w: 0, h: 0),        // PREENCHER POR MEDIÇÃO; zeros = não medido
      rule: Some(StretchX),                   // regra de layout, não só um retângulo
      source: ( capture: "win-100/channel-rack/two-channels/idle", sha256: "…", method: EdgeSnap ),
      status: Unmeasured ),
  ],
  tokens: [ ( id: "rack.bg", value: None, source: None, status: Unmeasured ) ],
  text: [ ( id: "channel-rack.title", baseline_y: None, cap_height: None, advance: None, status: Unmeasured ) ],
)
```

Os IDs da spec são **os mesmos** `WidgetId` que nossos widgets registram no LayoutProbe. É essa correspondência que torna a comparação automática.

### 4.3 Regras, não só retângulos

Para todo elemento redimensionável, a spec registra a **regra** observada: largura mínima, âncora, o que estica, o que corta e em que ordem os controles desaparecem quando a janela estreita. Por isso cada superfície é capturada em pelo menos três larguras. O manual confirma que a visibilidade de controles depende do tamanho em ao menos um caso (o Mixer, `mixer.htm:13,29`).

## 5. Três camadas de teste

### Camada A — Spec ↔ app (determinística, roda em CI)

- Renderiza o app real no `egui_kittest::Harness` com o tamanho e a escala do perfil. Animações e blink ficam desligados, como em `craft-libs` `visual_regression.rs`.
- Lê o LayoutProbe (`WidgetId → rect`) e as shapes emitidas.
- Para cada elemento `Measured`, verifica o retângulo dentro da tolerância, o token de cor (o valor que nosso tema usa deve ser igual ao valor da spec, porque o token **é** a spec) e as métricas de texto.
- **Testes de mutação** (padrão do `craft-libs`): para cada classe de verificação, um teste aplica uma alteração deliberada (deslocar 1 px, trocar uma cor) e exige que a verificação **falhe**. Isso prova que o teste detecta regressão.
- Tolerâncias iniciais, revisáveis só com dados: retângulo ±1 pt lógico a 1×; cor de token exata; baseline de texto ±1 pt. Uma tolerância maior precisa de justificativa no ADR da superfície.

### Camada B — Regressão interna de pixels (roda em CI, por SO)

- PNGs do nosso app em cenários fixos, comparados com baselines **nossas** do mesmo renderer.
- Detecta mudança acidental, mas **não mede fidelidade**: uma baseline errada continua errada. Atualizar uma baseline exige revisão humana ou do Claude do par antes/depois.

### Camada C — Paridade externa (local, gera relatório)

1. Para cada `capture_id` da referência, renderiza o mesmo cenário no nosso app, com o mesmo tamanho lógico e a mesma escala, a partir do roteiro equivalente (`fidelity/scenarios/<id>.jsonl` para o nosso lado).
2. Alinha por âncoras fixas (canto da janela e da toolbar). A única correção permitida é uma **translação** de até 2 px físicos, que é registrada; escala ou warp **proibidos**.
3. Máscaras, todas documentadas no cenário: playhead em movimento, relógios e regiões de áudio vazias no M1. A máscara nunca cobre o componente avaliado.
4. Métricas **por região** da spec: erro médio absoluto, percentil 95, fração de pixels acima do limiar, deslocamento de bordas detectadas e ΔE de cores planas. Não há escore global único.
5. Saída: `target/fidelity/<data>/report.html` com lado a lado, sobreposição 50/50, mapa de calor e "blink" alternando as duas imagens, mais `report.json` para os agentes.
6. Texto é avaliado pelas métricas de §4.1, não por diff de pixels. A diferença de rasterização entre a nossa fonte e a do FL é esperada e registrada.

## 6. Comportamento (não só aparência)

Cada widget e cada superfície tem uma **máquina de estados** escrita na spec do cenário: evento → transição → comando → efeito no histórico. Para cada transição:

- teste de engine (o comando faz o que deve; undo/redo; caso desabilitado);
- teste de UI (o evento real de ponteiro ou teclado no Harness produz o comando certo);
- captura do estado visual resultante (camadas A, B e C).

Os comportamentos do M1 já documentados no manual estão listados em `FINAL_ARCHITECTURE.md` §4.4. Tudo o que o manual não fixa, como a distância de encaixe das janelas, o toggle de janela visível sem foco, a pintura de passos por arraste ou o redo de Ctrl+Z, é **observado** e registrado como cenário antes de ser implementado.

## 7. Scorecard

`docs/ui-scorecard.md` é **gerado** por `cargo xtask fidelity --scorecard`. Uma linha por superfície × dimensão:

| Superfície | Geometria | Cor | Texto | Estados | Comportamento | Paridade externa |
|---|---|---|---|---|---|---|
| ex.: Toolbar | `n medidos / n total; n passam` | … | … | … | `n transições testadas / n catalogadas` | `última data; p95 por região` |

Status permitidos: `ausente`, `implementado (não medido)`, `medido`, `aprovado em A`, `aprovado em C (data, perfil)`. **É proibido declarar "paridade"** de uma superfície sem `aprovado em C` em todos os elementos `Measured` e sem revisão do Claude registrada.

## 8. Ciclo de correção

1. O relatório C aponta as regiões com pior p95.
2. A correção segue a hierarquia de fora para dentro: shell → janela → painel → widget → glifo. Corrigir o detalhe antes do contêiner gera retrabalho.
3. Um commit por região, com o relatório antes e depois anexado à descrição (só números e caminhos locais, nunca as capturas).
4. O Claude revisa o par antes/depois e marca a mudança como aprovada ou rejeitada no scorecard.

## 9. Limites de propriedade intelectual

- Medir posições, tamanhos e cores para reimplementar um layout é observação. **Ícones, logotipos, ilustrações e fontes do FL Studio são obras protegidas**: nunca extraí-los, decalcá-los ou vetorizá-los a partir de capturas. Os ícones do JetStudio são desenhos próprios que cumprem a mesma função, no mesmo tamanho e posição. A silhueta pode ser semelhante quando ditada pela função (play é um triângulo), mas o desenho é nosso.
- Nada do diretório de instalação do FL é lido, a não ser pela execução normal do programa.
- Antes de qualquer distribuição pública: revisão jurídica sobre *trade dress* e marca. Isto não é aconselhamento jurídico.
