# Inventário de funcionalidades do FL Studio 2026

> **Para que serve:** catálogo expansível das funcionalidades do FL Studio 2026 que o JetStudio precisa reproduzir. Para cada uma registra a fonte, o comportamento verificado, as dependências, a prioridade, o estado real no JetStudio e os testes.
> **Atualizado em:** 24/10/2026 (sessão Claude, branch `claude/M1-visual-fidelity`).
> **Matriz resumida:** [FEATURE_PARITY_MATRIX.md](FEATURE_PARITY_MATRIX.md). **Andamento:** [PROGRESS.md](PROGRESS.md).

## 1. Fontes e regras de evidência

- **[Manual: página]** é o manual oficial on-line do FL Studio 2026: `https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/<página>.htm`. As páginas foram baixadas em 23 e 24/10/2026 e lidas como texto. As citações entre aspas são trechos literais dessas páginas.
  Páginas lidas nesta sessão: `channelrack`, `chansettings_sampler`, `chansettings_misc`, `pianoroll`, `pianoroll_menu`, `playlist`, `mixer`, `mixer_levelsandmixing`, `songsettings_settings`, `fformats_save_export`, `toolbar_panels`, `app_feature`.
  Páginas baixadas e consultadas por títulos de seção: `playlist_automationclip`, `playlist_audioclip`, `recording_audio`, `recording_automation`, `basics_externalplugins`, `envsettings_audio`, `envsettings_midi`, `envsettings_files`, `mixer_trackprops`, `menu_*`.
- **[W]**, **[B]**, **[C]** são capturas de tela do FL Studio 2026 Trial feitas pelo usuário (ver `docs/SCREENSHOT_INVENTORY.md`).
- **Hipótese H-xxx:** comportamento que o manual não especifica. Implementamos uma escolha nossa, explícita e testada, e ela fica pendente até ser observada no FL. Nenhuma hipótese é marcada como "Validado".
- Não há engenharia reversa de binários, nem leitura de `.flp`, nem uso de assets ou código da Image-Line.

### 1.1 Estados

| Estado | Significado |
|---|---|
| Não pesquisado | Ainda não lido no manual. |
| Pesquisado | Lido no manual; comportamento resumido aqui. |
| Especificado | Pesquisado, e o desenho da implementação está decidido. |
| Em implementação | Há código parcial em andamento. |
| Implementado | O código existe e funciona, mas não tem teste automatizado próprio. |
| Testado | Implementado e coberto por teste automatizado (o teste está listado). |
| Validado | Testado e comparado com o comportamento observado no FL (captura ou medição). |
| Parcialmente compatível | Funciona, mas difere do FL em pontos listados. |
| Bloqueado | Depende de algo indisponível (licença, serviço on-line, SDK, captura). |

**Prioridade:** P0 é o núcleo sem o qual a DAW não funciona; P1 é o fluxo principal de produção; P2 é recurso avançado; P3 é periférico ou on-line.
**Complexidade:** S (horas), M (1 a 3 dias), L (1 a 2 semanas), XL (mais que isso).

### 1.2 Edições do FL Studio (verificado)

[Manual: app_feature] "In terms of core program functions and features, there are two editions of FL Studio Fruity Edition and Producer Edition. All higher 'Editions' are Producer Edition plus more plugins". Producer Edition "Provides full Playlist functionality, Audio Recording, Automation Clips, Slicex, Edison". Fruity Edition serve para quem "need[s] the Piano roll and Pattern Clips in the Playlist for sequencing, but don't need more than 8 Audio Clips or audio recording". No modo Trial, o projeto salvo só reabre depois da compra.
**Consequência:** o alvo funcional do JetStudio é o **Producer Edition**. Plugins pagos à parte (Harmor, Sytrus etc.) são catalogados apenas como **categorias**, nunca como clones.

## 2. Registro de hipóteses

| ID | Tema | Escolha implementada | Por que é hipótese | Onde está |
|---|---|---|---|---|
| H-VOL-1 | Curva do volume do canal | `ganho = knob^2,4224`, ajustada aos dois pontos do manual: 100% = 0 dB e 78% = −5,2 dB | O manual dá só esses dois pontos | `jet-dsp/src/level.rs` |
| H-VEL-1 | Velocity → ganho | `ganho = vel/100` (velocity 100 = 0 dB) | Deduzido do total "8.2 dB lower" = 5,2 dB + 3 dB [Manual: mixer_levelsandmixing] | `jet-dsp/src/level.rs` |
| H-MIX-1 | Fader do mixer | 80% do curso = 0 dB, topo = +5,6 dB, curva potência | Nenhuma página baixada dá a escala do fader | `jet-dsp/src/level.rs` |
| H-MIX-2 | Knob de send | mesma curva do fader | Idem | `jet-dsp/src/level.rs` |
| H-SEQ-1 | Swing máximo | 100% de swing atrasa os steps pares em ½ step | O manual descreve só a direção ("holds odd 16th notes") | `jet-seq/src/lib.rs` |
| H-SEQ-2 | Gate dos steps | Nota de step (comprimento zero) segura por 1 step em instrumentos com envelope | O manual diz só que steps são notas de comprimento zero | `jet-seq/src/lib.rs` |
| H-SEQ-3 | Pattern vazio | Dura 1 compasso | Não documentado | `jet-seq/src/lib.rs` |
| H-SEQ-4 | Fim da música | No modo Song, o playback volta ao início no fim do último compasso com dados | O manual descreve o marcador "Loop" de retorno, mas não o comportamento sem marcador | `jet-audio-graph` (loop) |
| H-PR-1 | Ctrl+B no Piano roll | Duplica com intervalo = vão da seleção arredondado para cima ao snap | O manual diz "to the right of the current location" | `jet-engine/src/cmd/note.rs` |
| H-BR-1 | Nível do preview do Browser | Ganho unitário direto no Master | Não documentado | `jet-audio-graph/src/render.rs` |
| H-PL-1 | Snap da Playlist | Clips andam de 1 em 1 batida; Alt desliga o snap | O snap padrão "Line" do FL acompanha o zoom | `jet-ui/src/playlist.rs` |
| H-AUT-1 | Valor depois de uma Automation Clip | O último valor se mantém até a próxima clip; antes da primeira vale o knob; com o transporte parado vale o knob | O manual descreve só o valor inicial ("Initialized controls") | `jet-seq/src/automation.rs` |
| H-AUT-2 | Formas das 10 curvas | Fórmulas próprias (potência 8^tensão, S, degraus, pulso, seno) | O manual dá só os nomes | `jet-seq/src/automation.rs` |
| H-AUT-3 | Taxa de controle | Uma avaliação por bloco (até 1024 frames) | Não documentado | `jet-audio-graph/src/render.rs` |
| H-KB-1 | Mapa do teclado de digitação | Duas fileiras: Z…M + teclas pretas em S, D, G, H, J (oitava inferior); Q…P + 2, 3, 5, 6, 7, 9 (superior); Z = C4 | O manual menciona "Standard keyboard layouts" sem mostrar o mapa | `jet-ui/src/app.rs` |
| H-REC-1 | Início da take | A take começa na posição do transporte no primeiro quadro de UI depois do Play (granularidade de ~30 ms) | O FL usa os relógios do driver | `jet-ui/src/app.rs` |
| H-PR-3 | Arpeggiate | Cada acorde vira uma sequência de notas de `step` sobre sua duração (up, down, updown) | A página do Arpeggiator do manual responde 404 | `jet-engine/src/cmd/note_tools.rs` |
| H-CR-2 | Faixa da aba Note do Graph editor | Duas oitavas para cada lado da nota raiz do canal | O manual não dá a faixa | `jet-ui/src/graph_editor.rs` |
| H-CR-3 | Ctrl no Graph editor | Todas as colunas mudam pelo mesmo deslocamento da coluna segura | O manual diz só "the others will follow" | `jet-ui/src/graph_editor.rs` |
| H-SMP-1 | Tempo SLIDE (Portamento Time) | 0 a 2000 ms, padrão 100 ms; glide linear em cents | O manual diz só "The more the knob is turned right, the longer the slide" | `jet-model` `Polyphony::slide_ms`, `jet-dsp/src/glide.rs` |
| H-UI-1 | Sensibilidade dos knobs e faders | 200 px por curso de knob; 150 px por curso de fader; 6 px por insert | Não medido | `jet-ui` |

