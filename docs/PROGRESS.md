# Progresso do JetStudio

> Atualizado continuamente. Esta é a primeira coisa a ler ao retomar o trabalho noutra sessão.
> Inventário: [FL_STUDIO_COMPLETE_FEATURES.md](FL_STUDIO_COMPLETE_FEATURES.md) · Matriz: [FEATURE_PARITY_MATRIX.md](FEATURE_PARITY_MATRIX.md)

## Estado atual (24/10/2026, fim da sessão)

- **Branch:** `claude/M1-visual-fidelity` (criada de `codex/M1-interface`). Nada foi enviado ao remoto.
- **Portas:** `cargo xtask ci` passa (fmt, clippy `-D warnings`, todos os testes, camadas, assets).
- **App:** `cargo run --release --bin jetstudio` abre a janela com o dispositivo de áudio padrão.
  No hint bar aparecem o dispositivo, a taxa e o número de canais. Se não houver dispositivo, o transporte roda pelo relógio do sistema, em silêncio, e o hint bar diz por quê.
- **Áudio real verificado nesta máquina** com `jet-cli play`: dispositivo "Fones de ouvido (4- Arctis Nova 3)", 48 kHz, F32, buffer de 480 frames, 210 callbacks em ~2 s, 0 overruns, pico de carga DSP de 0,78%, sinal real medido no Master.

## Commits desta sessão

| Commit | Conteúdo |
|---|---|
| `1e3c802` | Camada visual medida contra a captura W (trabalho anterior, agora commitado) |
| `d811f0d` | Motor de áudio: `jet-seq`, `jet-dsp`, `jet-media`, `jet-audio-graph`, `jet-playback`; modelo estendido; comandos de canal, step, nota, mixer, projeto e exportação; testes de integração |
| `710b912` | UI: áudio real, janelas Piano roll e Mixer, Channel rack interativo, Browser com preview e arraste, export WAV, `jet-cli play` |
| `2c763c5` | Playlist: mover/redimensionar/apagar/mutar clips, solo de faixa, seek no ruler; rack: swing e comprimento do pattern |
| `9e97e44` | Automation Clips (modelo, 10 curvas, aplicação no áudio, criação pelo menu do knob, edição de pontos) |
| `e394e5a` | Sampler: start, loops, reverse, envelope, polifonia; janela Channel Settings |
| `1dc8d64` | MIDI de entrada (midir), teclado de digitação (Ctrl+T), metrônomo (Ctrl+M) |
| `6b5fb18` | Audio Clips (canal AudioClip, soltar arquivo na Playlist, waveform) |
| `49ae904` | MIDI files (export/import), clipboard de notas, autosave/revert |
| `e84a417` | Efeitos Flanger, Phaser e Filter |
| `57a08d6` | Configurações de áudio (F10): dispositivo, taxa e buffer, guardados em `%APPDATA%\JetStudio\settings.json` |
| `9c92c34` | Correção: dados do usuário fora do checkout (`Documents\JetStudio` é o próprio repositório no Windows) |
| `bce08a7` | Gravação de áudio: R arma, Play grava a entrada padrão, a take vira Audio Clip no modo Song |
| `e6d1a04`, `589aade` | Mixer: invert phase, swap L/R e stereo separation no inspector |
| `ef7d2e7` | Playlist: menu de arranjos e time markers |
| `89b0d67` | Piano roll: chord stamp, snap to scale, flip, strum, limit, arpeggiate |
| `fcbd326` | Graph editor (Ctrl+K): 8 abas por step, arrastar, Ctrl desloca, rampa com botão direito, Alt reseta |
| `f595cbb` | Porta, Mono legato e slide notes (glide em cents, exato por amostra) |
| `da845f4` | Editor de eventos do Piano roll: Velocity, Pan, Release, Fine pitch, Mod X/Y |
| `67e4ff4` | Gravação de notas (MIDI e teclado de digitação) no pattern selecionado, filtro de gravação |

## 1. Funcionalidades pesquisadas

Todas as seções do manual listadas em `FL_STUDIO_COMPLETE_FEATURES.md` §1: 191 itens em 11 sistemas, com 11 hipóteses registradas (H-xxx).
O corpus de texto do manual fica na scratchpad da sessão (`flman/*.txt`). Ele não é versionado, então baixe de novo quando precisar (as URLs estão no §1 do inventário).

