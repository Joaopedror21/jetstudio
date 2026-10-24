# Auditoria visual da interface (M1)

> **Data:** 23/10/2026 · **Autor:** Claude · **Branch:** `claude/M1-visual-fidelity`
> **Referência mestre:** captura **W**: `reference/screenshots/Captura de tela 2026-10-23 202602.png`, 1567×1029, SHA-256 `3ebdd2e31a082e2d1976b6666970e0a1fc9e9b23905b618e4245a90bdcd192df`. FL Studio 2026 **Trial** no **Windows**, tema padrão, projeto novo, sem cursor visível. O build exato não aparece na imagem; o *Help › About* ainda não foi capturado.
> **Convenções:** `[W x,y]` = medido em W (mediana de região lisa, perfil de pixels para gradientes, cor do miolo dos glifos para texto). `[B]` e `[A]` = outras capturas convertidas para a escala de W. `UNMEASURED` = sem evidência. Todo valor de cor e geometria do código cita a sua origem em `crates/jet-ui/src/theme.rs` e nos comentários dos módulos de pintura.

## 1. Referências disponíveis

| Id | Arquivo | Conteúdo | Uso |
|---|---|---|---|
| **W** | `…202602.png` (1567×1029) | Windows, toolbar completa, Browser (aba SOUNDS), Playlist maximizada, Channel Rack fechado | **Mestre**: toolbar, Browser, Playlist, bordas das janelas, cores |
| A | `…193057.png` (1421×797) | macOS, **outro tema** (mais escuro), imagem reduzida, muitas janelas abertas | Só para a forma dos clipes e as colunas do Channel Rack (escala ×1,79, estimada pela altura da toolbar e pela largura do menu) |
| B | `…193243.png` (975×510) | Recorte do Channel Rack (Loop Starter), tema novo | Cores e forma dos passos, LED e knobs (escala ×1/1,49, estimada pelo passo de 23 px) |
| C | `…193307.png` | Piano roll | Fora do M1 |

A captura A **não** representa o tema padrão: a toolbar dela é `#373C40`, contra `#596267` em W. Todas as cores vêm de W, exceto as do Channel Rack (B), que não aparece em W.

## 2. Por que a interface estava genérica (diagnóstico do código anterior)

O código estava em `crates/jet-ui/src/*` no commit `9a7caef`.

1. **Tokens neutros:** `theme.rs` tinha 10 cinzas `UNMEASURED` (`from_gray(22..240)`), sem nenhuma cor do FL.
2. **Um único "botão" para tudo:** `PaintCtx::button` desenhava retângulo cinza com borda e texto centralizado. Menus de topo, transporte, botões de janela, passos e cabeçalhos de faixa eram todos o mesmo widget, com cara de egui padrão.
3. **Geometria inventada:** toolbar de 92 px (FL: 84), linha de status em português dentro da toolbar, janelas flutuantes nas duas superfícies (FL: Playlist maximizada e Channel Rack fechado), margem preta de 8 px em volta de tudo (o `Ui` raiz tinha margem).
4. **Fonte padrão do egui** (Ubuntu Light), mais larga e de outro desenho.
5. **Sem gradientes, hachuras, LEDs, knobs, LCDs claros ou ícones**: nenhum dos elementos característicos estava desenhado.
6. **Playlist sem sombreamento de 4 compassos, sem linhas de tempo, sem Picker, sem cabeçalhos de faixa e com régua clara.**

Nenhum widget padrão do egui (`Button`, `Slider`…) era usado; o problema era o desenho próprio genérico, não o framework.

## 3. O que foi refeito

