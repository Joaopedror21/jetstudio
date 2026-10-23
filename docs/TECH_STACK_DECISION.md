# Decisão de stack para uma DAW independente de alta fidelidade

> **Nota de revisão (Claude, 23/10/2026):** documento histórico. A decisão vinculante está em [FINAL_ARCHITECTURE.md](FINAL_ARCHITECTURE.md) (ADR-001: egui/eframe 0.36.2 como plataforma, com widgets 100% próprios, em vez de winit+wgpu com framework próprio). Ver [CLAUDE_REVIEW.md](CLAUDE_REVIEW.md) D1 e D5.

**Status:** recomendação técnica de pesquisa, sem aplicação implementada. **Data:** 23/10/2026. **Alvo inicial proposto:** desktop Windows x64; a arquitetura conserva portas para macOS e Linux. Esta escolha de plataforma é uma **proposta**, pois nenhuma versão/ambiente específico do FL Studio foi fixado ainda. As conclusões abaixo usam documentação dos projetos e do [manual oficial do FL Studio](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/), não o código interno da Image-Line.

## Decisão resumida

| Camada | Escolha proposta | Estado da evidência e motivo |
|---|---|---|
| Modelo, comandos, sequenciador e engine | **Rust**, crates separadas da UI | Decisão arquitetural nossa: estados explícitos, testes sem janela, controle de alocação no áudio. Rust não garante, por si, tempo real. |
| Janela/input e canvas final | **winit + wgpu**, desenho 2D próprio | [winit](https://github.com/rust-windowing/winit) entrega janelas/eventos, [wgpu](https://github.com/gfx-rs/wgpu) entrega API gráfica portátil. Precisaremos construir layout, texto, hit testing e widgets. |
| Ferramentas internas e telas não críticas | **egui** em integração isolada, se útil | [egui](https://github.com/emilk/egui/blob/main/README.md) facilita desenvolvimento imediato e oferece pintura 2D; seus widgets/layouts padrão não comprovam paridade visual 1:1. |
| I/O de áudio | **CPAL** inicialmente; backend especializado após medidas | [CPAL](https://github.com/RustAudio/cpal) enumera dispositivos/configurações e abre streams de entrada/saída. No Windows, README lista WASAPI por padrão e ASIO opcional. Não é engine DAW. |
| Passagem UI ↔ áudio | Mensagens pré-alocadas em SPSC, candidato **rtrb** | [rtrb](https://github.com/mgeier/rtrb) se apresenta como ring buffer SPSC seguro para tempo real. Requer política de overflow e múltiplos produtores. |
| Dispositivos MIDI | **midir** + parser/roteador próprio | [midir](https://github.com/Boddlnagg/midir) trata I/O MIDI multiplataforma; README declara que não fornece API de alto nível para analisar/montar mensagens. |
| Leitura de amostras | **Symphonia** fora da thread de áudio | [Symphonia](https://github.com/Samadaeus/symphonia) cobre demux/decode de formatos populares. Streaming, cache, resampling e arquivos ausentes são responsabilidade nossa. |
| Plugins | Interface de host própria; **CLAP/VST3** em fases posteriores | FL Studio aceita [AU, CLAP e VST](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_externalplugins.htm). [CLAP](https://github.com/free-audio/clap) e [SDK VST3](https://github.com/steinbergmedia/vst3sdk) publicam APIs. Hospedar plugins requer muito mais que carregar biblioteca. |
| Formato de projeto | Formato próprio versionado | `.flp` não é necessário para demonstrar comportamento central; [manual](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/fformats_project.htm) registra `.flp` e `.zip`, mas não fornece esquema completo. |

## 1. Por que Rust e como modularizar

**[Proposta]** Rust é viável para modelo de projeto, edição, grafo de áudio, DSP, agendamento e interface. Tipos e ownership ajudam a explicitar limites entre thread de UI, workers e callback de áudio. Isso não elimina latência de OS, page faults, plugins instáveis, prioridade de threads nem erros de protocolo. A condição de aceite deve ser medição de latência, underrun e equivalência, não a linguagem.

Workspace proposto (nomes ilustrativos, ainda não criados):

```text
crates/
  domain/          IDs, notas, patterns, clipes, parâmetros, tempo musical
  commands/        comandos reversíveis, transações, histórico e serialização
  project_io/      formato próprio, migrações, empacotamento de recursos
  audio_graph/     roteamento, buffers, DSP, PDC, agendamento
  audio_backend/   CPAL e backend de dispositivo
  midi_io/         dispositivos, parsing, mapeamento, clock
  plugin_host/     abstração de plugin, scanners e adaptadores CLAP/VST3
  ui_model/        seleção, foco, janelas, scroll, zoom, estado efêmero
  ui_render/       cena 2D, texto, widgets próprios, hit testing
  app/             composição, comandos globais, autosave e diagnóstico
```

**[Proposta]** `domain` não depende de windowing ou áudio; `audio_graph` lê um snapshot imutável/compacto derivado do projeto; a UI altera projeto por comandos. Os estados efêmeros de UI ficam fora do arquivo de projeto salvo, exceto posições de janelas que decidirmos persistir após observar a referência. O grafo de áudio é atualizado em pontos definidos entre blocos; nenhuma chamada da UI deve bloquear o callback.

Essa composição decorre do comportamento público: [Channel Rack](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm), [Playlist](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm) e [Mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm) mantêm entidades distintas e ligações opcionais. Não se está alegando que o FL Studio usa essa organização interna.

## 2. Interface: egui ou desenho próprio?

O [README de egui](https://github.com/emilk/egui/blob/main/README.md) descreve uma GUI de modo imediato, integração com `egui-winit` e `egui-wgpu`, além de `epaint` para desenho 2D customizado. O próprio projeto observa que scroll areas com muito conteúdo podem exigir virtualização para não refazer layout excessivo a cada frame. [winit](https://github.com/rust-windowing/winit/blob/master/FEATURES.md) deliberadamente não desenha na janela nem oferece menus nativos; [wgpu](https://github.com/gfx-rs/wgpu) expõe GPU, não widgets.

**[Decisão proposta]** Para o produto cuja exigência é interface 1:1, controlar geometria, tipografia, ordem das camadas, menus, janelas flutuantes, viewport e gestos em uma camada própria sobre `winit + wgpu`. Usar egui apenas onde os widgets padrão não aparecem no artefato final, ou reavaliar se um experimento comparativo demonstrar que pintura customizada via egui alcança as mesmas metas. Não presumir que “egui” inviabiliza fidelidade; ele oferece desenho próprio. O ponto decisivo é quanto layout e interação específicos terão de ser implementados de qualquer maneira.

**Experimento de decisão antes de fixar renderer:** medir um frame com Channel Rack, Piano roll e Playlist em resolução e DPI alvo, com milhares de notas e clipes visíveis. Comparar custo de layout, texto, scroll, hover e drag; captura de pixels; consistência entre GPUs e escalas. Um protótipo técnico de infraestrutura só deve ocorrer na etapa de implementação, não nesta fase documental. Os tokens de UI (larguras, alturas, cores, fontes, raios, sombras) serão extraídos de screenshots próprias da versão alvo, nunca copiados de recursos do programa.

## 3. Áudio em tempo real

[CPAL](https://github.com/RustAudio/cpal) é uma biblioteca de I/O de baixo nível; não fornece sequenciador, instrumentos, mixer, compensação de atraso nem formatos de projeto. O [manual de áudio do FL Studio](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_audio.htm) deixa claro que latência de buffer e underruns fazem parte do comportamento perceptível, e que há processamento multithread de geradores e efeitos. O [Mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm) exige efeitos ordenados, sends, sidechain, Master e PDC.

**[Proposta]** Um callback processa blocos pré-alocados, consome eventos com timestamp amostral, executa o grafo, aplica ganhos/pan e escreve saída. Edição, decoding, leitura de disco, scanner de plugins, serialização, locks e logs formatados ficam fora da thread de áudio. Mudanças estruturais no grafo são preparadas por worker e publicadas como snapshot; mudanças pequenas de parâmetros usam filas limitadas. `rtrb` é candidato para ligações SPSC, não solução universal de concorrência; várias fontes de eventos precisam de agregação explícita. [rtrb](https://github.com/mgeier/rtrb).

**Medições de aceite propostas:** duração de callback versus prazo do dispositivo, taxa de underrun com projetos de referência, latência de nota até saída, precisão de posicionamento de eventos e estabilidade ao mudar dispositivos/sample rate/buffer. Testes off-line devem renderizar a mesma sequência musical com saída determinística quando não há plugin externo. A margem exata precisa ser definida por ambiente, dispositivo e benchmark, não presumida do manual.

**Risco Windows:** CPAL lista WASAPI como padrão e ASIO como backend opcional. O manual usa ASIO extensamente para áudio profissional. Avaliar ASIO e requisitos de distribuição numa fase de backend específica, com a versão atual das fontes oficiais; não assumir paridade de driver só porque CPAL abre um stream. [CPAL](https://github.com/RustAudio/cpal), [Audio Settings](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_audio.htm).

## 4. MIDI e relógios

O [manual MIDI](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_midi.htm) cobre dispositivos de entrada e saída, portas, gravação, controladores e sincronização externa. [midir](https://github.com/Boddlnagg/midir) fornece a ponte com backends de OS, mas não regras de Channel Rack, gravação, mapeamento de CC ou quantização.

**[Proposta]** Separar quatro relógios: dispositivo MIDI (timestamp de entrada quando disponível), transporte musical (ticks), tempo de áudio (frames) e apresentação de UI. O agendador converte a posição musical em offsets dentro do bloco, respeitando mapa de tempo, loop, swing e mudanças de tempo. Mensagens MIDI entram numa fila limitada, são decodificadas fora do callback quando possível e se tornam eventos normalizados para canais e plugins. Há regras especiais para note-off/panic, sustain e troca de dispositivo; precisam de testes funcionais próprios. Sincronização externa e MIDI 2.0 só devem entrar quando o núcleo MIDI 1.0 estiver estável.

## 5. Plugins: CLAP, VST3 e limites

O [manual de instalação](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_externalplugins.htm) registra AU, CLAP e VST, e o [Wrapper](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/wrapper.htm) trata integração e *bridging*. Para um host, a [documentação VST3 da Steinberg](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/API%2BDocumentation/Index.html) exige buffers e blocos de processamento variáveis, eventos, parâmetros, componente/controlador e estado. A documentação de [automação VST3](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Parameters%2BAutomation/Index.html) distingue thread de GUI e processamento, com filas de pontos por parâmetro e offsets amostrais. A [FAQ de hosting](https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Hosting.html) informa que o host deve lidar com plugins sem editor próprio e possivelmente sem controlador separado. Isso torna VST3 um programa de trabalho próprio.

**[Decisão proposta]** Definir primeiro uma interface interna de instrumento/efeito (`prepare`, `process`, `reset`, `state`, `parameters`, `latency`, `editor`) e validar com plugins internos. Implementar hospedagem CLAP ou VST3 em marcos separados, com scanner fora do processo de áudio, banco de metadados, restauração de estado, ciclo de vida, editor e testes com plugins de terceiros autorizados. `NIH-plug` não é uma biblioteca de host: seu [README](https://github.com/robbert-vdh/nih-plug) o descreve como framework para **criar** plugins, hoje em modo de manutenção. Não o selecionar como atalho para hospedar VST3.

O [SDK VST3 oficial](https://github.com/steinbergmedia/vst3sdk) declara licença MIT no README consultado; conferir o commit e termos vigentes antes da integração e distribuição. O [CLAP oficial](https://github.com/free-audio/clap) define uma ABI de host/plugin. Essa documentação técnica não substitui revisão de distribuição e marcas quando o produto avançar.

## 6. Arquivos, estado e testes

[Symphonia](https://github.com/Samadaeus/symphonia) é opção para decodificar amostras no carregamento/streaming. Persistir projetos em formato próprio versionado, com referências relativas, hashes de recursos, migrações e autosave transacional. A documentação oficial esclarece que `.flp` normal pode referenciar amostras externas e que `.zip` incorpora parte desses recursos; isso dá um comportamento a reproduzir no produto sem exigir o mesmo formato binário. [Project File](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/fformats_project.htm).

**Testes propostos:**

1. Modelo/comandos: invariantes de IDs e roteamento, Undo/Redo, serialização → reabertura → igualdade semântica.
2. Sequenciamento: notas/steps/clipes/automação em posições amostrais e limites de loop, independente de UI.
3. DSP/áudio: render off-line determinístico, soma de busses, ordem de efeitos, PDC, ausência de alocação ou locks no callback sob teste instrumentado.
4. MIDI: captura e reprodução de sequências com timestamps, note-off, mudança de canal e desconexão.
5. Visual: capturas em mesmo tamanho/DPI/tema com máscaras de cursores e elementos animados; comparação de geometria, texto e estados, além de percursos de interação.
6. Plugins: matriz por formato/OS/plugin para scan, abrir, processar, automatizar, salvar/reabrir, falhar e recuperar.

Os testes visuais e a medição acústica devem usar observações de referência obtidas legalmente. Nenhuma das bibliotecas acima, isoladamente, garante equivalência com FL Studio. A recomendação de stack permanece condicionada aos experimentos de UI, latência e hospedagem descritos aqui.
