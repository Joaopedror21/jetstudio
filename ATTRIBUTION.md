# Atribuições de assets

O JetStudio não copia fontes, ícones ou imagens do FL Studio. A fonte de fallback (glifos ausentes) continua sendo a embutida na dependência `epaint_default_fonts 0.36.2` (via egui). A fonte proporcional é Ubuntu Light; o pacote também inclui Hack Regular, Noto Emoji e emoji-icon-font como alternativas internas. As licenças declaradas pelo pacote são `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0` ([fonte do pacote](https://github.com/emilk/egui/tree/v0.36.2/crates/epaint_default_fonts)). A tipografia do produto permanece `Unmeasured` até a análise das capturas de referência.

## Assets empacotados

| Caminho | Autor | Fonte | Licença |
|---|---|---|---|
| `assets/fonts/BarlowCondensed-Regular.ttf` | Jeremy Tribby (The Barlow Project Authors) | https://github.com/google/fonts/tree/main/ofl/barlowcondensed | SIL OFL 1.1 |
| `assets/fonts/BarlowCondensed-Medium.ttf` | Jeremy Tribby (The Barlow Project Authors) | https://github.com/google/fonts/tree/main/ofl/barlowcondensed | SIL OFL 1.1 |
| `assets/fonts/BarlowCondensed-Light.ttf` | Jeremy Tribby (The Barlow Project Authors) | https://github.com/google/fonts/tree/main/ofl/barlowcondensed | SIL OFL 1.1 |
| `assets/fonts/OFL-Barlow.txt` | The Barlow Project Authors | https://github.com/google/fonts/tree/main/ofl/barlowcondensed | texto da licença SIL OFL 1.1 |

Barlow Condensed foi escolhida por métrica medida (largura/altura de texto) contra a captura de referência; ver `docs/UI_VISUAL_AUDIT.md` §9. Não é a fonte do FL Studio, que não foi identificada.
