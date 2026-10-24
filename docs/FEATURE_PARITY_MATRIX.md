# Matriz de paridade funcional: FL Studio 2026 × JetStudio

> **Atualizado em:** 24/10/2026. **Fonte dos itens:** [FL_STUDIO_COMPLETE_FEATURES.md](FL_STUDIO_COMPLETE_FEATURES.md), que tem os IDs, as referências e os testes.
> **Regra:** uma funcionalidade só conta como disponível quando o comportamento existe e tem teste. Botão desenhado não conta. Nenhum item é "Validado", porque nenhum comportamento foi ainda comparado lado a lado com o FL (veja os pedidos de captura em `PROGRESS.md`).

Legenda: ✅ disponível e testado · 🟡 parcial (o que falta está ao lado) · ⬜ não implementado · ⛔ bloqueado.

## 1. Resumo por sistema

| Sistema | Itens catalogados | ✅ | 🟡 | ⬜ | ⛔ |
|---|---:|---:|---:|---:|---:|
| Interface e controles globais (GLB) | 23 | 7 | 9 | 7 | 0 |
| Channel Rack e Step Sequencer (CR) | 33 | 14 | 9 | 9 | 1 |
| Sampler e instrumentos (SMP/INS) | 20 | 8 | 3 | 9 | 0 |
| Piano Roll (PR) | 27 | 9 | 12 | 6 | 0 |
| Playlist (PL) | 17 | 5 | 8 | 4 | 0 |
| Mixer (MX) | 17 | 8 | 5 | 4 | 0 |
| Motor de áudio (AE) | 15 | 9 | 2 | 4 | 0 |
| Efeitos (FX) | 14 | 9 | 1 | 4 | 0 |
| Plugins externos (PLG) | 4 | 0 | 0 | 4 | 0 |
| Browser e arquivos (BR/FILE) | 12 | 2 | 4 | 5 | 1 |
| Automação, MIDI, gravação (AUT/MIDI/REC) | 10 | 0 | 3 | 7 | 0 |
| **Total** | **192** | **71** | **56** | **63** | **2** |

As contagens foram geradas por script a partir da coluna Estado do inventário: ✅ = Testado; 🟡 = Implementado ou Parcialmente compatível; ⬜ = Pesquisado ou Especificado; ⛔ = Bloqueado. 

## 2. Matriz detalhada (itens P0 e P1)

