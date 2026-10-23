# Roadmap para uma reimplementação independente do FL Studio

> **Nota de revisão (Claude, 23/10/2026):** a ordem de marcos vinculante está em [IMPLEMENTATION_ROADMAP.md](IMPLEMENTATION_ROADMAP.md): o M1 passa a ser o shell de interface sem áudio, o áudio vira o M2, e o M0 corre em paralelo como trilha R0. As portas de saída por área deste documento continuam válidas.

**Data da proposta:** 23/10/2026. **Estado:** planejamento; nenhuma aplicação foi implementada nesta etapa. Este roteiro transforma as evidências de [FL_STUDIO_RESEARCH.md](FL_STUDIO_RESEARCH.md), [ARTCRAFT_RESEARCH.md](ARTCRAFT_RESEARCH.md), [ARTCRAFT_ARCHITECTURE.md](ARTCRAFT_ARCHITECTURE.md), [UI_REPLICATION_METHOD.md](UI_REPLICATION_METHOD.md) e [TECH_STACK_DECISION.md](TECH_STACK_DECISION.md) em marcos verificáveis. Cada arquitetura ou prazo abaixo é **proposta nossa**, não descrição do código interno do FL Studio nem previsão calibrada em horas.

## Norte técnico e critérios de paridade

O [manual do Channel Rack](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm), [Playlist](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm) e [Mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm) estabelece três distinções fundamentais: (1) um pattern contém eventos para canais compartilhados, (2) a Playlist aceita diferentes tipos de clipe em faixas multiuso, (3) canal, faixa de Playlist e insert do Mixer só se vinculam em certos fluxos, como Track Mode. A arquitetura deve representar isso desde o primeiro marco. Transporte [PAT/SONG](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/toolbar_panels.htm), automação, plugins e áudio ampliam o mesmo modelo; não devem forçar uma reescrita posterior.

**Fidelidade é medida por dimensões independentes**, sem somá-las numa porcentagem impressionante porém vazia:

| Dimensão | Medida proposta | Evidência de conclusão |
|---|---|---|
| Comportamento | Casos de uso reproduzidos a partir de estado inicial fixo, com mesmo estado final e undo/redo. | Matriz de observações e testes executáveis por caso. |
| Visual | Geometria, tipografia, cores, estados e resposta à janela/DPI, por região e cenário. | Capturas pareadas, mapas de diferença, revisão humana e baseline da própria UI. |
| Musical | Mesmos eventos por pattern, canal e posição musical, inclusive loop, swing e snap. | Inspeção de eventos normalizados e render offline com tolerância documentada. |
| Áudio | Roteamento, ordem de efeitos, latência de eventos e estabilidade em playback. | Testes offline determinísticos, medições de callback, underruns e loopback por hardware. |
| Arquivos | Salvar, fechar, reabrir e preservar o significado do projeto e suas referências. | Corpus de projetos próprios versionados e testes de migração/recuperação. |
| Integração | Browser, plugins, MIDI e fluxo entre janelas em tarefas completas. | Cenários de ponta a ponta, não apenas contagem de botões/menus. |

