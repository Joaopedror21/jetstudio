# Matriz de paridade funcional: FL Studio 2026 × JetStudio

> **Atualizado em:** 24/10/2026. **Fonte dos itens:** [FL_STUDIO_COMPLETE_FEATURES.md](FL_STUDIO_COMPLETE_FEATURES.md), que tem os IDs, as referências e os testes.
> **Regra:** uma funcionalidade só conta como disponível quando o comportamento existe e tem teste. Botão desenhado não conta. Nenhum item é "Validado", porque nenhum comportamento foi ainda comparado lado a lado com o FL (veja os pedidos de captura em `PROGRESS.md`).

Legenda: ✅ disponível e testado · 🟡 parcial (o que falta está ao lado) · ⬜ não implementado · ⛔ bloqueado.

## 1. Resumo por sistema

| Sistema | Itens catalogados | ✅ | 🟡 | ⬜ | ⛔ |
|---|---:|---:|---:|---:|---:|
| Interface e controles globais (GLB) | 23 | 7 | 7 | 9 | 0 |
| Channel Rack e Step Sequencer (CR) | 33 | 14 | 9 | 9 | 1 |
| Sampler e instrumentos (SMP/INS) | 19 | 6 | 2 | 11 | 0 |
| Piano Roll (PR) | 27 | 7 | 8 | 12 | 0 |
| Playlist (PL) | 17 | 4 | 5 | 8 | 0 |
| Mixer (MX) | 17 | 6 | 8 | 3 | 0 |
| Motor de áudio (AE) | 15 | 7 | 3 | 5 | 0 |
| Efeitos (FX) | 14 | 7 | 1 | 6 | 0 |
| Plugins externos (PLG) | 4 | 0 | 0 | 4 | 0 |
| Browser e arquivos (BR/FILE) | 12 | 2 | 3 | 6 | 1 |
| Automação, MIDI, gravação (AUT/MIDI/REC) | 10 | 0 | 0 | 10 | 0 |
| **Total** | **191** | **60** | **46** | **83** | **2** |

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
| Graph editor | engine aplica note, vel, pan, pitch, shift | 🟡 | falta o editor visual e "Rep" |
| Swing global/canal | engine | 🟡 | H-SEQ-1; falta o knob |
| Comprimento do pattern | engine | 🟡 | falta o controle na UI |
| Fill each 2/4/8 | menu do canal | ✅ | `context_menu_fill_each_four_steps_sets_real_steps` |
| Cut groups / cut itself | engine e menu | ✅ | `cut_groups_choke_the_cut_by_channel` |
| Sampler: carregar wav/mp3/ogg/flac | decodificação Symphonia | ✅ | `wav_round_trips_through_the_decoder` |
| Sampler: nível padrão −8,2 dB | medido no render | ✅ | `default_sampler_plays_8_2_db_under_the_recorded_level` |
| Sampler: loop points, start offset, envelopes | — | ⬜ | SMP-005/008 |
| Sintetizador | próprio (2 osc, ADSR, SVF) | ✅ | `sine_voice_plays_the_requested_frequency` |
| Piano roll: Draw/Paint/Delete/Mute/Select | sim | ✅ | `piano_roll_click_adds_a_note...` |
| Piano roll: mover/redimensionar/snap | sim | ✅ | `piano_roll_editing_tools` (engine) |
| Quantize, chop, glue, duplicate, transpose, discard lengths | sim | ✅ | `piano_roll_editing_tools` |
| Editor de eventos | só velocity | 🟡 | pan, release, Mod X/Y, pitch |
| Teclado de preview | notas ao vivo | ✅ | `live_notes_sound_until_note_off` |
| Copy/Paste de notas | — | ⬜ | PR-020 |
| Playlist: pattern clips | criar por clique; tocar no modo Song | 🟡 | mover, redimensionar e apagar só por comando |
| Playlist: audio clips | — | ⬜ | PL-003 |
| Playlist: automation clips | — | ⬜ | PL-004 / AUT-001 |
| Mute de faixa/clip | engine | ✅ | `song_mode_places_clip_windows...` |
| Mixer: fader, pan, mute/solo, meters | sim | 🟡 | fader H-MIX-1 |
| Sends pós-fader, Route to Master | sim, sem ciclos | ✅ | `sends_route_post_fader...` |
| 10 FX slots | 8 efeitos próprios, mix, enable, mover | ✅ | `effects_keep_their_state_across_plan_swaps` |
| Export WAV 16/24/32 | Song/Pattern, Tail leave/cut/wrap | ✅ | `offline_render_leaves_the_remainder...` |
| Export MP3/OGG/FLAC/MIDI | — | ⬜ | AE-007/008 |
| Render == playback | mesmo renderizador | ✅ | `offline_render_..._matches_live_playback` |
| Callback sem alocação | assert_no_alloc | ✅ | `processing_a_busy_project_does_not_allocate` |
| Browser: pastas, preview, arrastar | sim | 🟡 | busca, favoritos, abas |
| Projeto: salvar/abrir com samples relativos | sim | ✅ | `save_and_load_keep_samples_and_render_identically` |
| Autosave/backups | — | ⬜ | FILE-003 |
| Entrada MIDI | renderizador pronto | ⬜ | MIDI-001 |
| VST3/CLAP | — | ⬜ | PLG-001/002 (licença do VST3 resolvida: MIT desde a 3.8) |
| Gravação de áudio | — | ⬜ | REC-001 |

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