## 3. Interface e controles globais (GLB)

| ID | Nome | Referência | Comportamento verificado (resumo) | Dep. | Cx | P | Estado JetStudio | Testes |
|---|---|---|---|---|---|---|---|---|
| GLB-001 | Menus principais | [Manual: menu_*], [W] | FILE, EDIT, ADD, PATTERNS, VIEW, OPTIONS, TOOLS, HELP | — | M | P1 | Parcialmente compatível: FILE (New/Open/Save/Save as/Export WAV), EDIT (Undo), ADD (Channel Sampler/Synth), VIEW (Playlist, Channel rack, Browser, Piano roll, Mixer); os demais itens aparecem como "Not implemented yet" | `app::tests::*` (indireto) |
| GLB-002 | Play/Pause | [Manual: toolbar_panels] | Space faz play/stop, Ctrl+Space pausa; clique esquerdo toca ou pausa | engine | S | P0 | Testado | `silent_transport_uses_injected_clock_and_pause_freezes_position`, `pointer_click_adds_a_real_channel_and_transport_starts_and_stops` |
| GLB-003 | Stop | [Manual: toolbar_panels] "Double-click to stop all notes (panic mode)" | Stop solta as notas; o duplo clique é pânico | GLB-002 | S | P0 | Parcialmente compatível: stop + comando `transport.panic`; falta o duplo clique e o "Remember seek time" | `stop_releases_synth_voices_and_keeps_sampler_tails` |
| GLB-004 | PAT/SONG | [Manual: toolbar_panels] tecla L | O modo Pattern repete o pattern selecionado; o modo Song toca o arranjo | jet-seq | S | P0 | Testado | `pattern_mode_loops_the_pattern`, `playlist_song_mode_plays_clips_at_their_positions` |
| GLB-005 | Tempo | [Manual: toolbar_panels] 10–522 BPM, precisão de 1/1000 BPM | LCD editável | — | S | P0 | Testado (edição por digitação; falta arrastar o LCD) | `keyboard_toggles_playlist_and_tempo_editor_commits_value` |
| GLB-006 | Tap tempo | [Manual: toolbar_panels] "Tap" | O tempo se ajusta a partir do 2º toque | GLB-005 | S | P2 | Pesquisado | — |
| GLB-007 | Fórmula de compasso | [Manual: songsettings_settings] | Numerador e denominador; BPM conta semínimas | jet-time | S | P1 | Testado no engine (`project.time_sig.set`); sem controle na UI | `step_ticks_follow_ppq_and_steps_per_beat` |
| GLB-008 | Metrônomo | [Manual: toolbar_panels] "Plays a click at each beat"; [menu_options] "an accent at the beginning of each bar" (Ctrl+M) | Clique por tempo, acento no 1º | AE | S | P1 | Parcialmente compatível: som próprio (FL usa Metronome.wav), exato por amostra, nunca exportado; falta escolher som e mixer track | `metronome_clicks_on_each_beat_with_an_accent_on_the_bar`, `typing_keyboard_plays_notes...` |
| GLB-009 | Count-in | [Manual: toolbar_panels] | Contagem antes de gravar | REC | S | P2 | Pesquisado | — |
| GLB-010 | Loop recording | [Manual: toolbar_panels] | Gravação em loop do pattern ou seleção | REC | M | P2 | Pesquisado | — |
| GLB-011 | Snap global | [Manual: pianoroll_menu "Snap"] | Grade de edição; Alt ignora o snap | — | S | P1 | Parcialmente compatível: snap próprio no Piano roll ((none), 1/4 step, 1/2 step, Step, Beat, Bar); o seletor global "Line" é só visual | `piano_roll_click_adds_a_note_and_mixer_fader_moves_the_master` |
| GLB-012 | Seletor de pattern | [Manual: channelrack §16] | +/− e F4 para o próximo pattern vazio | — | S | P0 | Testado | `patterns_keep_independent_steps_and_undo_toggle_restores_them` |
| GLB-013 | Undo/Redo | [Manual: basics_shortcuts] Ctrl+Z alterna; Ctrl+Alt+Z desfaz passo a passo | Gestos contínuos viram um passo só | engine | M | P0 | Testado | `a_continuous_gesture_undoes_as_one_step`, `dragging_a_volume_knob_changes_volume_in_one_undo_step` |
| GLB-014 | Histórico de ações | [Manual: menu_edit] | Lista navegável | GLB-013 | M | P2 | Pesquisado | — |
| GLB-015 | Hint bar | [Manual: toolbar_hint] | Dica do controle sob o mouse | — | S | P1 | Implementado (dica, erro ou status do áudio) | — |
| GLB-016 | CPU / polifonia / memória | [W] painel do toolbar | Carga e vozes reais | AE | S | P1 | Implementado: CPU = tempo do callback / duração do buffer; polifonia = vozes ativas; memória em branco | (medido em `jet-cli play`) |
| GLB-017 | Osciloscópio do toolbar | [W] | Forma de onda do Master | AE | S | P2 | Implementado (ring buffer atômico do Master) | — |
| GLB-018 | Gerenciamento de janelas | [Manual: basics_windows] | Janelas flutuantes que encaixam umas nas outras (Alt desliga), maximizar, F5/F6/F7/F9, F12 fecha todas | — | M | P0 | Testado (Playlist, Channel rack, Piano roll, Mixer) | `workspace::tests::*` |
| GLB-019 | Detach de janelas | [Manual: basics_windows] | Janela vira janela do sistema operacional | GLB-018 | M | P3 | Pesquisado | — |
| GLB-020 | Atalhos globais | [Manual: basics_shortcuts] | F5, F6, F7, F9, Alt+F8, Ctrl+S/O, Space, L, F4, +/−, Ctrl+Z, Ctrl+L, Ctrl+R | — | S | P1 | Implementado (lista ao lado) | `app::tests::*` |
| GLB-021 | Configurações (F10) | [Manual: envsettings_*] "press F10 to open the settings panel" | Áudio, MIDI, general, arquivos | AE | L | P1 | Parcialmente compatível: aba Audio (dispositivo, taxa, buffer com latência, status, callbacks atrasados), guardada em `%APPDATA%\JetStudio\settings.json`; faltam MIDI, General, File | `f10_opens_audio_settings_and_apply_keeps_the_choice` |
| GLB-022 | Personalização do toolbar | [Manual: toolbar_panels] | Painéis configuráveis | — | M | P3 | Pesquisado | — |
| GLB-023 | Main volume / main pitch | [Manual: toolbar_panels] | Knobs globais | AE | S | P2 | Pesquisado (campos no modelo, sem efeito no áudio) | — |