| Arquivo | Mudança |
|---|---|
| `theme.rs` | Cerca de 110 tokens medidos em W/B, cada um com origem; `Layout` em pixels de W; fonte Barlow Condensed |
| `widgets.rs` | Primitivas pintadas: gradiente vertical/horizontal (mesh), face de botão da toolbar (borda dupla), face "acesa" laranja, LCD claro, knob, LED, hachura de scrollbar, botões de janela |
| `icons.rs` | 35 ícones próprios desenhados em código (nenhum decalcado) |
| `toolbar.rs` | Toolbar de duas faixas com posições medidas: painel de menu, PAT/SONG, Play/Stop, Record, LCD de tempo, painel de tempo, botões, osciloscópio, CPU, controles da janela, painel de hint, atalhos, snap, seletor de pattern, botões de toque |
| `browser.rs` | Barra de título, abas em texto (ALL…STARRED) com contorno laranja, linha de navegação, árvore de pastas |
| `channel_rack.rs` | LED verde, knobs de pan/volume (mostram o valor do modelo), caixa do mixer, botão do canal colorido, seletor, passos com entalhe e grupos alternados cinza/vermelho |
| `playlist.rs` | Ferramentas na barra de título, título com separadores, Picker, aba inclinada com NOTE/CHAN/PAT, faixa de overview, régua escura, cabeçalhos de faixa arredondados com LED, grade com sombreamento de 4 compassos, linhas de compasso, de tempo e de faixa, scrollbars hachuradas, clipes coloridos |
| `workspace.rs` | Projeto novo = Playlist **maximizada** e Channel Rack **fechado** [W]; Channel Rack com largura para 16 passos |
| `app.rs` | Pintura de borda a borda (sem margem), separador Browser/desktop, bordas das janelas, instalação das fontes antes do 1º frame, linhas vazias "Track N" como no FL (clicar cria as faixas reais), `pending_show` para a ferramenta de captura |
| `jet-model` | Arranjo padrão chamado "Arrangement" e tempo padrão 130 BPM, os dois como em W. **Únicas mudanças fora da UI** |
| `examples/shot.rs`, `tools/compare/` | Captura sem janela (wgpu) e comparador por região |

**Comportamento preservado:** os comandos, o modelo e o motor não mudaram, salvo os dois valores padrão acima. Um teste do motor que assumia 120 BPM implicitamente passou a definir o tempo de forma explícita.

## 4. Posição dos painéis [W]

| Elemento | Retângulo (x0-x1, y0-y1) |
|---|---|
| Toolbar | 0-1566, 0-81; linhas `#424B50` em y 0, 41 e 81; borda do desktop em y 82-83 |
| Browser | 0-489, 84-1028 |
| Separador Browser/desktop | 490-499 |
| Janela Playlist (maximizada) | 500-1566, 84-1028 |
| Picker (lista de patterns) | 500-638, a partir de y 148; scrollbar hachurada em 639-659 |
| Cabeçalhos de faixa | 660-760; calha em 761-770 |
| Grade | 771-1546; scrollbar vertical em 1547-1566 |

## 5. Toolbar [W]

**Faixa 1** (y 1-40):
- Painel de menu: x 4-296, fundo `#5F7374`, borda `#415556`. Palavras em x 9, 36, 62, 91, 145, 178, 227 e 267 (FILE…HELP); altura das maiúsculas 12 px.
- PAT/SONG: 306-336.
- Play: 337-369. Stop: 370-400.
- Record: círculo com centro 419,20.
- LCD de tempo: 437-490 × 8-33, fundo `#C0CFD6`, borda `#7E8D94`.
- Painel de tempo: 500-628, fundo `#536566`.
- Botões de gravação: 636, 669, 702 e 735, cada um com 30×33; o primeiro aparece aceso, laranja `#F28B3D→#FFA153→#FFD88A`.
- Painéis: 768-858, 862-1030 (osciloscópio) e 1035-1206 (CPU/memória).
- Controles da janela: 1502-1560 × 11-28.

**Faixa 2** (y 42-80):
- Hint: 4-296, fundo `#3E464B`.
- Atalhos de janela: 304 + 33k, cada um com 30×33.
- Ímã: 537. Snap "Line": 555-617.
- Botão: 627-643. LCD do pattern: 644-746. "+": 747-763.
- Shift/Ctrl/Alt/?/carrinho: 771 + 33k.
- Notícias: 938-1104.

A face dos botões tem borda externa `#2A3338`, borda interna `#323B40` e gradiente `#444D52→#525B60`.

## 6. Browser [W]

- Barra de título `#434C51` (y 84-105) com "▸ ↑ ↶ Browser" (texto `#829096`).
- Abas (y 106-138): fundo `#1F282D`; rótulos centrados em x 40, 121, 202, 283, 364 e 445; aba ativa com contorno e texto `#DA833A`, cantos arredondados, 78×26.
- Linha separadora `#2C353A` (y 139-141); corpo `#253035`.
- W mostra a aba SOUNDS (conteúdo on-line); o JetStudio abre na aba ALL com as pastas do usuário.

## 7. Geometria do Channel Rack (derivada de B ×1/1,49 e de A ×1,79)

O Channel Rack **não aparece em W**, então tudo aqui é derivado de outras capturas.

