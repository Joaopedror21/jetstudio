# M1 — Relatório de progresso para revisão técnica

Última revisão: 23/10/2026 · Referência-alvo: FL Studio 2026, Windows x64 (build exato pendente de captura) · Branch: `codex/M1-interface`.

## Estado recuperado

O repositório foi iniciado a partir dos 13 documentos de pesquisa e arquitetura já existentes. A implementação atual é uma primeira aplicação desktop executável; **M1 ainda não está concluído** segundo as portas de `IMPLEMENTATION_ROADMAP.md`. Não houve reinício da pesquisa ArtCraft. As decisões vinculantes continuam em `FINAL_ARCHITECTURE.md` e `UI_FIDELITY_STRATEGY.md`.

As três imagens disponíveis estão em `reference/screenshots/` (singular), não em `references/screenshots/`. O [inventário](SCREENSHOT_INVENTORY.md) registra cada imagem, suas dimensões e SHA-256. Nenhuma tem o perfil Windows completo exigido para medir a UI do M1; a primeira mostra macOS, a segunda é um recorte de Loop Starter no Channel Rack e a terceira é um recorte de Piano roll. Todas as cores e dimensões provisórias da implementação continuam marcadas `Unmeasured`.

## O que funciona agora

| Área | Implementação verificada |
|---|---|
| Bootstrap | Workspace Rust 2024, toolchain 1.99.0, camadas separadas, `cargo xtask ci/layers/assets`, regras para código sem `unsafe` e sem `unwrap` de produção. |
| Núcleo | `jet-time` calcula ticks, tempo e conversões; `jet-model` valida Project, canais, patterns e arrangements; `jet-format` salva e reabre `.jetproj` com escrita atômica; `jet-engine` despacha comandos com validação estrita, histórico e transporte silencioso com relógio monotônico. |
| CLI | `jet-cli run`, `inspect` e `commands`; teste ponta a ponta cria canal, step, faixa e clip, salva e reabre o projeto. |
| Aplicação desktop | Janela nativa `JetStudio` em eframe/wgpu, toolbar e menus desenhados por widgets próprios, Browser de pastas locais, Channel Rack, Playlist, Hint Bar e janelas internas com foco, mover, redimensionar, maximizar e ocultar. |
| Controles ligados ao estado | PAT/SONG, Play/Stop/Pause, BPM, seleção e criação de pattern, adicionar canal, mute, step esquerdo liga/direito desliga, adicionar faixa e Pattern Clip, zoom/scroll da Playlist, Browser com seleção/árvore/rolagem e atalhos principais de painéis. |
| Arquivos na UI | FILE › Open, Save e Save As estão ligados ao formato `.jetproj` por diálogos nativos. O round-trip foi testado pela CLI; os diálogos da janela ainda precisam de inspeção manual. |

O transporte do M1 avança a posição musical e **não produz áudio**. Record e recursos sem engine aparecem desabilitados.

## Verificações executadas

Com `RUSTUP_TOOLCHAIN=stable` (instalação local de Rust 1.99.0), `CARGO_TARGET_DIR=target/agent-codex` e `CARGO_NET_OFFLINE=true`:

| Comando | Resultado |
|---|---|
| `cargo check --offline --workspace` | Passou. Foram corrigidas chamadas da UI para a API do eframe 0.36.2 (`App::ui` e root `Ui`) e erros de borrow/input. |
| `cargo test --offline --workspace` | Passou: 27 testes, nenhum falhou. Inclui eventos egui para canal, steps, faixa/clip, transporte, BPM e F5; round-trip CLI; validação de modelo, tempo e workspace. |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | Passou. |
| `cargo xtask ci` | Passou os gates disponíveis: fmt, Clippy, testes, camadas e atribuição de assets. |
| `cargo build --offline -p jetstudio` | Passou; criou `target/agent-codex/debug/jetstudio.exe`. |
| Executar o `.exe` | Processo abriu janela `JetStudio`, com `MainWindowHandle` não nulo e `Responding=True` no Windows. |

A inspeção visual da **janela do JetStudio** não foi concluída: o helper nativo da skill Computer Use retornou `native pipe is unavailable` em três tentativas (incluindo reinicialização). Não foi gerada captura do nosso app nem comparação externa. Os testes egui exercitam os eventos, mas não substituem a revisão visual.

## Lacunas relevantes para a auditoria

- **M1.0/M1.2:** `xtask parity`, `xtask shot`, canal `ui.inspect` e harness offscreen ainda faltam. As crates `egui_kittest` e `proptest` da arquitetura aprovada não estão no cache Cargo offline; foram usados testes determinísticos com `egui::Context::run_ui`, sem declarar equivalência com o harness planejado.
- **R0/M1.8:** faltam capturas calibradas da referência no Windows, sidecars, medidas, specs e relatório de comparação. `references/comparisons/` foi preparado localmente e ignorado pelo Git, sem imagens ou resultados.
- **Toolbar e arquivos:** ordem do preset, menus completos, valor digitado por outros gestos de LCD e confirmação antes de descartar um projeto alterado estão pendentes. Abrir/salvar via diálogo precisa de QA na janela.
- **Channel Rack:** controles de volume/pan, reordenação e renomeação na UI, comprimento de pattern e pintura por arraste estão pendentes. O modelo e parte dos comandos já existem.
- **Playlist:** há inserção de Pattern Clip e navegação, mas mover/redimensionar/apagar clips pela UI, snap local, seleção múltipla e virtualização/performance do cenário M1.6 estão pendentes.
- **Browser:** adiciona pastas locais, expande e seleciona itens; ainda não cria canal Sampler ao arrastar arquivo, não persiste as pastas escolhidas e não faz preview de áudio.
- **Comandos/histórico:** o registro cobre as ações exercitadas, mas faltam IDs de `CODEX_FIRST_TASK.md` (como `view.*` e `snap.global.set`), coalescência de gestos e orçamento de histórico por bytes. O comportamento fino de `toggle_last` ainda exige observação dirigida da referência.
- **Escopo posterior:** áudio, Mixer, Piano roll, gravação, plugins e `.flp` ficam fora do M1 aprovado.

Não há evidência para declarar fidelidade 1:1, paridade externa, M1 completo ou revisão do Claude aprovada.

## Como executar no Windows (PowerShell)

```powershell
cd C:\Users\joaop\Documents\jetstudio
$env:RUSTUP_TOOLCHAIN = 'stable'
$env:CARGO_TARGET_DIR = 'target/agent-codex'
$env:CARGO_NET_OFFLINE = 'true'
cargo run --offline -p jetstudio
```

O override `stable` usa o Rust 1.99.0 já instalado; o nome de toolchain `1.99.0` fixado no projeto ainda não está registrado no rustup local. Para os gates: `cargo xtask ci` com as mesmas variáveis de ambiente.

## Próximo incremento recomendado

Concluir **M1.2**: expor `ui.inspect` a partir do `LayoutProbe`, produzir uma captura determinística do próprio app e revisar a geometria da janela. Depois implementar os controles restantes do Channel Rack e o arraste do Browser. Em paralelo, o usuário pode coletar o cenário `shell/startup` num perfil Windows completo para iniciar medidas reais. Uma revisão do Claude está pendente para os contratos de M1.1 e para a UI M1.2, conforme `CODEX_FIRST_TASK.md`.

## Revisões

| Data | Alteração |
|---|---|
| 23/10/2026 | Estado do código, gates, execução, lacunas de fidelidade e próximo incremento. |