## 4. Channel Rack e Step Sequencer (CR)

| ID | Nome | Referência | Comportamento verificado | Dep. | Cx | P | Estado | Testes |
|---|---|---|---|---|---|---|---|---|
| CR-001 | Adicionar canal | [Manual: channelrack §1 "Add one", §15] | Adiciona no fim; ADD > Channel | engine | S | P0 | Testado (Sampler e Synth) | `pointer_click_adds_a_real_channel...`, `channel_solo_clone_and_move` |
| CR-002 | Excluir canal | "Delete selected (Alt+Del)" | Remove o canal, seus steps e notas | CR-001 | S | P0 | Testado | `strict_command_validation...` + menu |
| CR-003 | Clonar canal | "Clone ... including settings and generator type"; clone dos selecionados "does not contain its notes" | Inserido logo após o original | CR-001 | S | P1 | Testado | `channel_solo_clone_and_move` |
| CR-004 | Renomear | Menu "Rename, recolor and icon" (F2) | Edição de nome | — | S | P1 | Implementado (edição inline com Enter/Esc) | — |
| CR-005 | Cor do canal | "Color selected", "Gradient", "Random" | Cor do botão | — | S | P2 | Parcialmente compatível: comando `channel.color.set`; falta o seletor de cor | — |
| CR-006 | Mover canal | "Move selected up/down (Alt+Up/Down)", Shift+roda | Reordena | — | S | P1 | Testado no engine; sem atalho na UI | `channel_solo_clone_and_move` |
| CR-007 | Mute | "Mute/Unmute - (Left-Click)" no LED | Mute vale para o playback de steps e do Piano roll; MIDI ao vivo continua | jet-seq | S | P0 | Testado | `muted_channels_produce_no_events`, `channel_volume_pan_and_mute_change_the_audio` |
| CR-008 | Solo | "(Ctrl+Left-Click), (Alt+Left-Click) OR (Right+Click)"; repetir "unsolo ... (unmute all)" | Muta todos os outros | CR-007 | S | P1 | Testado (Ctrl/Alt+clique e clique direito fazem solo direto, sem o menu) | `channel_solo_clone_and_move` |
| CR-009 | Volume do canal | §7-8: Sampler carrega em "-5.2 dB (78%)"; 100% restaura o nível original | Knob pré-mixer | jet-dsp | S | P0 | Parcialmente compatível (H-VOL-1); arraste vertical | `channel_volume_matches_the_two_manual_points`, `default_sampler_plays_8_2_db...` |
| CR-010 | Pan do canal | §7 | Pan enviado ao instrumento; lei circular | jet-dsp | S | P0 | Testado | `circular_pan_is_minus_three_db...`, `channel_volume_pan_and_mute_change_the_audio` |
| CR-011 | Seletor de mixer track | §14: "Click and drag vertically"; "---" = Master | Roteamento canal → insert | MX | S | P0 | Testado | `mixer_routing_faders_sends_and_effects_change_the_output` |
| CR-012 | Assign to free mixer track (Ctrl+L) | §1 | Primeiro insert livre, com o nome do canal | MX | S | P1 | Testado | idem |
| CR-013 | Step sequencer | §13: cada step é uma semicolcheia; clique esquerdo liga, direito desliga | Grade de steps | jet-seq | S | P0 | Testado (clique e pintura arrastando) | `step_buttons_and_playlist_grid_change_the_project`, `channel_rack_steps_render_at_the_step_grid` |
| CR-014 | Comprimento do pattern | §4: Auto = "end of the last bar with data"; 1–512 steps | — | jet-seq | S | P1 | Testado (LCD no cabeçalho: arrastar define os steps, botão direito volta ao Auto) | `auto_length_rounds...`, `rack_header_sets_swing_and_pattern_length` |
| CR-015 | Swing global e por canal | §3: dois multiplicadores; os dois > 0 para haver swing; move os steps pares | — | jet-seq | S | P1 | Parcialmente compatível (H-SEQ-1); knob global no cabeçalho do rack; swing por canal só por comando | `swing_needs_both...`, `rack_header_sets_swing_and_pattern_length` |
| CR-016 | Graph editor | §6: Note, Velocity, Release, Fine pitch (±100 cents), Pan, Mod X/Y, Shift (até ~1 step), Rep | Propriedades por step | CR-013 | M | P1 | Parcialmente compatível: janela Graph editor (Ctrl+K ou menu do canal) com abas Note, Velocity, Release, Fine pitch, Pan, Mod X, Mod Y e Shift; arrastar define as barras (arrasto rápido preenche as colunas do meio), Ctrl+arrastar desloca todas (H-CR-3), botão direito arrastado faz rampa, Alt+clique/arrasto volta ao padrão; faixa da aba Note é H-CR-2; nossos instrumentos ignoram Release e Mod X/Y; faltam Rep e o modo acoplado ao rack | `graph_editor_sets_ramps_and_resets_step_properties`, `graph_editor_pan_and_shift_change_the_render`, `step_fill_and_graph_editor_change_what_plays` |
| CR-017 | Fill each 2/4/8 steps | Menu do canal | — | CR-013 | S | P1 | Testado | `context_menu_fill_each_four_steps_sets_real_steps` |
| CR-018 | Advanced fill | [Manual: stepsequencer_advancedfilltool] | Preenchimento por regras | CR-013 | M | P2 | Pesquisado | — |
| CR-019 | Rotate left/right | "Shift+Ctrl+Left/Right ... Step-notes are unaffected" | Move as notas do Piano roll | PR | S | P2 | Pesquisado | — |
| CR-020 | Mute removed steps | §1 opção | Desligar um step preserva as propriedades | CR-013 | S | P1 | Implementado (desligar mantém as props) | — |
| CR-021 | Cut / Cut by / Cut itself | [Manual: chansettings_misc "Cut Groups"]; cut "sends an all-notes off (release)" | Choke de hi-hat | jet-audio-graph | S | P1 | Testado | `cut_groups_choke_the_cut_by_channel` |
| CR-022 | Grupos / filtro de exibição | "Channel Filter Groups", Alt+G | Mostrar subconjuntos | — | M | P2 | Pesquisado (campo `group` no modelo) | — |
| CR-023 | Zip / Unzip | Alt+Z / Alt+U | Modo compacto | — | S | P3 | Pesquisado | — |
| CR-024 | Sort by color/name/track/type | §1 | Reordenar | CR-006 | S | P3 | Pesquisado | — |
| CR-025 | Select unused channels | §1 | Seleciona canais sem notas | — | S | P3 | Pesquisado | — |
| CR-026 | Transpose selected | §1 | Pitch em semitons | CR-009 | S | P2 | Testado no engine (`channel.pitch.set`, em cents) | — |
| CR-027 | Preview do Piano roll | §11: substitui os steps; não é editável; clique abre o Piano roll | Mini-notas | PR | S | P1 | Implementado | — |
| CR-028 | Looping de canais | §5: Loop step channels/all channels/advanced | Loops independentes por canal | jet-seq | M | P2 | Pesquisado | — |
| CR-029 | Carregar sample por arraste | [Manual: chansettings_sampler] "drag and drop the sample from the Browser or Windows Explorer onto Channel Rack" | Soltar numa linha troca o sample; soltar no vazio cria canal | BR | S | P0 | Implementado (Browser e SO) | engine: `channel.sample.load` testado |
| CR-030 | Canais vermelhos (sample ausente) | §"Red Channel buttons" | Recurso ausente no load | FILE | S | P1 | Parcialmente compatível: marca `ResourceStatus::Missing`; o botão não fica vermelho | `missing_resource_is_reported_but_project_loads` |
| CR-031 | Loop Starter | §2 (FL Cloud) | Serviço on-line | — | — | P3 | Bloqueado (serviço proprietário on-line) | — |
| CR-032 | Receive notes from (MIDI) | §"Lock Controller Notes" | Fixa o controlador no canal | MIDI | M | P2 | Pesquisado | — |
| CR-033 | Burn MIDI to pattern | §9 | Grava a saída MIDI do plugin | PLG | M | P3 | Pesquisado | — |