## 2. Funcionalidades implementadas (com teste)

- **Sequenciador** (`jet-seq`): steps com swing, shift e pitch por step; notas do Piano roll substituem os steps do canal; comprimento Auto ou explícito; clips do modo Song como janela sobre o pattern; mute de canal, faixa e clip.
- **DSP** (`jet-dsp`): curvas de nível (canal −5,2 dB a 78%; Sampler padrão −8,2 dB; lei de pan circular e triangular); voz de Sampler (Hermite, declick de 10 ms); sintetizador próprio; ADSR; SVF; biquads RBJ; efeitos EQ, delay, reverb, compressor, limiter, distortion, chorus e gate.
- **Mídia** (`jet-media`): decodificação de wav/mp3/ogg/flac (Symphonia); escrita de WAV 16, 24 e 32 float.
- **Grafo** (`jet-audio-graph`): render exato por amostra, independente do tamanho do bloco; mixer com sends sem ciclos, Route to Master, FX com estado preservado entre planos; medidores; osciloscópio; cut groups; roubo de voz; notas ao vivo; preview; render offline (leave/cut/wrap) idêntico ao playback; sem alocação no callback.
- **Playback** (`jet-playback`): cpal 0.18.2, filas rtrb, relógio do dispositivo, plano e transporte aplicados juntos, estatísticas de carga e overruns.
- **Engine** (`jet-engine`): cerca de 80 comandos (lista: `jet-cli commands`); pool de samples fora do histórico e liberado só na thread de UI; undo de gestos contínuos; posição do transporte que volta ao início no fim do material.
- **UI** (`jet-ui`): Channel rack (knobs, seletor de mixer, mute/solo, pintura de steps, renomear, menu de contexto, arrastar samples, swing e comprimento do pattern), Piano roll (ferramentas, snap, mover/redimensionar, velocity, teclado, atalhos, clipboard, MIDI import/export, menu Tools, snap to scale, chord stamp), Mixer (faders, pan, mute/solo, meters, sends, FX e parâmetros, invert, swap, stereo sep), Playlist (clips de pattern, áudio e automação, arranjos, markers), Channel Settings, Audio Settings (F10), gravação, toolbar com CPU, polifonia e osciloscópio reais, Export WAV/MIDI.
- **Contagem atual da matriz:** 192 itens: 71 ✅, 57 🟡, 62 ⬜, 2 ⛔ (recontada pelo script depois de `67e4ff4`).

## 3. Testes realizados e resultados reais

- `cargo xtask ci`: verde em 24/10/2026, depois de cada commit até `67e4ff4` (o commit só é feito se o CI passar).
- Hardware: `jet-cli play` com Mono legato e slide note no dispositivo real: 0 overruns, pico de DSP de 2%. A gravação por MIDI real não foi exercitada (nenhum controlador foi usado).
- Testes novos por área:
  - jet-seq: 6;
  - jet-dsp: 20;
  - jet-media: 3;
  - jet-audio-graph: 13, mais 1 de não alocação (o detector foi conferido: um `vec!` colocado no callback faz o teste abortar);
  - jet-engine: 10 de integração de áudio e 1 de gestos;
  - jet-ui: 3 de interação nova (13 no total).
- Integrações pedidas, todas cobertas:
  - Channel Rack com o engine: `channel_rack_steps_render_at_the_step_grid`;
  - Piano Roll com os instrumentos: `piano_roll_notes_drive_the_synth_at_their_pitch`;
  - Playlist com o sequenciador: `playlist_song_mode_plays_clips_at_their_positions`;
  - Mixer com o áudio: `mixer_routing_faders_sends_and_effects_change_the_output`;
  - salvar e carregar: `save_and_load_keep_samples_and_render_identically`.
- Execução visual: o app em release abriu (1567×1029) e foi capturado pela janela real.
  Telas offscreen: `cargo run -p jet-ui --example shot --release -- out.png --demo --show piano,mixer`.

## 4. Arquivos novos ou modificados

