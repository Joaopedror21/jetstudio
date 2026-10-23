# FL Studio: pesquisa funcional para uma implementação independente

> **Nota de revisão (Claude, 23/10/2026):** documento histórico. O manual on-line descreve o FL Studio 2026 (changelog até 26.1.7.5). Correções e fatos acrescentados (Arrangements, encaixe e *Detached* de janelas, toolbar configurável, Ctrl+Z alterna undo/redo, PPQ por projeto, "16 passos" sem evidência no manual, contradição do manual sobre F10) estão em [CLAUDE_REVIEW.md](CLAUDE_REVIEW.md) E6–E8.

**Data da consulta:** 23/10/2026. **Fonte primária:** [manual on-line oficial da Image-Line](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/). Esta página é atualizada pela Image-Line; a versão exata do executável usado para comparações visuais ainda precisa ser fixada. Aqui, **[Fato]** designa comportamento documentado no manual, **[Proposta]** uma decisão para nosso projeto e **[Pendente]** algo que exige observação do aplicativo em uma instalação autorizada. Nenhuma implementação do FL Studio foi iniciada nesta etapa.

## 1. Modelo mental que precisa ser preservado

**[Fato]** O Channel Rack contém os instrumentos e geradores internos; cada *pattern* pode usar os mesmos canais, com dados de sequenciador de passos, Piano roll e automação. Os patterns são colocados na Playlist como Pattern Clips. O áudio de cada canal segue para uma faixa do Mixer. A Playlist, por padrão, aceita Pattern Clips, Audio Clips e Automation Clips em qualquer faixa, inclusive sobrepostos. Uma faixa de Playlist não equivale obrigatoriamente a um canal ou faixa do Mixer. Arrastar instrumento ou amostra sobre o cabeçalho de uma faixa cria, opcionalmente, um vínculo de *Track Mode* entre Channel Rack, Playlist e Mixer. Esse é o maior desvio em relação ao modelo rígido “uma faixa = um instrumento = um canal de áudio” de muitas DAWs. [Channel Rack](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm), [Playlist](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm), [Mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm).

```text
Projeto
 ├─ Canais (instrumentos, amostras, geradores de automação)
 ├─ Patterns (eventos por canal; todos podem acessar os mesmos canais)
 ├─ Playlist (faixas com instâncias de Pattern/Audio/Automation Clips)
 ├─ Mixer (faixas de áudio, inserções, sends, master)
 └─ Recursos externos (amostras, plugins, dispositivos MIDI/áudio)

Pattern Clip ──> Pattern ──> eventos de um ou vários Canais ──> Mixer
Audio Clip ──> amostra/canal de áudio ─────────────────────────> Mixer
Automation Clip ──> parâmetro vinculado (canal, plugin ou Mixer)
```

**[Proposta]** Modelar `PlaylistTrack`, `Channel` e `MixerTrack` como entidades separadas, com IDs estáveis e vínculos opcionais. Um pattern deve mapear eventos por `ChannelId`. Clipes referenciam a fonte, início, duração, recorte e estado local. Essa proposta deriva das relações descritas pelo manual; não é descrição da arquitetura interna da Image-Line.

## 2. Superfície principal e janelas

**[Fato]** A barra superior reúne transporte, gravação, tempo, indicação de posição, snap global, seletor de pattern e comandos de janelas. O modo **PAT** reproduz o pattern selecionado; **SONG** reproduz a Playlist. O snap local do Piano roll, Playlist ou Event Editor pode herdar o snap global usando “Main”. A visualização do tempo alterna entre barras/batidas/passos/ticks e tempo real. Atalhos globais incluem **F5** Playlist, **F6** Channel Rack, **F7** Piano roll, **F8** Plugin Picker, **F9** Mixer, **Alt+F8** Browser, **Espaço** play/stop, **Ctrl+Espaço** play/pause e **L** PAT/SONG. Os atalhos dependem do contexto, e atalhos de uma tecla podem mudar quando “typing keyboard to piano keyboard” está ligado. [Toolbar](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/toolbar_panels.htm), [atalhos](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_shortcuts.htm), [interface](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_interface.htm).