## 5. Sampler (SMP) e instrumentos (INS)

| ID | Nome | Referência | Comportamento verificado | Dep. | Cx | P | Estado | Testes |
|---|---|---|---|---|---|---|---|---|
| SMP-001 | Carregar/remover sample | [Manual: chansettings_sampler "File"] | wav/mp3/ogg (FLAC é importado) | jet-media | S | P0 | Testado | `wav_round_trips_through_the_decoder`, `channel_rack_steps_render_at_the_step_grid` |
| SMP-002 | Root key C5 e transposição por velocidade de leitura | [Manual: chansettings_misc] "The default Root Key is C5" | Pitch muda a velocidade | jet-dsp | S | P0 | Testado | `octave_up_halves_the_duration` |
| SMP-003 | Sem envelope, o sample toca até o fim | "samples will continue to play after note release unless you activate the Volume envelope" | — | jet-dsp | S | P0 | Testado | `stop_releases_synth_voices_and_keeps_sampler_tails` |
| SMP-004 | Declick "Out only" 10 ms em S cossenoidal | "Declicking ... 'Out only' (10 ms fade)" | Só no corte | jet-dsp | S | P1 | Testado | `cut_fades_out_within_ten_ms` |
| SMP-005 | Start offset, loop start/end, ping-pong | "Playback": o loop repete "while a note is held" | — | SMP-001 | M | P1 | Testado (o loop para no note-off e o sample toca até o fim; reverse incluso) | `start_offset_skips_the_beginning`, `forward_loop_repeats_while_held...`, `ping_pong_reverses...`, `sampler_settings_and_polyphony_shape_the_output` |
| SMP-006 | Time stretch / pitch shift | "Time stretching" (Elastique é proprietário) | Modos resample/stretch | DSP | XL | P2 | Pesquisado (precisa de algoritmo próprio, ex.: phase vocoder/WSOLA) | — |
| SMP-007 | Precomputed effects | Reverse, normalize, fade, trim, boost etc. | Processamento offline do sample | jet-dsp | M | P2 | Pesquisado | — |
| SMP-008 | Envelopes ADSR vol/pan/cutoff/pitch e LFO | "Channel Settings > INS" | — | jet-dsp | L | P1 | Parcialmente compatível: envelope de volume ADSR do Sampler (liga e desliga; o release encerra a nota); faltam pan, cutoff, pitch e LFO | `envelope_release_ends_the_voice`, `sampler_settings_and_polyphony...` |
| SMP-009 | Polifonia máxima, Mono, Porta, Slide | [Manual: chansettings_misc "Polyphony"]: Max poly "Notes stop playing immediately"; Mono "Triggers the current release state" | — | jet-audio-graph | M | P1 | Parcialmente compatível: Max poly (corte com declick); Mono por grupo de cor, em que notas sobrepostas deslizam na mesma voz (legato) pelo tempo SLIDE; botão Porta (toda nota desliza da anterior); tempo SLIDE de 0 a 2000 ms (H-SMP-1); na troca, pan e velocity mudam de uma vez em vez de deslizar; falta Full porta | `sampler_settings_and_polyphony_shape_the_output`, `porta_glides_from_the_previous_note_over_the_slide_time`, `mono_overlapping_notes_glide_on_one_voice`, `polyphony_from_older_files_gets_porta_off_and_the_default_slide_time` |
| SMP-010 | Gate / Shift / Swing mix por canal | "Time" | — | jet-seq | S | P1 | Swing mix: Testado; Gate/Shift: Pesquisado | `swing_needs_both_multipliers...` |
| SMP-011 | Arpeggiator | "Arpeggiator": Up, Down, Up-Down, Random; Time, Gate, Range, Repeat, Chord | — | jet-seq | M | P2 | Pesquisado | — |
| SMP-012 | Echo delay de notas | "Echo Delay" | Ecos como notas | jet-seq | M | P3 | Pesquisado | — |
| SMP-015 | Janela Channel Settings | [Manual: channelrack §9] "Left-click - Opens/Closes the Channel Settings window" | Volume, pan, pitch, root note, swing mix, mixer, cut/cut by, polifonia; Sampler (forma de onda real, start, loop, reverse, envelope); Synth | UI | M | P1 | Testado (layout próprio, UNMEASURED) | `channel_button_opens_settings_whose_controls_edit_the_channel` |
| SMP-013 | Key region / fine tune | "Preview Keyboard" | — | — | S | P2 | Pesquisado | — |
| SMP-014 | Interpolação | [Manual: fformats_save_export] linear, 6-point hermite, sinc | Qualidade do resample | jet-dsp | M | P2 | Parcialmente compatível: Hermite de 4 pontos (escolha nossa) | `interpolation_hits_sample_points_and_is_smooth_between` |
| INS-001 | Sintetizador subtrativo próprio | (não clona 3xOsc nem Sytrus) | 2 osciladores polyBLEP (saw, square, tri, sine, noise), detune, ADSR, SVF low-pass | jet-dsp | M | P1 | Testado | `sine_voice_plays_the_requested_frequency`, `piano_roll_notes_drive_the_synth_at_their_pitch` |
| INS-002 | Drum sampler / FPC equivalente | [Manual: FPC] | Pads com vários samples | SMP | L | P2 | Pesquisado | — |
| INS-003 | Slicer equivalente (Slicex) | [Manual: Slicex] | Fatiar loops | SMP | L | P2 | Pesquisado | — |
| INS-004 | Gerenciamento de vozes | — | Pool fixo, roubo da mais antiga | jet-audio-graph | S | P0 | Testado (sem alocação) | `processing_a_busy_project_does_not_allocate` |
| INS-005 | Presets de instrumento | [Manual: browser "Presets"] | Salvar e carregar estado | FILE | M | P2 | Pesquisado | — |

## 6. Piano Roll (PR)

