# Tarefas ativas

Última revisão: 23/10/2026 · Alvo: JetStudio M1 / FL Studio 2026 Windows x64.

| Task-id | Agente | Branch | Caminhos reservados | Início | Estado |
|---|---|---|---|---|---|
| M1.0 | Codex bootstrap | `codex/M1-interface` | `Cargo.toml`, `rust-toolchain.toml`, `.cargo/**`, `.gitignore`, `xtask/**`, `AGENTS.md`, `CLAUDE.md`, `ATTRIBUTION.md`, `docs/tasks/**` | 23/10/2026 | Implementado parcialmente; revisão pendente |
| M1.1-model | Codex model | `codex/M1-interface` | `crates/jet-time/**`, `crates/jet-model/**`, `crates/jet-format/**` | 23/10/2026 | Código e testes verdes; revisão pendente |
| M1.1-engine | Codex integração | `codex/M1-interface` | `crates/jet-engine/**`, `apps/jet-cli/**` | 23/10/2026 | Código e testes verdes; revisão pendente |
| M1.2-ui | Codex UI | `codex/M1-interface` | `crates/jet-ui/**`, `apps/jetstudio/**` | 23/10/2026 | Executável funcional; incremento incompleto |
| M1-visual | Claude | `claude/M1-visual-fidelity` | `crates/jet-ui/**`, `assets/fonts/**`, `tools/compare/**`, `docs/UI_VISUAL_AUDIT.md` | 23/10/2026 | Camada visual refeita contra a captura W; 3 ciclos de comparação; revisão do Codex pendente |

| M2-M5-audio | Claude | `claude/M1-visual-fidelity` | `crates/jet-seq/**`, `crates/jet-dsp/**`, `crates/jet-media/**`, `crates/jet-audio-graph/**`, `crates/jet-playback/**`, `crates/jet-engine/**`, `crates/jet-ui/**`, `apps/**`, `docs/FL_STUDIO_COMPLETE_FEATURES.md`, `docs/FEATURE_PARITY_MATRIX.md`, `docs/PROGRESS.md` | 24/10/2026 | Motor de áudio, Piano roll, Mixer e Channel rack funcionais; próxima: Playlist (ver `docs/PROGRESS.md`) |

## Revisões

| Data | Alteração |
|---|---|
| 23/10/2026 | Reservas iniciais do M1 registradas. |
| 23/10/2026 | Reserva da tarefa M1-visual (Claude). |
| 23/10/2026 | Estado recuperado; compilação, testes e lacunas descritos em `docs/MILESTONE_1_REPORT.md`. |
| 24/10/2026 | Reserva M2-M5-audio (Claude): áudio real, inventário e matriz de paridade. |