**[Proposta]** Tratar transporte, seleção, foco, zoom, scroll, estado de painéis e histórico de edição como domínios distintos. Um `CommandRouter` deve resolver teclas de acordo com janela ativa, foco em campo de texto e modo de teclado musical. O transporte gera posição musical comum para Channel Rack, Playlist, Piano roll, Mixer e áudio.

**[Pendente]** A documentação não fixa, em pixels, largura de painéis, fontes, geometria de menus, ícones, paleta de cada tema, tolerância de bordas para drag, z-order de janelas nem ordem exata de foco. Capturar essas medidas em um executável e tema definidos, em DPI e resolução fixos. Não inferir medidas de screenshots redimensionadas do manual.

## 3. Channel Rack e Step Sequencer

**[Fato]** Cada linha representa um canal. A linha pode mostrar mute, pan, volume, destino do Mixer, botão que abre o instrumento, seleção/atividade e passos ou miniatura do Piano roll. O seletor de pattern troca os dados visíveis sem duplicar os instrumentos. Passos são, normalmente, semicolcheias; clique esquerdo ativa e direito desativa. O valor inicial descrito pelo manual é 16 passos, com configuração por projeto. O *graph editor* (`Ctrl+K`) edita propriedades por passo, incluindo nota, velocity, release, fine pitch, Mod X/Y e deslocamento. Há swing global, grupos, compactação (“zip”), clonagem e reordenação de canais. [Channel Rack](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm), [Project Settings](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/songsettings_settings.htm).

**[Implicação]** Um passo não é apenas um booleano. É preciso conservar evento e propriedades quando o estado visual muda, obedecer a preferência que preserva valores de passos removidos e converter de modo coerente entre passo e nota do Piano roll. O manual diz que passos do Step Sequencer não têm *note off* próprio; notas sustentadas exigem Piano roll. [Channel Rack](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm).

**Contrato de observação inicial:** criar dois canais, alternar entre dois patterns, ativar/desativar passos, definir velocity por passo, mudar swing, abrir o Piano roll do canal, copiar passos entre patterns, rotear um canal ao Mixer e verificar som/seleção/representação. Registrar menus e atalhos no estado resultante, não só a imagem inicial.

## 4. Piano roll

**[Fato]** Altura codifica pitch e posição/largura codificam tempo e duração. Há desenho, seleção, movimento, redimensionamento, clonagem, pintura, quantização, zoom e scroll. O snap local afeta desenho, deslocamento, redimensionamento e cortes; `Alt` desativa temporariamente o snap durante o arraste. O modo “Line” muda a resolução aparente com o zoom. A área inferior edita propriedades de nota e eventos, como velocity, pan, release e pitch. O alvo do editor e o canal de destino são selecionáveis. O botão do meio com drag navega horizontal e verticalmente. Notas de *slide* têm semântica própria. [Piano roll](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/pianoroll.htm), [menu do Piano roll](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/pianoroll_menu.htm).

**[Proposta]** Uma transformação única entre `(tick, pitch)` e coordenadas de tela deve reger desenho, hit test, seleção, scroll e zoom. Armazenar tempo em inteiros de resolução musical configurável, sem converter continuamente para `f32` em operações de edição. Notas precisam de IDs, início, duração, pitch, velocity e atributos adicionais; ferramentas devem produzir comandos reversíveis com seleção e propriedades preservadas. O agendamento de áudio/MIDI converte ticks para amostras a partir do mapa de tempo.

**[Pendente]** Observar os detalhes de hit test no início/fim da nota, prioridade de clipes sobrepostos, comportamento ao soltar notas fora da viewport, scroll sob cursor, regras de seleção multicolorida e compatibilidade de *slide notes* por plugin. O manual descreve várias regras, mas não substitui testes no aplicativo de referência.

## 5. Playlist