- **Linha:** 29 px [B 42,5 px; A 16,7 px].
- **Colunas a partir da borda esquerda:** LED 12 (raio 4) · pan 29,5 · volume 47 (knobs de raio 6) · caixa do mixer 59-75 · botão do canal 80-181 · seletor 186-190 · passos a partir de 201.
- **Passos:** 11,5 × 17,5, a cada 15,5 px, com um entalhe a 3 px do topo [B].
- **Cores [B]:** fundo `#4E5C65`; passo desligado `#585F67→#4B525A→#434A52`; grupo alternado `#795D5B→#614543`; passo ligado `#AAC6D2→#93AFBB`; ligado no grupo alternado `#D09C9D`; LED `#ABE15B`.
- **Estimativa:** a conversão de escala foi estimada com 2 medidas em cada captura, com incerteza de cerca de ±1 px por coluna em W. É preciso uma captura do Channel Rack aberto no Windows.

## 8. Geometria da Playlist [W]

- **Barra de título** `#3F484D` (y 84-105). Ferramentas em x 513, 534, 566, 588, 608, 628, 650, 671, 692, 713 e 735. Título "Playlist - Arrangement ▸ (none) ▸" a partir de x 782 (texto `#CED5D8`).
- **Faixa de overview:** x 770-1545 × 107-125, fundo `#4F5D64`, bordas `#636C71`.
- **Régua:** y 128-144, fundo `#212D34`; números `#82888C` em barra+3; bordas `#2D3940` e `#1B272E`.
- **Faixas:** 48 px a partir de y 148. Separação em 3 linhas: `#2A3A44`, `#182832`, `#2A3A44`.
- **Cabeçalho de faixa:** fundo `#495357`, topo `#525459`, borda `#1D262B`, realce `#525B60`; nome "Track N" `#788084` em +3,+8; LED com centro em x 750, y +37.
- **Compassos:** 51,5 px; tempos de 12,875 px.
- **Sombreamento alternado a cada 4 compassos:** `#34444E` / `#2E3E48`. Linhas de tempo `#22323C` / `#1C2C36`; linhas de compasso `#10202A` / `#0A1A24`.
- **Picker:** fundo `#313C45`; item `#495358` (topo `#50595E`) em 508-637 × 151-175; marcador `#E64363`.

## 9. Tipografia

- O W usa **ClearType** (franjas coloridas nos glifos). O egui renderiza em tons de cinza, então um diff de pixels nunca chega a zero em texto. A comparação de texto usa métricas: posição, altura e largura.
- A fonte original não foi identificada. Testei 13 fontes livres pela razão largura/altura de 4 textos de W ("FILE…HELP", "PATTERNS", "Track 1", "Pattern 1"). A **Barlow Condensed** teve o menor erro médio (11-12%, contra 19-23% das demais), com licença SIL OFL 1.1 (ver `ATTRIBUTION.md`).
- Como a Barlow ainda é 6-20% mais larga, cada palavra do menu é reduzida até caber na **largura medida** em W. As posições ficam exatas e a altura fica cerca de 1 px menor.
- **Tamanhos:** menu 17 px Medium · títulos 15,5 · abas 14 Medium · LCD 18 · relógio 40 Light com segundos 27.

## 10. Estados e aparência dos controles

| Controle | Estado medido | Estado inferido ou faltando |
|---|---|---|
| Botão da toolbar | normal [W], aceso laranja [W] | hover e pressionado: `UNMEASURED` (clareado 12%) |
| PAT/SONG | PAT aceso, SONG apagado [W] | SONG aceso: mesma face laranja (inferido) |
| Play/Stop | normal [W] | tocando: ícone de pausa laranja (inferido) |
| Aba do Browser | ativa [W] | hover `UNMEASURED` |
| Passo | ligado e desligado, nos dois grupos [B] | hover `UNMEASURED` |
| Menu aberto | — | **sem captura**: tamanho e cores `UNMEASURED` |
| Janela flutuante | — | **sem captura** de janela não maximizada: borda e alça `UNMEASURED` |

## 11. Ciclos de comparação

Ferramentas: `cargo run -p jet-ui --example shot -- out.png` (sem janela, wgpu, 1567×1029, projeto novo) e `python -I tools/compare/compare.py REF OURS tools/compare/regions_w.json OUT`. Os artefatos ficam em `references/comparisons/` (local, fora do Git).

