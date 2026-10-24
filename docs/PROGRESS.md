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
- **UI** (`jet-ui`): Channel rack (knobs, seletor de mixer, mute/solo, pintura de steps, renomear, menu de contexto, arrastar samples), Piano roll (ferramentas, snap, mover/redimensionar, velocity, teclado, atalhos), Mixer (faders, pan, mute/solo, meters, sends, FX e parâmetros), toolbar com CPU, polifonia e osciloscópio reais, Export WAV.

## 3. Testes realizados e resultados reais

- `cargo xtask ci`: verde em 24/10/2026, depois do commit `710b912`.
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

1. **Playlist completa** (PL-005, PL-017): mover, redimensionar e apagar clips na UI; arrastar o playhead; seleção de timeline e loop; Ctrl+B.
2. **Controles que faltam no Channel rack:** knob de swing (CR-015), comprimento do pattern (CR-014), Graph editor visual (CR-016), copy/paste de steps.
3. **Sampler:** start offset, loop points, ping-pong (SMP-005); envelopes e LFO (SMP-008); polifonia, mono e porta (SMP-009).
4. **Automation clips** (AUT-001 / PL-004): modelo, curvas, aplicação por bloco aos parâmetros (volume, pan, fader, parâmetros de FX).
5. **Audio clips** (PL-003): clip de sample na Playlist com ganho e fades.
6. **Entrada MIDI** (MIDI-001) com `midir`, direto nas notas ao vivo já existentes; typing keyboard.
7. **Configurações de áudio** (GLB-021, AE-004): janela F10 com dispositivo, taxa e buffer.
8. **Autosave e backups** (FILE-003).
9. **Export MIDI** (AE-008) e **split mixer tracks** (AE-009).
10. **Hospedagem de plugins** (PLG-001/002) em crate FFI isolado; o VST3 está sob MIT desde a 3.8.

## 6. Dependências técnicas

- Automação depende de parâmetros endereçáveis e estáveis (ids de canal, faixa e slot já existem) e de interpolação por bloco no renderizador.
- Audio clips dependem de `ClipSource::Audio` no modelo, no sequenciador (eventos de clip) e de uma voz de sample com offset.
- MIDI depende de `midir` (cache pendente) e de uma fila de eventos com timestamp até a thread de áudio (mesma fila rtrb).
- Plugins dependem da ADR sobre crates FFI com `unsafe` isolado.

## 7. Próxima tarefa

**Janela de configurações de áudio (F10; GLB-021/AE-004).** Ainda não começou no código. Plano:
1. Em `jet-playback`: adicionar `output_devices()` e o campo `device: Option<String>` em `OutputRequest`, escolhendo o dispositivo pelo nome (hoje `crates/jet-playback/src/lib.rs` usa `default_output_device()`). Se a taxa pedida falhar (WASAPI shared), voltar à configuração padrão.
2. Em `jet-ui`: `WindowKind::Settings` (F10, OPTIONS) com lista de dispositivos, taxa, buffer (128 a 2048, mostrando a latência em ms) e status (callbacks, callbacks atrasados). Ao aplicar, reabrir o `AudioOutput`, passar o graveyard para o host novo, chamar `set_clock` e reabrir o MIDI. Guardar em `Documentos/JetStudio/settings.json`.

Depois disso: gravação de áudio (REC-001, entrada cpal → WAV → Audio Clip), controles de stereo sep/invert/swap no Mixer, arranjos múltiplos e marcadores (PL-008/009), Graph editor visual (CR-016), ferramentas do Piano roll (PR-017/018/019), hospedagem de plugins (PLG).

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