**[Fato]** A Playlist sequencia Pattern Clips, Audio Clips e Automation Clips em faixas multiuso. Suporta sobreposição, cabeçalhos com mute/solo, grupos, marcadores, ferramentas de edição, snap local, Picker Panel e diferentes modos de vínculo entre canal/faixa/Mixer. Clipes contêm origem e instância: mover ou duplicar um Pattern Clip não cria necessariamente outro pattern; “Make unique”/clonagem têm semântica específica. Double click em Pattern Clip pode abrir Channel Rack, Piano roll ou Event Editor conforme conteúdo e configuração. O painel Picker gerencia e arrasta clipes do projeto. [Playlist](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm), [Pattern Clips](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist_patterns.htm).

**[Proposta]** Separar `ClipSource` e `ClipInstance`, com IDs próprios. Track Mode deve ser um vínculo opcional, não mudar a possibilidade de arranjos livres sem confirmação documental. Edição de clipes e edição da fonte exigem comandos de Undo/Redo distintos. Uma faixa visualmente silenciada não deve alterar o estado do canal ou Mixer sem regra explícita.

**Sequências para teste de equivalência:** soltar o mesmo pattern em duas faixas; sobrepor um Audio Clip e um Automation Clip; selecionar prioridade por foco; arrastar amostra do Browser ao cabeçalho e ao corpo da Playlist; redimensionar clipe; clonar versus “Make unique”; alternar PAT/SONG e observar reprodução. [Playlist](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm).

## 6. Mixer e caminho de sinal

**[Fato]** O áudio passa pelo Mixer. Canais novos podem ir ao Master por padrão ou ser associados a *insert tracks*. A documentação atual informa 500 inserts, uma faixa “Current” e Master. Cada insert tem roteamento para destinos, sends/sidechains, mute/solo, pan, fader, medidores, entrada/saída, efeitos em slots e compensação de atraso de plugins (PDC). Cada faixa tem dez slots de efeito; a ordem dos slots afeta o sinal. O fader fica após os efeitos. O Mixer oferece vários layouts e docks, com visibilidade dos controles dependente do tamanho da janela. [Mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm).

**[Proposta]** Representar o Mixer como grafo dirigido de busses com canais e efeitos ordenados. O grafo precisa validar realimentação indesejada, manter conexões send e sidechain separadas, somar compensações de latência e permitir recomputação fora da thread de áudio. A thread de áudio recebe snapshots estáveis ou mudanças de parâmetros sem bloqueios. Visualização de faders e medidores consome telemetria do áudio sem controlar o relógio do áudio. Isso é arquitetura recomendada, não engenharia reversa da implementação interna do FL Studio.

## 7. Browser, plugins e janelas de instrumentos

**[Fato]** O Browser dá acesso a projetos, amostras, conteúdo local/on-line e plugins. As abas padrão incluem All, Current Project, Plugin Database, conteúdo on-line e favoritos; a árvore e abas são configuráveis. Arrastar conteúdo para Channel Rack, Playlist ou Mixer produz resultados dependentes do destino. O Plugin Manager varre formatos **AU, CLAP e VST**; o manual documenta VST3 e a distinção entre geradores e efeitos. O Wrapper hospeda plugins nativos e de terceiros e expõe opções de processamento, UI, estado, MIDI e *bridging*. [Browser](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/browser.htm), [instalação de plugins](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_externalplugins.htm), [Wrapper](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/wrapper.htm).

**[Proposta]** Começar com Browser de recursos locais e instrumentos próprios simples. Depois acrescentar scanner, catálogo de metadados e hospedeiro de plugins em etapas. Um plugin de terceiros exige isolamento de falhas, estado serializável, MIDI/eventos, automação, latência, editor e redimensionamento; um botão “carregar VST3” isolado não alcança equivalência funcional.

**[Pendente]** O conteúdo on-line e a organização visual da aba Sounds mudam no serviço da Image-Line; a própria documentação adverte que conteúdo, layout e design podem mudar. Não congelar o Browser on-line como alvo de fidelidade sem versão, conta e estado capturados. [Browser/Sounds](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/browser_content.htm).

## 8. Áudio, MIDI, automação e persistência