| Região | Antes (`9a7caef`) | Ciclo 1 | Ciclo 2 | Ciclo 3 | Janela real |
|---|---:|---:|---:|---:|---:|
| Tela inteira (MAE 0-255) | 46,09 | 8,18 | 8,05 | 8,05 | 8,88 |
| toolbar faixa 1 | 70,78 | 14,64 | 13,60 | 13,67 | — |
| painel de menu | 81,19 | 27,60 | 20,88 | 20,88 | — |
| grade da Playlist | 47,78 | 1,17 | 1,17 | 1,17 | 2,05 |
| cabeçalhos de faixa | 57,37 | 7,49 | 6,50 | 6,50 | — |
| Picker | 37,82 | 1,72 | 1,71 | 1,71 | — |
| barra do Browser | 43,39 | 1,85 | 1,85 | 1,85 | — |

A "janela real" é o `target/release/jetstudio.exe` executado e capturado por `PrintWindow` (1567×1029). Ela confirma que o app real desenha o mesmo que a captura sem janela; a diferença de 0,8 vem de um deslocamento de 1 px.

**As 5 maiores diferenças de cada ciclo, e o que foi feito:**

- **Ciclo 1:**
  1. palavras do menu sobrepostas: cada palavra passou a caber na largura medida;
  2. tempo padrão 120 contra 130: padrão do projeto agora é 130;
  3. cabeçalhos de faixa retangulares: cantos direitos arredondados e faixa clara no topo;
  4. faltava a aba inclinada sobre os cabeçalhos: desenhada com os ícones;
  5. Picker sem "▸" branco e sem o botão da scrollbar: adicionados. O título também ganhou os separadores em triângulo.
- **Ciclo 2:**
  1. aba inclinada 20 px fora do lugar: corrigida;
  2. "---" pequeno: 17 px;
  3. painel de CPU sem subdivisões: duas colunas com duas linhas;
  4. osciloscópio sem o ícone de olho: adicionado;
  5. botões não implementados esmaecidos: mantido, por honestidade (§12).
- **Ciclo 3:**
  1. Channel Rack largo demais (sobrava área à direita dos passos): largura padrão de 470.

  O restante está no §12.

## 12. Diferenças que ainda existem

1. **Texto:** ClearType contra tons de cinza, e a fonte não é a original. É o maior resíduo na toolbar e nos títulos (MAE de 13-28 nessas regiões).
2. **Primeiro botão de gravação aceso** em W: a função não foi identificada (o *hint* do FL ao passar o mouse diria qual é). Mostramos apagado e desabilitado.
3. **Botões não implementados** (piano roll, mixer, plugin picker, entrada de áudio, Shift/Ctrl/Alt, ajuda, loja, opções de gravação) têm ícone esmaecido; no FL eles são claros. É escolha deliberada: não fingir funções que não existem.
4. **Conteúdo on-line:** a aba SOUNDS, o painel de notícias e a memória "337 MB" não são reproduzidos. A aba ativa é ALL, e a memória fica em branco até ser medida de verdade.
5. **Hint:** W mostra "(Trial)" (status de licença); mostramos o nome do projeto.
6. **Ícones:** desenhos próprios simplificados, com silhueta diferente da do FL. É intencional por propriedade intelectual, mas os tamanhos e as posições podem ser refinados.
7. **Channel Rack, clipes, menus abertos, hover e janela restaurada** não têm captura no Windows. Valores derivados de A/B ou `UNMEASURED`.
8. **Linha da Playlist com 48 px:** é o zoom vertical padrão de W. O zoom vertical ainda não é ajustável.

## 13. Capturas pedidas ao usuário

Todas no mesmo perfil de W (Windows, escala de 100%, tema padrão, FL maximizado):

1. *Help › About* (build).
2. Channel Rack aberto (F6) com 4 canais e alguns passos.
3. Um Pattern Clip na Playlist.
4. Cada menu do topo aberto.
5. Mouse sobre o 1º botão aceso da faixa 1, para ler o *hint*.
6. Mouse sobre o Play (hover).
7. Uma janela restaurada (não maximizada).
8. Aba ALL do Browser.

Cada nova captura substitui os valores derivados ou `UNMEASURED` correspondentes.

## Revisões

| Data | Mudança |
|---|---|
| 23/10/2026 | Criação: diagnóstico, medições em W/A/B, reescrita da camada visual, 3 ciclos de comparação e validação na janela real |