| ID | Nome | Referência | Comportamento verificado | Dep. | Cx | P | Estado | Testes |
|---|---|---|---|---|---|---|---|---|
| PR-001 | Notas como eventos reais | [Manual: pianoroll] | Cada nota tem início, duração, tecla, velocity, pan, release, fine pitch, Mod X/Y, mute, slide, porta e cor (16 grupos) | jet-model | S | P0 | Testado | `piano_roll_notes_drive_the_synth_at_their_pitch` |
| PR-002 | Draw (P) | "Left-click to add a note ... Right-Click to delete" | Comprimento padrão = "last touched note" | PR-001 | S | P0 | Testado | `piano_roll_click_adds_a_note...` |
| PR-003 | Paint (B) | "Paints multiple notes as you drag" | — | PR-002 | S | P1 | Implementado | — |
| PR-004 | Delete (D), Mute (T), Select (E) | Ferramentas | — | PR-001 | S | P1 | Implementado | — |
| PR-005 | Mover e redimensionar com snap; Alt ignora o snap; Shift/Ctrl travam um eixo | "Moving & resizing notes" | — | PR-001 | M | P0 | Implementado (aplicado ao soltar) | engine: `piano_roll_editing_tools` |
| PR-006 | Seleção (Ctrl+arrastar, Ctrl+A, Shift adiciona) | "Selecting/Deselecting" | — | — | S | P1 | Implementado | — |
| PR-007 | Delete sem seleção apaga tudo | [Manual: pianoroll_menu] "If no selection is made - deletes all Notes" | — | — | S | P1 | Testado | `piano_roll_editing_tools` |
| PR-008 | Duplicate (Ctrl+B) | [Manual: pianoroll_menu] | À direita da seleção | — | S | P1 | Parcialmente compatível (H-PR-1) | `piano_roll_editing_tools` |
| PR-009 | Quick quantize (Ctrl+Q) e quantize de inícios (Shift+Q) | [Manual: pianoroll_menu] | Pelo snap | — | S | P1 | Testado | `piano_roll_editing_tools` |
| PR-010 | Quick chop (Ctrl+U), Glue (Ctrl+G), Discard lengths (Shift+D) | [Manual: pianoroll_menu] | — | — | S | P1 | Testado | `piano_roll_editing_tools` |
| PR-011 | Transpor semitom (Shift+setas) e oitava (Ctrl+setas) | [Manual: pianoroll_menu] | — | — | S | P1 | Testado | `piano_roll_notes_drive_the_synth_at_their_pitch` |
| PR-012 | Editor de eventos (propriedades de nota) | "Note Properties & Event Editor" | Hastes com bolinha | PR-001 | M | P1 | Parcialmente compatível: alvos Velocity, Pan, Release, Fine pitch, Mod X e Mod Y (clique, botão direito, roda do mouse ou Shift+F trocam); hastes com bolinha e cauda; arrastar edita a nota próxima ou a seleção; Shift+arrastar aplica o valor inicial a todas as notas atravessadas; Alt+clique volta ao padrão; nossos instrumentos ignoram Release e Mod X/Y; faltam Alt+roda sobre a nota, cutoff/resonance e eventos de automação | `event_editor_targets_edit_note_properties` |
| PR-013 | Teclado de preview | "Preview keyboard" | Clique toca a nota | AE | S | P1 | Testado (notas ao vivo) | `live_notes_sound_until_note_off` |
| PR-014 | Zoom e scroll | Ctrl+roda, Shift+roda | — | — | S | P1 | Implementado | — |
| PR-015 | Slide e portamento | "Understanding Slides & Portamento" | Só em instrumentos nativos | INS | L | P2 | Parcialmente compatível: toggles Slide (S) e Porta (O) para notas novas, com marcas nas notas; slide notes não soam e levam as notas sobrepostas da mesma cor até o tom da slide ao longo do seu comprimento; num acorde a nota mais alta é a referência e todas movem o mesmo intervalo; o desvio fica depois da slide; notas porta deslizam da nota anterior pelo tempo SLIDE; o render é igual com qualquer tamanho de bloco; faltam o ciclo por clique-e-segurar, a transição de velocity/pan durante a slide e o stamp Slide up | `slide_notes_bend_overlapping_notes_of_their_color_and_are_silent`, `porta_glides_from_the_previous_note_over_the_slide_time`, `slide_toggle_adds_slide_notes`, `glides_linearly_in_cents_and_holds_the_target` |
| PR-016 | Ghost notes | "Ghost Notes" | Notas de outros canais | — | M | P2 | Pesquisado | — |
| PR-017 | Chord stamp | "Chord Stamp" | Acordes por clique | PR-002 | M | P2 | Parcialmente compatível: 10 formas (maior, menor, dim, aum, sus2, sus4, maj7, m7, 7, power) colocadas pelo clique do Draw; faltam acordes automáticos e escalas como stamp | `chord_stamp_and_tools_menu_edit_the_score`, `piano_roll_tools_...` |
| PR-018 | Snap to scale / realce de escala | "Snap to scale", "Root note"; linhas da escala mais claras | — | — | M | P2 | Parcialmente compatível: 12 escalas e tônica, realce das linhas, notas novas encaixam na escala, ferramenta para encaixar a seleção (empate vai para baixo); notas movidas ainda não encaixam | `piano_roll_tools_flip_strum_limit_stamp_arpeggiate_and_snap` |
| PR-019 | Arpeggiate, Strum, Flam, Limit, Flip, Randomize, Scale levels, Articulate | [Manual: pianoroll_menu, pianoroll_strum, pianoroll_flip, pianoroll_limit] | Ferramentas | PR-001 | L | P2 | Parcialmente compatível: Flip (H/V), Strum (tempo, velocity, preserve end), Limit (por oitavas), Arpeggiate (H-PR-3) no menu Tools com presets próprios; faltam diálogos com parâmetros, Flam, Randomize, Scale levels, Articulate | `piano_roll_tools_...`, `chord_stamp_and_tools_menu_edit_the_score` |
| PR-020 | Copy/Cut/Paste | Ctrl+C/X/V; "Notes are pasted starting from the leftmost bar that is visible" | Clipboard | — | S | P1 | Testado (a posição dentro do compasso é mantida) | `midi_export_import_and_note_clipboard` |
| PR-021 | Delete space, Insert space | [Manual: pianoroll_menu] | — | — | S | P2 | Pesquisado | — |
| PR-022 | Grupos de notas (Shift+G) | "Group notes" | — | — | M | P3 | Pesquisado | — |
| PR-023 | Gravação MIDI | [Manual: recording] | — | MIDI | L | P2 | Pesquisado | — |
| PR-024 | Import/Export MIDI | [Manual: pianoroll_menu "File"]: "Importing MIDI from the Piano roll only adds Notes" | Arquivos .mid (SMF tipo 0/1, running status) | FILE | M | P2 | Testado (botões no Piano roll; reescala PPQ) | `midi_export_import_and_note_clipboard`, `smf::tests::*` |
| PR-025 | Fórmula de compasso por pattern | "Time Signatures" | Só visual | GLB-007 | M | P3 | Pesquisado | — |
| PR-026 | Riff machine, Chord progression | [Manual: pianoroll_riff, pianoroll_chordprogression] | Geradores | — | L | P3 | Pesquisado | — |
| PR-027 | Playhead no Piano roll | — | Posição no modo Pattern | AE | S | P1 | Implementado | — |