**Áudio — [Fato].** As configurações de áudio expõem dispositivo, sample rate, buffer, contador de underruns e processamento multithread de geradores e Mixer. Buffer menor reduz latência de monitoramento, mas aumenta risco de underrun. Em Windows, o manual aborda ASIO; em macOS, Core Audio. “Smart disable” e PDC fazem parte da experiência. Isso especifica comportamento observável, não o algoritmo interno de agendamento da Image-Line. [Audio Settings](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_audio.htm), [Mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm).

**MIDI — [Fato].** A tela MIDI configura entradas e saídas, habilitação, portas, controladores, clock e gravação. MIDI carrega controle, não áudio; notas e movimentos de controle podem gravar no projeto. O manual registra particularidades de sincronização externa. [MIDI Settings](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_midi.htm).

**Automação — [Fato].** Automation Clips são geradores internos que controlam parâmetros vinculados; não se limitam a um pattern e aparecem como curvas na Playlist. Eventos de automação podem estar associados ao pattern. Controles nativos podem expor “Create automation clip” por menu de contexto. A distinção entre evento de pattern, Automation Clip e gravação de movimento precisa aparecer no modelo de dados. [Automation Clips](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist_automationclip.htm), [gravação de automação](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/recording_automation.htm).

**Projeto — [Fato].** `.flp` é o formato de projeto nativo e não incorpora automaticamente todas as amostras externas. O projeto zipado contém `.flp` e amostras/presets utilizados, com limitações para conteúdo interno de plugins de terceiros. A Image-Line documenta backups e uma pasta de dados do projeto. [Project File](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/fformats_project.htm), [Project Settings](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/songsettings_settings.htm).

**[Proposta]** Criar formato próprio versionado, com manifesto, recursos referenciados ou empacotados, migração e recuperação. Suporte `.flp` só após estudo independente e testes de interoperabilidade autorizados; não prometer compatibilidade com um formato não documentado aqui.

## 9. Matriz de fidelidade e evidência a coletar

| Fluxo | Referência documental | Observação ainda necessária | Critério de comparação |
|---|---|---|---|
| PAT/SONG, transporte, tempo, snap | [Toolbar](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/toolbar_panels.htm) | atraso de UI, posição do playhead, estados de hover | mesmo estado e mesmo evento musical após sequência de cliques/teclas |
| Canal e pattern | [Channel Rack](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm) | geometria, cores, gestos por tema | persistência de passos/propriedades por pattern |
| Nota e propriedades | [Piano roll](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/pianoroll.htm) | hit test, zoom sob cursor, seleção | notas/ticks/velocity idênticos após ações equivalentes |
| Arranjo e Track Mode | [Playlist](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm) | prioridade de sobreposição e vínculo criado por drop | fontes e instâncias corretas, áudio roteado corretamente |
| Mixer | [Mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm) | níveis, curvas de fader, PDC, tamanho de painéis | roteamento, medição, saída e latência dentro de tolerância definida |
| Plugin | [Wrapper](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/wrapper.htm) | scan, falha, abertura de editor, restauração | carregar, processar, automatizar e recuperar estado |
| Salvar/reabrir | [formatos](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/fformats_project.htm) | mensagens de recursos ausentes e ordem de restauração | round trip sem perda no formato próprio |

**Próximo passo da pesquisa de referência:** fixar SO, versão, resolução, escala de DPI, tema, idioma, tamanho de janela e projeto de teste. Para cada fluxo, guardar um registro de estado inicial, eventos com coordenadas/teclas e tempos, screenshots de estados importantes, valores observáveis e estado final. A comparação visual deve usar capturas próprias e análise de diferenças com máscaras para cursores, animações e medidores. Isso é metodologia proposta; o manual não publica um corpus de testes pixel a pixel.

## 10. Limites da fonte e abordagem independente

O manual confirma comportamento público e nomes de funções; não publica engine, classes internas, esquema completo de `.flp`, medidas 1:1 de UI nem implementação de plugins. Uma reimplementação limpa deve registrar comportamentos observáveis, implementar código e recursos próprios e manter a origem de cada observação. Interoperabilidade com arquivos e plugins deve ser avaliada separadamente. Engenharia reversa de binários, extração de ativos, contorno de proteção e acesso a código proprietário não fazem parte desta pesquisa.