- Crates novos: `crates/jet-seq`, `crates/jet-dsp`, `crates/jet-media`, `crates/jet-audio-graph`, `crates/jet-playback`.
- Modificados:
  - `crates/jet-model/src/lib.rs`;
  - `crates/jet-engine/src/{lib.rs, history.rs, samples.rs, transport.rs, cmd/*}`;
  - `crates/jet-engine/tests/{audio.rs, commands.rs}`;
  - `crates/jet-ui/src/{app.rs, channel_rack.rs, piano_roll.rs, mixer.rs, toolbar.rs, browser.rs, widgets.rs, workspace.rs, demo.rs}`;
  - `crates/jet-ui/examples/shot.rs`;
  - `apps/jet-cli/src/main.rs`;
  - `apps/jetstudio/src/main.rs`;
  - `xtask/src/main.rs` (camadas: `jet-media` em L2).
- Docs: este arquivo, `FL_STUDIO_COMPLETE_FEATURES.md`, `FEATURE_PARITY_MATRIX.md`.

## 5. Recursos pendentes (por prioridade, respeitando dependências)

Feitos nesta etapa: Graph editor (CR-016), Porta/Mono legato/slide notes (SMP-009, PR-015), editor de eventos (PR-012), gravação de notas (REC-002).

1. **Audio clips:** ganho, fades e stretch (PL-003).
2. **Tipos de marcador e loop da música** (PL-009); Merge de arranjos (PL-008).
3. **Efeitos que faltam:** stereo shaper (FX-010), pitch shifter (FX-012), analisadores.
4. **Hospedagem de plugins** (PLG-001/002) em crate FFI isolado; o VST3 está sob MIT desde a 3.8; CLAP é MIT.
5. **Outras abas de configurações** (GLB-021) e os demais modos de autosave (FILE-003).
6. **Ghost notes** (PR-016); countdown e Score logger na gravação (REC-002).

## 6. Dependências técnicas

- Automação depende de parâmetros endereçáveis e estáveis (ids de canal, faixa e slot já existem) e de interpolação por bloco no renderizador.
- Audio clips dependem de `ClipSource::Audio` no modelo, no sequenciador (eventos de clip) e de uma voz de sample com offset.
- MIDI depende de `midir` (cache pendente) e de uma fila de eventos com timestamp até a thread de áudio (mesma fila rtrb).
- Plugins dependem da ADR sobre crates FFI com `unsafe` isolado.

## 7. Próxima tarefa

**Audio clips: ganho e fades (PL-003).** Ainda não começou no código. Plano:
1. Releia `flman/playlist_audioclip.txt` (ou baixe `playlist_audioclip.htm`) para as propriedades do clip: volume, fade in/out, e o que o manual diz de stretch.
2. Modelo: campos com `serde(default)` no canal AudioClip (o clip de áudio é um canal: `Channel::audio_clip`); comando `channel.audio.set`.
3. Render: o `SamplerVoice` já tem envelope; fades lineares no início e no fim da clip (o fim é o `hard_end`). Teste de render com rampas medidas.
4. UI: menu da clip na Playlist ou página no Channel Settings do canal AudioClip.

Depois disso, siga a lista do §5 a partir do item 2.

## 8. Como continuar noutra sessão

1. `git checkout claude/M1-visual-fidelity && cargo xtask ci` deve passar.
2. Leia este arquivo e a matriz. O inventário diz, por ID, o comportamento verificado e onde está o código.
3. Antes de implementar um item, releia a página do manual citada no inventário. Se o manual não define o comportamento, registre uma hipótese H-xxx.
4. Para cada item: escreva o teste de comportamento (engine/graph), implemente, integre à UI e atualize o estado do item no inventário e a contagem da matriz (o script está descrito na matriz, §1).
5. Áudio real: `cargo build --release -p jet-cli` e depois `target/release/jet-cli play --cmd 'channel.add={"name":"Lead","kind":"synth"}' --cmd 'note.add={"pattern":1,"channel":1,"start":0,"length":96,"key":69}' --seconds 2` (usar o bash; o PowerShell 5.1 remove as aspas).
6. Telas: `cargo run -p jet-ui --example shot --release -- out.png --demo --show rack,piano,mixer`.

## 9. Capturas do FL que ajudariam a validar (pedido ao usuário)

1. Mixer aberto (F9) com o fader do Master arrastado até o topo: o hint mostra o valor em dB e confirmaria ou corrigiria a H-MIX-1.
2. Channel rack com o volume de um Sampler em 100%, 50% e 25%, lendo o valor em dB no hint bar (H-VOL-1).
3. Um pattern com swing em 100%, renderizado em WAV (H-SEQ-1).
4. Piano roll com notas (geometria e cores).