## 7. Playlist (PL)

| ID | Nome | Referência | Comportamento verificado | Dep. | Cx | P | Estado | Testes |
|---|---|---|---|---|---|---|---|---|
| PL-001 | Faixas multipropósito | [Manual: playlist] "multi-purpose 'Clip Tracks'"; não são ligadas ao mixer | — | — | S | P0 | Testado | `playlist_clips_share_pattern_source` |
| PL-002 | Pattern clips | Clip = "window over the original data" | Janela sobre os dados do pattern | jet-seq | S | P0 | Testado | `song_mode_places_clip_windows_and_skips_muted_tracks`, `playlist_song_mode_plays_clips...` |
| PL-003 | Audio clips | [Manual: playlist_audioclip] | Canal "Audio Clip" no rack; 100% de volume; ganho, fades, crossfades e stretch | SMP | L | P1 | Parcialmente compatível: toca a partir do offset, o fim da clip corta com declick, começa no meio da clip quando o play parte de dentro dela, waveform real; faltam ganho/fades/crossfade/stretch e o slip edit | `audio_clips_play_their_sample_at_the_recorded_level`, `starting_inside_an_audio_clip_plays_from_that_point` |
| PL-004 | Automation clips | [Manual: playlist_automationclip] | Pontos de controle, 10 tipos de curva, LFO; o clip se move pela barra de título | AUT | L | P1 | Parcialmente compatível: criar, desenhar envelope, adicionar (botão direito), mover (arrastar), apagar (Alt+clique ou menu) e tipo de curva por ponto; faltam tensão arrastável, LFO, step/slide | `knob_menu_creates_an_automation_clip_whose_envelope_takes_points` |
| PL-005 | Draw/Paint/Delete/Mute/Slip/Slice/Select/Zoom/Play | Ferramentas P, B, D, T, S, C, E, Shift+Z, Y | Draw: clique adiciona, arrastar move, botão direito apaga | — | L | P1 | Parcialmente compatível: comportamento de Draw (adicionar, mover com snap de 1 batida e Alt sem snap, redimensionar pela borda, apagar com botão direito, Ctrl+clique muta, H-PL-1); faltam as demais ferramentas e a seleção múltipla | `playlist_clips_move_and_delete_with_the_mouse`, `clicking_an_empty_placeholder_row...` |
| PL-006 | Mute/solo de faixa | "Track Mute/Solo" | Mute de faixa vale para seus clips; Ctrl+clique ou botão direito faz solo | jet-seq | S | P0 | Testado (LED da faixa na UI) | `playlist_song_mode_plays_clips...`, `clip_mute_and_track_solo_change_the_song` |
| PL-007 | Mute de clip | Ferramenta Mute | Independente do mute da faixa | — | S | P1 | Testado (Ctrl+clique) | `clip_mute_and_track_solo_change_the_song` |
| PL-008 | Arranjos múltiplos | "Arrangements ... share the Channel Rack and Mixer" | Clone, rename, delete, merge | jet-model | M | P2 | Parcialmente compatível: menu no título (trocar, Add one, Clone, Delete); rename só por comando; falta Merge | `arrangements_share_channels_and_play_their_own_clips`, `arrangement_menu_adds_arrangements_and_alt_t_adds_a_marker` |
| PL-009 | Time markers e song loop | "Time Markers", "Song Loop" | Alt+T; tipos | — | M | P2 | Parcialmente compatível: Alt+T adiciona no playhead, clique salta, botão direito apaga; faltam os tipos (Loop, Start, Time signature...) | `arrangement_menu_adds_arrangements_and_alt_t_adds_a_marker` |
| PL-010 | Seleção de timeline | Ctrl+arrastar no ruler | Loop da seleção | — | M | P1 | Pesquisado | — |
| PL-011 | Zoom e scroll | Ctrl+roda, Page Up/Down, Shift+1..5 | — | — | S | P1 | Parcialmente compatível | — |
| PL-012 | Picker panel | "Picker Panel" | Escolher, renomear e colorir patterns | — | S | P1 | Implementado (seleção) | — |
| PL-013 | Grupos de faixas, track mode | "Group with above track", "Track mode" | — | — | M | P3 | Pesquisado | — |
| PL-014 | Performance mode | [Manual: playlist_performance] | Disparo ao vivo | — | L | P3 | Pesquisado | — |
| PL-015 | Duplicar (Ctrl+B), merge de pattern clips, make unique | [Manual: playlist] | — | — | M | P2 | Pesquisado | — |
| PL-016 | Arrastar arquivos para a Playlist (cria audio clip) | [Manual: chansettings_sampler] "If you drop the sample on the Playlist you will create an Audio Clip" | Do Browser ou do sistema; linha e batida sob o mouse | PL-003 | M | P1 | Testado | `dropping_a_browser_file_on_the_playlist_creates_an_audio_clip` |
| PL-017 | Playhead | "Play Position Marker"; clique no ruler reposiciona | Arrastável | — | S | P0 | Parcialmente compatível: clique ou arraste no ruler reposiciona (com snap de batida); sempre visível no modo Song | — |

## 8. Mixer (MX)

| ID | Nome | Referência | Comportamento verificado | Dep. | Cx | P | Estado | Testes |
|---|---|---|---|---|---|---|---|---|
| MX-001 | 500 inserts + Master + Current | [Manual: mixer] | Canais novos vão ao Master | — | S | P0 | Testado (inserts materializados sob demanda; Current sem uso) | `mixer_tracks_default_lazily_and_reject_cycles` |
| MX-002 | Fader pós-FX | "The level fader is applied after the effects" | — | jet-dsp | S | P0 | Parcialmente compatível (H-MIX-1) | `mixer_fader_unity_and_top`, `mixer_routing_faders...` |
| MX-003 | Pan do mixer | "mixing one side into the other, so a 100% pan is the mono-sum" | — | — | S | P0 | Implementado (soma cruzada; sem escalas medidas) | — |
| MX-004 | Stereo separation | "Turn the knob right to decrease ... left to increase" | Mid/side | — | S | P2 | Testado (controle no inspector) | `mixer_phase_swap_and_stereo_separation_change_the_output` |
| MX-005 | Invert phase, Swap L/R | Ícones da faixa | — | — | S | P2 | Testado (botões no inspector) | `mixer_phase_swap_and_stereo_separation_change_the_output` |
| MX-006 | Mute/solo de faixa | "Left-click to mute ... Right-Click to solo" | Solo muta os outros inserts | — | S | P0 | Testado | `mixer_routing_faders...` |
| MX-007 | Sends pós-fader | "Send Tracks", "Post-fader Sends (default)" | Nível de send; send em 0 = só sidechain | — | M | P1 | Testado (sem ciclos) | `sends_route_post_fader...`, `mixer_routing_faders...` |
| MX-008 | Route to Master desligável | "deselect its send to master switch" | — | — | S | P1 | Testado | `sends_route_post_fader_and_master_routing_can_be_disabled` |
| MX-009 | Sidechain | "Sidechain Sends" | Entrada de sidechain nos plugins | FX | M | P2 | Especificado (send nível 0 guardado; os efeitos ainda não leem sidechain) | — |
| MX-010 | 10 FX slots, mix por slot, enable, mover, master FX switch | "Each Track has 10 effects slots" | — | FX | M | P0 | Testado | `effects_keep_their_state_across_plan_swaps`, `mixer_routing_faders...` |
| MX-011 | Peak meters (laranja acima de 0 dB) | [Manual: mixer_levelsandmixing] | — | AE | S | P0 | Implementado (picos reais por faixa) | `meters_report_master_peaks` |
| MX-012 | Faixas de inserts não clipam; o Master pode clipar | "it is practically IMPOSSIBLE to clip insert Mixer Tracks" | Float interno; hard clip só na saída do dispositivo | AE | S | P0 | Testado (o render em float mantém overs) | — |
| MX-013 | EQ paramétrico integrado de 3 bandas | "Mixer Track Properties - The Parametric Equalizer" | — | FX | S | P1 | Parcialmente compatível: EQ como efeito de slot | `peak_filter_boosts_its_center_by_its_gain` |
| MX-014 | Track delay / PDC | "Track delay", PDC | Compensação de latência | AE | L | P2 | Especificado (`Effect::latency`; nenhum efeito atual tem latência) | — |
| MX-015 | Entrada/saída de áudio por faixa | "Audio Input & Output" | Requer ASIO | REC | L | P2 | Pesquisado | — |
| MX-016 | Docks, layouts, renomear/colorir faixas | — | — | — | S | P3 | Renomear: Testado no engine; o resto: Pesquisado | — |
| MX-017 | Ctrl+L / Ctrl+Shift+L (rotear canais selecionados) | "Route selected channels to this track" | — | CR-011 | S | P1 | Parcialmente compatível (Ctrl+L no canal selecionado) | — |