| FL Studio | JetStudio | Estado | Evidência / o que falta |
|---|---|---|---|
| Play, Stop, Pause, PAT/SONG | transporte com relógio do dispositivo | ✅ | `pattern_mode_loops_the_pattern`, `stop_releases_synth_voices...`; falta o duplo clique de pânico e "Remember seek time" |
| Tempo 10–522 BPM | LCD editável por teclado | 🟡 | falta arrastar o LCD e o tap tempo |
| Undo/Redo (Ctrl+Z, Ctrl+Alt+Z) | histórico com gestos coalescidos | ✅ | `a_continuous_gesture_undoes_as_one_step` |
| Janelas F5/F6/F7/F9 | Playlist, Channel rack, Piano roll, Mixer | ✅ | `workspace::tests::*`, `piano_roll_click_adds_a_note...` |
| CPU, polifonia, osciloscópio | medidos no callback | 🟡 | memória em branco |
| Adicionar, excluir, clonar, mover, renomear canal | comandos e UI | ✅ | `channel_solo_clone_and_move` |
| Mute/Solo de canal | LED (clique; Ctrl/Alt/direito = solo) | ✅ | falta o menu do clique direito |
| Volume/Pan de canal | knobs arrastáveis; curva H-VOL-1 | 🟡 | curva intermediária não verificada |
| Roteamento canal → mixer, Ctrl+L | arrastar o seletor; insert livre | ✅ | `mixer_routing_faders...` |
| Step sequencer (clique, pintura) | sim | ✅ | `channel_rack_steps_render_at_the_step_grid` |
| Graph editor | janela Ctrl+K com 8 abas, arrastar, rampa, reset | 🟡 | `graph_editor_sets_ramps_and_resets_step_properties`; falta "Rep" |
| Swing global/canal | knob global no cabeçalho do rack | 🟡 | H-SEQ-1; swing por canal só por comando |
| Comprimento do pattern | LCD no cabeçalho do rack | ✅ | CR-014 |
| Fill each 2/4/8 | menu do canal | ✅ | `context_menu_fill_each_four_steps_sets_real_steps` |
| Cut groups / cut itself | engine e menu | ✅ | `cut_groups_choke_the_cut_by_channel` |
| Sampler: carregar wav/mp3/ogg/flac | decodificação Symphonia | ✅ | `wav_round_trips_through_the_decoder` |
| Sampler: nível padrão −8,2 dB | medido no render | ✅ | `default_sampler_plays_8_2_db_under_the_recorded_level` |
| Sampler: loop points, start offset, envelopes | Channel Settings | 🟡 | SMP-005 ✅; SMP-008 só envelope de volume; falta LFO |
| Mono, Porta, slide notes | glide de pitch exato por amostra | 🟡 | SMP-009 / PR-015: `slide_notes_bend_...`, `porta_glides_...`, `mono_overlapping_...`; tempo SLIDE é H-SMP-1 |
| Sintetizador | próprio (2 osc, ADSR, SVF) | ✅ | `sine_voice_plays_the_requested_frequency` |
| Piano roll: Draw/Paint/Delete/Mute/Select | sim | ✅ | `piano_roll_click_adds_a_note...` |
| Piano roll: mover/redimensionar/snap | sim | ✅ | `piano_roll_editing_tools` (engine) |
| Quantize, chop, glue, duplicate, transpose, discard lengths | sim | ✅ | `piano_roll_editing_tools` |
| Editor de eventos | Velocity, Pan, Release, Fine pitch, Mod X/Y | 🟡 | `event_editor_targets_edit_note_properties`; faltam cutoff/resonance e eventos de automação |
| Teclado de preview | notas ao vivo | ✅ | `live_notes_sound_until_note_off` |
| Copy/Paste de notas | Ctrl+C/X/V | ✅ | PR-020 |
| Chord stamp, snap to scale, flip, strum, limit, arpeggiate | menu Tools e seletores | 🟡 | `piano_roll_tools_flip_strum_limit_stamp_arpeggiate_and_snap`; faltam diálogos com parâmetros, Flam, Randomize, Articulate |
| Import/Export MIDI | SMF tipo 0/1 | ✅ | AE-008 |
| Playlist: pattern clips | criar, mover, redimensionar, apagar, mutar | 🟡 | PL-005: faltam Slip, Slice, seleção de timeline |
| Playlist: audio clips | soltar arquivo, waveform, offset | 🟡 | PL-003: faltam ganho, fades e stretch |
| Playlist: automation clips | criar pelo knob, editar pontos e curvas | 🟡 | PL-004 / AUT-001: faltam LFO e gravação |
| Arranjos e time markers | menu do título, Alt+T | 🟡 | PL-008/009: faltam Merge e os tipos de marcador |
| Mute de faixa/clip | engine | ✅ | `song_mode_places_clip_windows...` |
| Mixer: fader, pan, mute/solo, meters | sim, mais invert, swap L/R e stereo sep | 🟡 | fader H-MIX-1 |
| Sends pós-fader, Route to Master | sim, sem ciclos | ✅ | `sends_route_post_fader...` |
| 10 FX slots | 11 efeitos próprios, mix, enable, mover | ✅ | `effects_keep_their_state_across_plan_swaps` |
| Export WAV 16/24/32 | Song/Pattern, Tail leave/cut/wrap | ✅ | `offline_render_leaves_the_remainder...` |
| Export MP3/OGG/FLAC | — | ⬜ | AE-007 (encoder com licença compatível) |
| Render == playback | mesmo renderizador | ✅ | `offline_render_..._matches_live_playback` |
| Callback sem alocação | assert_no_alloc | ✅ | `processing_a_busy_project_does_not_allocate` |
| Browser: pastas, preview, arrastar | sim | 🟡 | busca, favoritos, abas |
| Projeto: salvar/abrir com samples relativos | sim | ✅ | `save_and_load_keep_samples_and_render_identically` |
| Autosave/backups | a cada 5 min e ao salvar; revert | 🟡 | FILE-003: faltam os outros modos |
| Entrada MIDI e typing keyboard | todas as portas, Ctrl+T | 🟡 | MIDI-001: faltam escolha de dispositivo, CC, gravação de notas |
| Configurações de áudio (F10) | dispositivo, taxa, buffer | 🟡 | AE-004 ✅ no hardware; GLB-021: faltam as outras abas |
| VST3/CLAP | — | ⬜ | PLG-001/002 (licença do VST3 resolvida: MIT desde a 3.8) |
| Gravação de áudio | R arma, Play grava, take vira Audio Clip | 🟡 | REC-001: não exercitado no microfone do usuário; faltam pickup, takes e monitoramento |

## 3. Itens bloqueados

| Item | Motivo |
|---|---|
| CR-031 Loop Starter | serviço on-line proprietário (FL Cloud) |
| FILE-006 Ler .flp | sem especificação pública; fora de escopo pela ADR |
| BR-005 aba SOUNDS | conteúdo FL Cloud. A linha BR-005 conta como ⬜ no resumo, porque as demais abas ainda não existem |

## 4. Diferenças conhecidas em itens marcados ✅ ou 🟡

- O nível dos canais, faders e sends usa as curvas H-VOL-1, H-MIX-1 e H-MIX-2. Só os pontos citados do manual são exatos.
- O pan do mixer soma os lados como o manual descreve, mas sem lei de compensação medida.
- O swing máximo, o gate de step, o pattern vazio e o loop do fim da música são as hipóteses H-SEQ-1 a H-SEQ-4.
- A geometria do Piano roll e do Mixer é nossa (UNMEASURED); a fidelidade visual delas não foi medida.