Esse cuidado deriva da diferença explícita entre cobertura de menus e uso real no [PhotoCraft target-app-parity.md](https://github.com/storytold/photocraft/blob/main/docs/target-app-parity.md) e das lacunas de harness registradas pelo [SoundCraft](https://github.com/storytold/soundcraft/blob/main/docs/gaps.md). Os números desses projetos não se transferem como previsão para a nossa DAW.

## M0 — Base de evidência da referência (ainda sem implementação)

**Primeira ação após esta pesquisa.** Obter acesso legítimo a uma instalação de uma versão/build definida do FL Studio. Fixar inicialmente Windows x64, resolução, escalas de DPI, tema, idioma e dispositivo de áudio de teste; registrar variações por versão. Usar o manual como especificação inicial e executar um conjunto pequeno de observações *black box* sem extrair assets ou inspecionar binários. A [metodologia de UI](UI_REPLICATION_METHOD.md) detalha captura e medidas.

**Entregáveis:** inventário de 20–30 fluxos centrais com passos, estados e capturas internas; árvore de menus e atalhos por contexto; primeiros tokens/retângulos; 5 projetos de referência criados pela equipe com conteúdo original; decisões documentadas para semântica de PAT/SONG, pattern, Step Sequencer, Playlist, Mixer e Save/Open. Cada achado recebe nível de confiança: manual, observação repetida, hipótese. Não publicar imagens ou recursos do software de referência como assets do produto.

**Porta de saída:** nenhuma regra central do primeiro marco de implementação depende apenas de suposição. Pendências ficam enumeradas com experimento para resolvê-las. A instalação/versão de referência ainda não foi informada nesta pesquisa; logo a paridade visual em pixels não pode ser fixada honestamente agora.

## M1 — Primeira etapa de implementação: fatia vertical musical

**Objetivo:** um projeto próprio pequeno, salvo/reaberto, cujo *Channel Rack*, transporte e colocação mínima de um Pattern Clip se comportem de modo mensurável como a referência no subconjunto escolhido. Esse é o **primeiro marco de código recomendado**, após M0. O produto nesta fase pode ter apenas as superfícies necessárias ao fluxo; não será anunciado como réplica do FL Studio.

**Arquitetura que deve nascer aqui:**

```text
modelo de projeto (ChannelId, PatternId, PlaylistTrackId, ClipId,
                   Step, Tempo, TimeSignature)
       ↓  comando validado + histórico reversível
sequenciador em tempo musical inteiro → eventos com posição amostral
       ↓
instrumento/sampler próprio → mix estéreo simples → render offline
       ↓                                      ↓
transporte e Channel Rack               saída de dispositivo
       ↓
formato próprio versionado + CLI de inspeção/teste
```

**Escopo:** criar/renomear/reordenar canais; criar e alternar dois patterns; ligar/desligar passos; editar velocity; definir BPM e tamanho de pattern; criar uma faixa de Playlist e inserir nela uma instância simples de Pattern Clip; selecionar PAT/SONG e iniciar/parar; roteamento inicial ao Master; reprodução de amostras originais ou livres com atribuição; salvar/abrir e undo/redo. M1 precisa só da edição mínima dessa instância; a Playlist completa e os outros tipos de clipe pertencem a M3. Um canal/instrumento simples e a saída de áudio bastam, desde que o fluxo completo seja real e verificável. Começar com mix estéreo e arquivo de projeto próprio. Qualquer menção a `.flp` deve ser tratada como futuro estudo de interoperabilidade, pois o [manual de projetos](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/fformats_project.htm) não fornece esquema binário completo.

**Portas de saída:**

1. Alternar patterns preserva eventos por canal; PAT toca o selecionado e SONG toca a instância na Playlist no tempo colocado; repetir os mesmos passos produz os mesmos eventos/timestamps offline; desfazer/refazer edição restaura a sequência sem alterar outras entidades.
2. Um projeto com dois patterns e vários canais sobrevive a salvar/fechar/reabrir com igualdade semântica de modelo e recursos disponíveis; recurso ausente produz erro legível e não corrompe o documento.
3. O callback de áudio não executa I/O de disco, não espera locks da UI e não faz alocações não controladas no caminho normal; instrumentar e medir prazo de callback/underruns em hardware de teste. Rust/CPAL não garantem isso sozinhos ([CPAL](https://github.com/RustAudio/cpal), [manual de áudio](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_audio.htm)).
4. Capturas de Channel Rack e transporte são comparadas ao alvo do M0 nas resoluções/escalas selecionadas; todos os controles implementados têm teste de hover, foco, clique e atalho. Registrar cada diferença remanescente como parcial ou ausente.
5. O mesmo comando de edição é acionável por UI e por uma API local/CLI de testes. Não é necessário criar um servidor MCP completo para cumprir M1; preservar IDs/contratos para adicioná-lo sem duplicar lógica.

**Dependências técnicas:** escolher renderer de UI após um experimento mensurável entre pintura customizada com egui e `winit + wgpu` ([decisão de stack](TECH_STACK_DECISION.md)); criar crate de tempo, modelo, comandos, áudio e UI com dependências em camadas. Essa avaliação é parte de M1 e não constitui interface demonstrativa nesta etapa documental.

## M2 — Piano Roll e edição musical completa

**Escopo:** notas com pitch, início, duração e velocity; ferramentas de desenho, seleção, movimento, redimensionamento, clonagem; snap local e global; zoom/scroll centrado; painel de propriedades; conversão consistente de passos para notas quando o fluxo da referência o exigir; atalhos e regras de foco. O [manual do Piano Roll](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/pianoroll.htm) ancora funções; medidas de hitbox e modificadores vêm do M0.

**Porta de saída:** a sequência de ações sobre um mesmo pattern gera dados finais e MIDI/eventos equivalentes; gestos longos coalescem em um passo de undo; panorama/zoom não modificam o conteúdo; testes de notas no limite de clipe, de janela e de loop. A UI precisa permanecer responsiva com uma fixture grande, medida em máquina definida.

## M3 — Playlist, clipes e modos PAT/SONG

**Escopo:** faixas multiuso, Pattern Clips, Audio Clips, instâncias versus fontes, Picker Panel, mover/copiar/redimensionar, mute/solo, marcadores, loop e snap; depois Automation Clips básicos e Track Mode. A [Playlist oficial](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm) permite clipes de tipos diferentes sobrepostos na mesma faixa: o modelo não pode impor “uma faixa = um instrumento”.

**Porta de saída:** duas instâncias do mesmo pattern permanecem ligadas à fonte correta; Make Unique (quando implementado) altera uma fonte sem alterar a outra; PAT reproduz o pattern selecionado e SONG o arranjo; Audio Clip e Pattern Clip se sobrepõem sem perda de roteamento; exportação offline determinística do projeto pequeno.

## M4 — Mixer e engine de áudio profissional

**Escopo:** inserts, Master, sends/sidechains, efeitos internos, ganho/pan/faders, medidores, roteamento de canal, gravação de áudio, compensação de latência de plugins (PDC), bounce e configuração de dispositivo/buffer. O [Mixer](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm) define ordem de slots e conexões; [Audio Settings](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_audio.htm) define experiências de buffer/dispositivo a observar.

**Portas de saída:** grafo de sinal validado (incluindo prevenção de ciclos não suportados), efeitos na ordem correta, PDC por caminho testado com delays sintéticos, medidores lendo telemetria sem bloquear callback, bounce offline comparado ao playback para material determinístico. Medir underruns, latência, CPU e memória em projetos de várias faixas e taxas de amostragem. A meta quantitativa por hardware só deve ser fixada com baseline do M0/M1, sem prometer “latência zero”.

## M5 — Browser, MIDI de hardware e automação

**Escopo:** Browser local e Current Project, preview e drag/drop contextual; MIDI input/output e gravação, CC e mapeamento; curvas de automação, vínculos de parâmetros, gravação de movimentos e distinção entre eventos de pattern e Automation Clips. Base documental: [Browser](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/browser.htm), [MIDI Settings](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_midi.htm), [Automation Clips](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist_automationclip.htm).

**Portas de saída:** arrastar o mesmo recurso a destinos diferentes cria as entidades esperadas; desconectar MIDI não deixa notas presas; automação preserva valor e curva após salvar/reabrir, opera em reprodução e bounce com timestamps consistentes; o Browser falha claramente para arquivo ausente. Conteúdo on-line do serviço da Image-Line permanece fora da promessa de fidelidade até haver escopo autorizado e observações versionadas.

## M6 — Hospedagem de plugins em fases

**Escopo:** contrato interno de instrumento/efeito primeiro; scanner em processo separado; banco de plugins; depois CLAP e VST3, AU onde aplicável; eventos MIDI, automação com offset amostral, estado, latência, editor e recuperação de falhas. A existência de suporte a [VST/CLAP/AU no FL Studio](https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/basics_externalplugins.htm) é fato; a sequência de implementação é proposta. Consultar SDKs oficiais [VST3](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/API%2BDocumentation/Index.html) e [CLAP](https://github.com/free-audio/clap).

**Portas de saída:** matriz por plugin de teste/OS para descoberta, primeira carga, áudio/MIDI, parameter change, save/reopen, resize/editor, latência e falha. Um plugin defeituoso não deve derrubar o projeto sem recuperação. Só declarar “suporte VST3” para os requisitos que a matriz demonstrar; plug-in hosting não se resume a `dlopen`.

## M7 — Profundidade, interoperabilidade e validação de produto

**Escopo:** recursos avançados de Channel Rack, Piano Roll, Playlist e Mixer; project bundles, backups, migração, recuperação após crash, atalhos/painéis adicionais, instrumentos e efeitos próprios, desempenho de projetos grandes, acessibilidade e plataformas além do alvo inicial. Avaliar `.flp` por estudo independente de formato e testes autorizados, sem copiar código da Image-Line ou contornar proteção; manter o formato próprio utilizável mesmo se a interoperabilidade não for viável.

**Portas de saída:** um conjunto de tarefas criativas reais de ponta a ponta (construir beat, arranjar, automatizar, gravar MIDI/áudio, mixar, exportar, reabrir) tem evidência de resultado e UX; scorecard separa `implementado`, `parcial`, `ausente` e `testado contra referência`. O produto só recebe rótulo de alta paridade quando casos representativos, estabilidade, arquivos e desempenho sustentarem essa afirmação.

## Riscos que governam a ordem

| Risco | Por que exige atenção cedo | Ação/medida |
|---|---|---|
| Áudio em tempo real | O OS e plugins podem bloquear; callback tem prazo por bloco; UI e disco são assíncronos. | Render offline como referência, instrumentação de callback, testes de underrun em hardware real desde M1. |
| Semântica de patterns/Playlist | Um modelo rígido de faixas tornaria Track Mode, clipes sobrepostos e PAT/SONG difíceis de corrigir. | IDs e entidades separadas desde M1; cenários M0/M3. |
| Fidelidade visual | egui/wgpu permitem desenho customizado, mas nenhum cria sozinho widgets equivalentes ao FL. | Tokens/retângulos medidos, captura pareada, estados e gestos testados, experimento de renderer M1. |
| VST3/CLAP | Ciclo de vida, estado, editor, eventos, latência e falhas cruzam threads/processos. | Interface de host, scanner e matriz de plugins antes de promessa de compatibilidade. |
| MIDI e relógios | Ticks musicais, timestamps MIDI, frames de áudio e pixels da UI não são o mesmo relógio. | Conversões únicas e testes em limites de bloco, loop e mudança de tempo. |
| Arquivos de projeto | `.flp` não é especificado integralmente pelo manual; samples podem ficar externos. | Formato próprio versionado, round trip e mensagens de dependência ausente. |
| Paridade inflada | Itens de menu podem estar ativos mas com comportamento parcial. | Scorecard por fluxo, UX, visual, áudio, formato e plataforma; sem percentual único como prova. |

## Organização de execução

Codex e Claude Code podem trabalhar em crates e superfícies independentes com dono explícito, contrato de tarefa e validação cruzada ([fluxo de agentes](ARTCRAFT_AI_WORKFLOW.md)). Alterações compartilhadas de modelo de projeto, ids de comandos, grafo de áudio, formato e tokens de UI devem ser pequenas e integradas em sequência. Uma tarefa fecha apenas quando há teste do comportamento, comparação de referência quando aplicável, revisão de UI/áudio e documentação de lacunas. O [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/main/AGENTS.md) inspira registry, gates e documentação de status; a organização concreta aqui é uma proposta para o projeto FL Studio.