## 9. Motor de áudio (AE)

| ID | Nome | Referência | Comportamento | Dep. | Cx | P | Estado | Testes |
|---|---|---|---|---|---|---|---|---|
| AE-001 | Saída em tempo real | [Manual: envsettings_audio] | Dispositivo padrão (WASAPI via cpal 0.18.2) | cpal | M | P0 | Testado (medido: 48 kHz, buffer de 480, 0 overruns) | `jet-cli play` (manual) |
| AE-002 | Callback sem alocar e sem lock | [ARQ §1.5] | — | — | M | P0 | Testado | `processing_a_busy_project_does_not_allocate` (o detector foi conferido com um `vec!` inserido de propósito) |
| AE-003 | Sequenciamento exato por amostra | — | O evento soa no 1º frame com tick ≥ T, igual à fórmula do transporte | jet-seq | M | P0 | Testado | `steps_start_on_their_exact_frame`, `rendering_is_independent_of_block_size` |
| AE-004 | Sample rate e buffer configuráveis | "Sample Rate", "Buffer Length" | Pedido recusado volta ao padrão do dispositivo | AE-001 | S | P1 | Testado no hardware (`jet-cli play --rate/--buffer/--device`: 22 050 Hz/256 aceitos; 1 MHz cai para 48 kHz com aviso; dispositivo por nome) e na UI | `f10_opens_audio_settings...` |
| AE-005 | Render offline idêntico ao playback | [ARQ M2] | — | — | M | P0 | Testado | `offline_render_leaves_the_remainder_and_matches_live_playback` |
| AE-006 | Export WAV 16/24/32 float | [Manual: fformats_save_export] | Modo Song/Pattern, Tail leave/cut/wrap, taxa 22–192 kHz | AE-005 | M | P0 | Testado | `wav_round_trips...`, testes de `jet-engine/tests/audio.rs` |
| AE-007 | Export MP3/OGG/FLAC | idem | Licenças do encoder | — | M | P2 | Pesquisado (requer encoder de licença compatível; LAME é LGPL e é proibido pela ADR) | — |
| AE-008 | Export MIDI | idem | Uma faixa por canal, tempo e nomes | FILE | M | P2 | Testado (File > Export > MIDI file; steps viram notas de 1 step) | `midi_export_import_and_note_clipboard` |
| AE-009 | Split mixer tracks | idem | Uma faixa por arquivo | AE-006 | M | P2 | Pesquisado | — |
| AE-010 | Dithering | idem | Shaped dither para 16 bits | AE-006 | S | P3 | Pesquisado | — |
| AE-011 | Medidor de carga/underruns | [W], envsettings_audio "Underruns" | — | AE-001 | S | P1 | Testado no dispositivo (overruns = callback mais longo que o buffer) | `jet-cli play` |
| AE-012 | Multithread de geradores e mixer | "Multithreaded ... processing" | — | — | L | P2 | Pesquisado (thread de áudio única) | — |
| AE-013 | Preview de sample | [Manual: browser] | — | BR | S | P1 | Implementado (H-BR-1) | — |
| AE-014 | Notas ao vivo (teclado, MIDI) | — | — | — | S | P1 | Testado | `live_notes_sound_until_note_off` |
| AE-015 | Pânico | "stop all notes" | — | — | S | P1 | Implementado | — |

## 10. Efeitos (FX), todos com algoritmos próprios e citados

| ID | Nome | Equivalente FL (categoria) | Algoritmo | Cx | P | Estado | Testes |
|---|---|---|---|---|---|---|---|
| FX-001 | EQ paramétrico | Fruity Parametric EQ 2 (categoria) | Biquads RBJ | S | P0 | Testado | `peak_filter_boosts_its_center_by_its_gain`, `lowpass_passes_dc_and_cuts_highs` |
| FX-002 | Delay | Fruity Delay 3 | Linha com feedback, ping-pong, tempo em batidas | S | P1 | Testado | `delay_repeats_an_impulse_after_its_time` |
| FX-003 | Reverb | Fruity Reeverb 2 | Schroeder–Moorer, afinações Freeverb (domínio público) | M | P1 | Testado | `reverb_produces_a_decaying_tail` |
| FX-004 | Compressor | Fruity Compressor | Feed-forward de pico, joelho duro | S | P1 | Testado | `compressor_static_curve_follows_ratio` |
| FX-005 | Limiter | Fruity Limiter | Ataque instantâneo, release exponencial | S | P1 | Testado | `limiter_never_exceeds_its_ceiling` |
| FX-006 | Distortion | Fruity Blood Overdrive / WaveShaper | Waveshaper tanh normalizado | S | P2 | Testado (via mixer) | `mixer_routing_faders...` |
| FX-007 | Chorus | Fruity Chorus | Delay modulado em quadratura | S | P2 | Implementado | — |
| FX-008 | Noise gate | Fruity Gate | Limiar e release | S | P2 | Testado | `gate_silences_signal_below_threshold` |
| FX-009 | Flanger, Phaser | Fruity Flanger / Phaser | Flanger: delay curto (0,5–5 ms) modulado com feedback; Phaser: 6 all-pass de 1ª ordem varridos de 200 Hz a ~5 kHz | S | P2 | Testado | `flanger_and_phaser_change_the_signal_and_stay_finite` |
| FX-010 | Stereo shaper / enhancer | Fruity Stereo Shaper | — | S | P2 | Pesquisado | — |
| FX-011 | Filtro | Fruity Filter / Love Philter | Biquad RBJ LP/HP/BP com ressonância | S | P2 | Testado | `filter_effect_cuts_highs_in_low_pass_mode` |
| FX-012 | Pitch shifter | Pitcher / Pitch Shifter | — | L | P3 | Pesquisado | — |
| FX-013 | Analisadores (espectro, osciloscópio) | Wave Candy | — | M | P2 | Pesquisado | — |
| FX-014 | Vocoder | Vocodex | — | L | P3 | Pesquisado | — |

