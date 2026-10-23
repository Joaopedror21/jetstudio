# Método para reproduzir e verificar a interface

**Pesquisa em 23/10/2026.** Objetivo: construir uma interface e uma experiência de interação altamente fiéis à versão de referência do FL Studio, por observação e implementação própria. As seções “Encontrado” descrevem práticas verificadas no código/documentos públicos das aplicações Craft; “Proposto” descreve o protocolo que recomendamos adotar. Uma captura semelhante isolada não comprova equivalência de comportamento.

## 1. O que a pesquisa encontrou

| Técnica | Evidência pública | Limite da conclusão |
|---|---|---|
| Aparência parametrizada | O [PhotoCraft `ui-design.md`](https://github.com/storytold/photocraft/blob/main/docs/ui-design.md) documenta variantes de tema, tokens de cores e raios, widgets compartilhados, fonte e gramática de layout. Implementação em [`theme.rs`](https://github.com/storytold/photocraft/blob/main/crates/ui-egui/src/theme.rs) e [`widgets.rs`](https://github.com/storytold/photocraft/blob/main/crates/ui-egui/src/widgets.rs). Ex.: o tema Pro documenta painéis `#323232`, acento `#378ef0` e tipo compacto de 12 px. | São valores **do PhotoCraft**, não medições do FL Studio, e não documentam o processo histórico de extração desses valores. |
| Hierarquia de painéis | O tema Pro do PhotoCraft especifica ordem de dock; [`dock.rs`](https://github.com/storytold/photocraft/blob/main/crates/ui-egui/src/dock.rs) guarda ordem, alturas e grupos recolhidos. O [PR #139](https://github.com/storytold/photocraft/pull/139) descreve redimensionamento, reordenação, persistência e testes em janelas pequenas. | O PR testa o produto reimplementado, não um diff pixel a pixel contra Photoshop. |
| Componentes e tipografia | `widgets.rs` centraliza botões, campos, separadores, labels truncados; `ui-design.md` cita Inter/JetBrains Mono e ícones Lucide sob licença própria. | Ícones e fontes são alternativas licenciadas, não cópias dos recursos proprietários. |
| Captura determinística do próprio app | [`examples/snapshot.rs`](https://github.com/storytold/photocraft/blob/main/crates/ui-egui/examples/snapshot.rs), [`development.md`](https://github.com/storytold/photocraft/blob/main/docs/development.md) e o protocolo [`ui.screenshot`](https://github.com/storytold/photocraft/blob/main/docs/control-protocol.md) permitem render offscreen/captura da janela. `ui-design.md` cita fixture com painéis reais em 800×480 e 1280×720 pontos, escalas 1×/1,5×/2× e 60 PNGs iniciais/rolados. | Isso prova regressão visual interna e inspeção humana; não encontramos afirmação de comparação automatizada sistemática com screenshots do Photoshop. |
| Gestos, foco e atalhos | [`ui-parity.md`](https://github.com/storytold/photocraft/blob/main/docs/ui-parity.md) é uma lista por ferramenta e gesto com estados done/partial/missing; [`shortcut_dispatch.rs`](https://github.com/storytold/photocraft/blob/main/crates/ui-egui/src/shortcut_dispatch.rs) resolve foco e atalhos. O [PR #141](https://github.com/storytold/photocraft/pull/141) relata teste dos atalhos exibidos em menu sob quatro focos. | A lista reconhece diferença em interações ainda parciais ou ausentes. |
| Controle programático | O [`control-protocol.md`](https://github.com/storytold/photocraft/blob/main/docs/control-protocol.md) oferece `ui.inspect`, `ui.set`, `ui.pointer`, `ui.key`, menu e screenshot, com validação de parâmetros. O [SoundCraft AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md) relata `ui_shot` offscreen e comparação humana com screenshots locais de Pro Tools. | A captura de referência do SoundCraft fica sob `plan/` ignorado; não a auditamos. |
| Evidência datada de menus da referência | [`context-menu-parity.md`](https://github.com/storytold/photocraft/blob/main/docs/context-menu-parity.md) separa itens documentados dos inferidos. Registra uma screenshot do Photoshop fornecida em 07/10/2026 para um menu da Pen, com ordem, separadores e estados desabilitados, e exige outras capturas para nó, segmento e canvas vazio. | É uma captura de um contexto específico, não evidência de todos os menus ou de medição automática de pixels. |
| Oráculo de saída | O [corpus do PhotoCraft](https://github.com/storytold/photocraft-corpus) cria documentos no Photoshop por script e compara o composto gerado pelo Photoshop ao renderizador próprio. | Trata conteúdo/imagem e formato de documento, não garante a geometria da interface. |

**Conclusão factual:** os projetos usam especificações de UI, tokens, widgets próprios, listas de paridade, testes de gestos, automação de estados e captura do app reimplementado. A documentação pública consultada **não fornece** uma receita única comprovada de extração automática de pixels do app proprietário nem um erro visual global “1:1” medido.

## 2. Preparar a referência do FL Studio — proposto

1. **Fixar a versão**: número/build do FL Studio, edição e sistema operacional. Atualizações podem mover controles e alterar ícones/menus; evidências só são comparáveis para a mesma versão. Registrar idioma, tema, escala do FL Studio, escala de sistema, resolução, DPI, cor de monitor e preferências de acessibilidade.
2. **Criar projetos de referência próprios e pequenos**: vazio; Channel Rack com quatro canais; padrão com steps; piano roll com notas e velocidades; Playlist com Pattern/Audio/Automation Clips; Mixer com roteamento e efeito; Browser em diversas abas. Cada fixture deve ter uma sequência de ações e estado final conhecido. Não usar loops, presets, imagens, logos ou assets proprietários como material distribuído.
3. **Capturar estados**: normal, hover, pressed, foco por teclado, menu aberto, popup, drag em andamento, item selecionado, painel flutuante, dock, zoom e scroll extremos, janela estreita e ampla. Capturar apenas a janela de referência, com consentimento e condições estáveis, para uso interno de comparação.
4. **Anotar comportamento**: para cada gesto, origem/destino, modificadores, foco, arredondamento/snap, preview, commit/cancel, efeito em undo/redo, arquivo salvo e áudio resultante. Marcar evidência da documentação oficial ou observação e incertezas separadamente.
5. **Priorizar superfícies**: primeiro shell/toolbar/transporte, Channel Rack e suas interações; depois Piano Roll, Playlist, Mixer e Browser. A ordem de implementação real depende do roadmap funcional, mas a matriz visual deve começar abrangente para evitar uma arquitetura que só suporta uma janela fixa.

### Registro mínimo de uma observação

| Campo | Exemplo de registro (fictício; preencher por medição) |
|---|---|
| ID e versão | `UI-CH-001`, FL Studio versão/build observado |
| Superfície e estado | Channel Rack, pattern vazio, janela 1440×900 em 100% |
| Captura | Caminho interno + hash SHA-256 + recorte da janela |
| Ação | Clique, arraste ou tecla; coordenadas relativas ao controle; modificadores |
| Resultado | Estado visual, comando/estado, efeito audível, undo/redo |
| Medidas | Retângulos `[x,y,w,h]` em pixels físicos e em pontos lógicos |
| Confiança | Documentação oficial / observação repetida / hipótese pendente |
| Critério | Diferenças admissíveis para geometria, cor e comportamento |

## 3. Medir geometria, cor, fontes e ícones — proposto

### Coordenadas e dimensões

- Tratar **ponto lógico** e **pixel físico** como grandezas diferentes. Registrar escala do SO, escala da aplicação e escala de captura. Medir borda externa, título, toolbar, slots do dock, separadores, padding, grade e área de clique, com mesma origem. Uma captura com escala 150% não pode ser comparada diretamente com uma renderização 100%.
- Marcar retângulos por hierarquia: janela → região → painel → linha → controle → glifo/ícone. Guardar coordenadas relativas ao pai e também absolutas. Para layout ajustável, medir *regras* (largura mínima, prioridade de compressão, ancoragem e divisão de espaço), não só dimensões de uma screenshot.
- Comparar pelo menos janela larga, estreita, alta e curta, além de escalas 1× e 1,5× ou 2×. Exercitar painéis redimensionados/flutuantes. O teste de PhotoCraft em 800×480, 1280×720 e várias escalas é um precedente verificável em seu [`ui-design.md`](https://github.com/storytold/photocraft/blob/main/docs/ui-design.md), não um tamanho obrigatório para FL Studio.
- Separar retângulo visual de **hitbox**. Testar onde a interação começa/termina, a prioridade quando hitboxes se sobrepõem e o modo de captura do ponteiro durante drag. O [PR #141 do PhotoCraft](https://github.com/storytold/photocraft/pull/141) mostra por que retângulo e clique não podem ser considerados equivalentes por aparência.

### Cores e estados

- Amostrar regiões planas *longe* de texto, bordas, transparência e antialiasing. Registrar valores sRGB/alpha, variação entre pixels e se a captura tem perfil de cor. Uma média de toda a região mistura superfícies e glifos e não identifica o token base.
- Inventariar tokens semânticos: fundo do shell, barra, título de painel, separador, grade, linha alternada, texto primário/secundário, acento, seleção, hover, pressed, disabled, foco, drop target e warning. Aplicar cada token por estado. PhotoCraft centraliza esse tipo de escolha em [`theme.rs`](https://github.com/storytold/photocraft/blob/main/crates/ui-egui/src/theme.rs); cores dispersas em dezenas de widgets tornariam calibração lenta.
- Comparar estados sob mesmo gamma/perfil e mesma plataforma. Para componentes translúcidos, comparar a composição final e guardar a fórmula de blending no modelo próprio.

Para menus, capturar e transcrever **ordem, separadores, checagens, itens desabilitados e diálogos** por contexto; marcar cada linha como documentada, observada ou inferida. O [contrato de menus contextuais do PhotoCraft](https://github.com/storytold/photocraft/blob/main/docs/context-menu-parity.md) mostra por que uma captura do menu sobre um clipe ou nota não valida o menu sobre área vazia ou outro objeto.

### Tipografia

- Medir família aproximada, altura de x, largura de glifo, peso, tamanho, baseline, espaçamento entre letras, alinhamento vertical e elisão. Uma “fonte de 12 px” sozinha não basta. Verificar numerais em BPM, time ruler e parâmetros do Mixer, onde a largura fixa importa.
- Preferir fonte livre com métricas compatíveis, embutida com licença e fallback explícito. Renderização de texto varia por OS e DPI; exigir comparação visual por plataforma. O [PhotoCraft `ui-design.md`](https://github.com/storytold/photocraft/blob/main/docs/ui-design.md) documenta famílias e elisão como decisões de design.

### Ícones e controles personalizados

- Inventariar silhueta, stroke, bounding box, alinhamento, cor por estado e hitbox. Desenhar ícones próprios ou usar biblioteca compatível com licença, mantendo atribuição. Não extrair SVG/PNG dos recursos do FL Studio.
- Modelar controles como widgets reutilizáveis com visual e comportamento separados: botão de transporte, knob, fader, medidor, seletor de step, régua, nota MIDI, clipe, menu contextual. Um widget deve expor estados e entrada por mouse, teclado e automação.
- Conferir casos de clipping e z-order: menus sobre painéis, tooltips, janelas flutuantes, seleção e drag ghost. PhotoCraft documenta menu com scroll e regras de janela estreita em [`ui-design.md`](https://github.com/storytold/photocraft/blob/main/docs/ui-design.md).

## 4. Modelar comportamento da interface — proposto

Cada superfície deve ter uma máquina de estados explícita, por exemplo `Idle → Armed → Dragging → Preview → Committed/Cancelled`. Definir por transição: evento de entrada, coordenadas (tela/painel/tempo musical), modificadores, snap, alterações temporárias, comando do engine e efeito no histórico. Isso evita o erro comum de criar um layout parecido em que os gestos do FL Studio fazem algo diferente.

| Área | Casos que devem ser observados e testados |
|---|---|
| Barras e menus | Ordem, atalhos exibidos, habilitação condicional, menu contextual, foco, navegação de teclado, popup em borda de janela. |
| Painéis/dock | Abrir/fechar, float/dock, z-order, redimensionar, persistir workspace e restaurar; comportamento em tela pequena. |
| Channel Rack | Step on/off, foco de canal, menus de canal, drag de ordem, zoom/scroll horizontal, preview antes de soltar. |
| Piano Roll | Nota criar/mover/redimensionar, seleção, velocity, snap, zoom horizontal/vertical, autoscroll, teclas/modificadores. |
| Playlist | Criar/mover/redimensionar Pattern/Audio/Automation Clips, corte, seleção, snap, Track Mode e drag entre superfícies. |
| Mixer | Seleção de insert, fader/knob, roteamento, slot de efeito, medidor, scroll horizontal e interação durante reprodução. |
| Browser | Árvore, filtro, preview, drag/drop e foco em combinação com atalhos globais. |

Para cada caso: teste de estado do engine, teste de evento UI e captura do estado visual. Os atalhos devem sair de um mapa único com regras de foco; [`shortcut_dispatch.rs`](https://github.com/storytold/photocraft/blob/main/crates/ui-egui/src/shortcut_dispatch.rs) e o [PR #141](https://github.com/storytold/photocraft/pull/141) são evidência de que foco em campo/painel é uma fonte real de falhas.

## 5. Comparar screenshots de forma útil — proposto

### Pipeline

1. Reproduzir a mesma fixture e estado nas duas aplicações, com janela, DPI e tema fixos. Guardar metadata, versão, sequência de comandos e hashes das capturas.
2. Recortar apenas a janela. Registrar o retângulo, sem “ajustar no olho”. Alinhar pelo chrome/âncoras fixas; limitar a correção de translação a um deslocamento pequeno e registrar o valor. Se foi necessário warp/escala arbitrária para casar telas, o layout falhou.
3. Criar máscaras para regiões realmente dinâmicas: playhead em movimento, medidores, relógio, caret piscante e variações de renderizador de fonte. Preferir congelar estado/tempo a mascarar. Toda máscara deve ser documentada; nunca mascarar o componente inteiro que se deseja avaliar.
4. Calcular diferença absoluta por canal em sRGB normalizado, mapa de calor e métricas por região: erro médio absoluto, percentil 95, fração acima do limiar e deslocamento de bordas/retângulos. Um único escore global esconderia um fader incorreto numa tela grande.
5. Gerar sobreposição 50/50 e imagens lado a lado para revisão humana. Registrar defeitos como geometria, tipografia, paleta, ícone, estado ou comportamento. Consertar da hierarquia externa para o glifo: shell → painel → widget → detalhe.
6. Rodar o conjunto de capturas após mudanças em tokens/widgets/layout e após mudanças de comportamento com efeito visual. Revisar snapshots novos antes de aceitar a baseline. Guardar baseline por SO e escala quando rasterização diferir legitimamente.

**Critérios iniciais sugeridos, não resultados medidos:** bordas de regiões principais dentro de ±2 px em escala 1×; nenhum controle essencial cortado em tamanhos suportados; cores planas dentro de tolerância definida por token; todos os fluxos prioritários com eventos, estado e undo/redo equivalentes. Ajustar tolerâncias só depois de capturar variabilidade real de mesma versão/plataforma. Para texto/antialiasing, usar máscara fina ou métrica específica, nunca aceitar grande desvio geométrico porque o pixel diff bruto é ruidoso.

### Duas famílias de teste

- **Regressão interna:** capturas da nossa própria aplicação comparadas a baselines nossas em CI, como os snapshots offscreen do PhotoCraft. Detecta mudança acidental, mas uma baseline errada continuará errada.
- **Paridade externa:** comparação com capturas permitidas da referência em ambiente controlado e inspeção humana dos resultados. É o teste que responde à pergunta “quão parecido com o FL Studio?”; não há evidência de que o PhotoCraft publique um harness geral equivalente para a UI do Photoshop. Para funções de processamento, complementar com arquivos e renderizações de saída, tal como o [corpus do PhotoCraft](https://github.com/storytold/photocraft-corpus).

## 6. Qualidade, rastreabilidade e clean-room

- Cada linha da matriz de paridade deve conter **versão de referência, passos reproduzíveis, resultado esperado, evidência, status `feito/parcial/ausente` e teste**. Não contar menus implementados como prova de UX equivalente: o [PhotoCraft target-app-parity.md](https://github.com/storytold/photocraft/blob/main/docs/target-app-parity.md) distingue amplitude funcional de prontidão real.
- Observar o aplicativo e consultar sua documentação e especificações públicas; implementar código e assets próprios. SoundCraft proíbe copiar recursos do bundle do Pro Tools em seu [AGENTS.md](https://github.com/storytold/soundcraft/blob/main/AGENTS.md); PhotoCraft registra a regra clean-room no [AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md).
- Screenshots da referência são material de comparação interno; não devem ser incorporados ao aplicativo, assets de distribuição ou testes publicados sem uma base de direitos adequada. Test fixtures distribuíveis devem usar conteúdo original ou licenciado.
- Onde a observação for ambígua, anotar hipótese e experimento que a discriminaria. Não inferir algoritmo interno do FL Studio a partir de uma tela ou resposta isolada.

## 7. Primeira aplicação deste método

Antes da implementação da UI, montar uma matriz de 20–30 cenários observáveis da versão de referência, cobrindo: abertura/fechamento e dimensões do shell, transporte PAT/SONG, Channel Rack com múltiplos canais, uma edição de Piano Roll, um Pattern Clip e um Audio Clip na Playlist, navegação do Browser, uma alteração de volume/roteamento no Mixer, undo/redo, salvar/reabrir. Capturar estados nas escalas e tamanhos escolhidos, medir tokens e retângulos de alto nível e definir os primeiros testes de equivalência. É uma **recomendação** derivada das práticas públicas acima, não um passo histórico comprovado do ArtCraft.