## 11. Plugins externos (PLG)

| ID | Nome | Referência | Comportamento | Cx | P | Estado |
|---|---|---|---|---|---|---|
| PLG-001 | Hospedar VST3 | [Manual: basics_externalplugins] | Scan, verify, plugin manager, favoritos | XL | P2 | Especificado. O SDK VST3 está sob **licença MIT desde a versão 3.8 (out/2025)** (comunicado da Steinberg de 29/10/2025; fórum "VST 3.8.0 SDK Released"), então não há bloqueio de licença. Exige um crate FFI isolado (o único lugar onde a ADR permite `unsafe`) e a escolha entre bindings existentes e geração própria. O SDK ASIO continua GPLv3 ou proprietário, logo ASIO segue fora (WASAPI/cpal). |
| PLG-002 | Hospedar CLAP | (fora do FL; padrão aberto MIT) | `clack-host` em crate FFI isolado | L | P2 | Especificado (FINAL_ARCHITECTURE §9) |
| PLG-003 | Wrapper: parâmetros, presets, estado, latência, crash isolation | [Manual: wrapper] | — | XL | P2 | Pesquisado |
| PLG-004 | AU (macOS) | — | — | L | P3 | Pesquisado |

## 12. Browser e arquivos (BR, FILE)

| ID | Nome | Referência | Comportamento | Cx | P | Estado | Testes |
|---|---|---|---|---|---|---|---|
| BR-001 | Pastas e navegação | [Manual: browser] | Árvore, adicionar pasta (engrenagem ou arrastar) | S | P0 | Implementado | `root_frame_registers_the_main_surfaces` |
| BR-002 | Preview de áudio ao clicar | [Manual: browser] | — | S | P1 | Implementado | — |
| BR-003 | Arrastar sample para o Channel Rack | — | — | S | P0 | Implementado | — |
| BR-004 | Busca | "Search" | — | S | P1 | Pesquisado |
| BR-005 | Abas PROJECT/PLUGINS/PRESETS/SOUNDS/STARRED | [W] | SOUNDS é FL Cloud | M | P2 | Pesquisado (abas desabilitadas, com motivo); SOUNDS: Bloqueado (on-line) |
| BR-006 | Favoritos e histórico | — | — | S | P2 | Pesquisado |
| FILE-001 | Formato próprio .jetproj (JSON versionado, save atômico) | ARQ §6 | — | S | P0 | Testado | `save_load_and_replace_existing_file`, `save_and_load_keep_samples_and_render_identically` |
| FILE-002 | Caminhos relativos de samples | [Manual: songsettings_settings "Project Data Folder"] | Relativo ao projeto, mais hint absoluto | S | P0 | Testado | `save_and_load_keep_samples...` |
| FILE-003 | Autosave e backups | [Manual: envsettings_files]: "Regularly (every 5 minutes)", adiado enquanto toca; backup a cada save manual; "Revert to last autosave" | — | M | P1 | Parcialmente compatível: modo Regularly fixo em `Documentos\JetStudio Projects\Backup`, revert no menu File; faltam os outros modos e o versionamento sequencial. Teste: `autosave_backs_up_changes_and_revert_restores_them` |
| FILE-004 | Templates | [Manual: menu_file] | — | S | P2 | Pesquisado |
| FILE-005 | Zip com samples | [Manual: fformats_project] | — | M | P2 | Pesquisado |
| FILE-006 | Ler .flp | [Manual: fformats_project] esquema não publicado | — | — | — | Bloqueado (sem especificação pública; fora de escopo pela ADR) |

## 13. Automação, MIDI e gravação (AUT, MIDI, REC)

| ID | Nome | Referência | Cx | P | Estado |
|---|---|---|---|---|---|
| AUT-001 | Automation clips (pontos, tensão, curvas, LFO, clone, decimate, flip) | [Manual: playlist_automationclip] | L | P1 | Parcialmente compatível: alvos volume/pan/pitch de canal, volume/pan de faixa e mix de slot de FX; 10 curvas (H-AUT-2); avaliação por bloco no renderizador, sem alocação; o último valor se mantém depois do clip (H-AUT-1); só no modo Song; faltam LFO, gravação, tempo e parâmetros internos de FX. Testes: `automation_clips_drive_parameters_in_song_mode`, `song_mode_automation_does_not_allocate`, `lanes_hold_the_last_value...` |
| AUT-002 | Gravação de automação | [Manual: recording_automation] | M | P2 | Pesquisado |
| AUT-003 | Event editor e eventos em patterns | [Manual: automation_eventeditor] | L | P2 | Pesquisado |
| AUT-004 | Link to controller / MIDI learn | [Manual: automation_linking] | M | P2 | Pesquisado |
| AUT-005 | Internal controllers | [Manual: automation_internal] | M | P3 | Pesquisado |
| MIDI-001 | Entrada MIDI (midir), omni, typing keyboard | [Manual: envsettings_midi]; [menu_options] "Typing to piano keyboard (Ctrl+T)" | M | P1 | Parcialmente compatível: todas as portas abertas, notas no canal selecionado por fila direta ao áudio; teclado de digitação (H-KB-1, Alt+NumPad 2–6); faltam escolha de dispositivo, CC/controles, gravação. Testes: `parses_note_on_off_and_running_zero_velocity`, `typing_keyboard_plays_notes_and_blocks_letter_shortcuts` |
| MIDI-002 | MIDI clock sync | idem | M | P3 | Pesquisado |
| REC-001 | Gravação de áudio (arm, pickup, takes, monitoramento) | [Manual: recording_audio]: "Arm record mode and press Play"; Audio Clip automática "only ... in Song mode" na "first track with enough room"; compensação de latência = buffer | L | P2 | Parcialmente compatível: R/botão armam (a entrada padrão só abre armada), Play grava, Stop salva `Recordings/Recording N.wav` (32-bit float) e cria a clip; compensação = buffer de saída + entrada; alinhamento ao bloco de UI (H-REC-1); faltam escolha de entrada por mixer track, pickup, monitoramento, takes/loop. Testes: `r_arms_recording_and_a_finished_take_becomes_an_audio_clip`, `audio_clips_without_a_track_go_to_the_first_track_with_room`. A captura cpal não foi exercitada no hardware (não liguei o microfone sem pedir). |
| REC-002 | Gravação de notas no Piano roll | [Manual: recording_scores] | M | P2 | Pesquisado |
| REC-003 | Comping de takes | [Manual: recording_audio "Audition takes"] | L | P3 | Pesquisado |

## 14. Como estender este inventário

1. Leia a página do manual e registre a página e uma frase-chave entre aspas.
2. Dê um ID novo no prefixo do sistema; nunca reutilize um ID.
3. Ao implementar, cite o nome do teste. Sem teste, o estado máximo é "Implementado".
4. Quando o comportamento vier de uma escolha nossa, crie uma linha H-xxx na §2.
5. "Validado" só com comparação documentada em `docs/UI_VISUAL_AUDIT.md` ou em um relatório de medição de áudio.
