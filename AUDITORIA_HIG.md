# Auditoria HIG — Yukkit Launcher

Data: 2026-11-17 · Versão auditada: `0.2.0` (commit `57b95e7`) · Referência: Apple Human Interface Guidelines (cópia local da skill `apple-design`, páginas de junho a outubro de 2026) + guia de Liquid Glass.

## Contexto

| | |
| --- | --- |
| Stack | Tauri 2 (Rust) + HTML/CSS/JS puro em `ui/` (sem framework, sem bundler) |
| Plataforma-alvo | **Windows** (bundle só `nsis`, WebView2, caminhos `C:\…`, fonte Segoe UI). Não há build para macOS/iOS |
| Janela | 1180×760 padrão, mínimo 960×620, redimensionável, barra de título nativa do Windows |
| Telas | Play, Versions, Patch notes, Settings, Logs + elementos globais: sidebar, banner de aviso (`#notice`), toast, `confirm()` de remoção |
| Tese do app | "Aperte Play e jogue" — o botão Play e o estado de prontidão (Java / Minecraft / Toolkit) são o conteúdo principal |

**Importante:** a HIG é escrita para plataformas Apple. Como o app roda no Windows, aplico os *princípios* (hierarquia, contraste, deferência, acessibilidade, materiais) e traduzo as convenções de macOS para os equivalentes do Windows. Onde uma regra não se aplica, está marcado como **N/A com motivo** (ver seção final).

Medições: contraste calculado pela fórmula WCAG 2.x a partir dos hex do `styles.css` (não estimado por screenshot). Tamanhos em px CSS ≈ pt/DIP no WebView2 a 100%.

---

## Placar (antes)

| Área | Nota (0–10) | Resumo |
| --- | :-: | --- |
| Hierarquia visual | 6 | Play bem destacado, mas fundo animado e hero serifado competem com o conteúdo |
| Tipografia | 5 | Escala sem tokens, serif de display que nem existe no Windows, 11 px em vários lugares |
| Espaçamento (grade 8 pt) | 4 | Valores soltos: 7, 9, 13, 26, 34, 44 px |
| Cores semânticas + light/dark | 2 | Só dark, `color-scheme: dark` fixo, cores hard-coded espalhadas |
| Contraste WCAG AA | 5 | 4 falhas em texto (até 2,36:1), bordas de controles a 1,4:1 |
| Áreas de toque / clique | 6 | Links de 18 px de altura, abaixo do mínimo de 20 |
| Navegação | 6 | Sidebar correta, mas sem `aria-current`, sem atalhos, ações no rodapé |
| Materiais / Liquid Glass | 3 | Glass não usado onde deveria (sidebar) e "glow" decorativo no conteúdo |
| Ícones | 2 | Nenhum sistema de ícones; "✕" e "▾" como caracteres de texto |
| Estados (hover/pressed/disabled/focus) | 4 | Foco removido em campos, toggle inacessível por teclado |
| Animação / Reduce Motion | 3 | 4 animações infinitas, nenhuma resposta a `prefers-reduced-motion` |
| Acessibilidade (leitor de tela, teclado) | 3 | Controles sem rótulo, segmentados sem papel, toast e progresso mudos |
| **Total** | **49 / 120** | Classificação da skill: **Critical issues** |

---

## Crítico

### C1 — Toggle "Install new builds automatically" não é alcançável pelo teclado
- **O quê:** `.switch input { display: none; }` (`ui/styles.css:282`) tira o checkbox da ordem de Tab e da árvore de acessibilidade. Quem usa só teclado ou leitor de tela não consegue ligar/desligar.
- **Por quê:** `accessibility.md` — o app precisa funcionar com teclado e com tecnologias assistivas; regra desktop da skill: "Keyboard-only use works on desktop".
- **Correção:** esconder visualmente (`opacity:0; position:absolute`) mantendo o input, adicionar `role="switch"` e anel de foco visível no trilho.

### C2 — Foco invisível nos campos e no seletor de versão
- **O quê:** `.field { outline: none }` e `.select select:focus { outline: none }` (`styles.css:270, 128`). O único sinal é a borda passar para azul em 1 px, contra uma borda de repouso de 1,44:1 — quase imperceptível. Botões não têm estilo `:focus-visible` próprio (dependem do contorno padrão do WebView, inconsistente com o resto).
- **Por quê:** `focus-and-selection.md` / regra desktop: foco de teclado sempre visível.
- **Correção:** anel `:focus-visible` único para todo controle (2 px accent + 2 px offset), com contraste ≥ 3:1 contra o fundo.

### C3 — Texto do botão Play "Playing" a 2,36:1
- **O quê:** branco sobre `#2fbf7f` (estado `.running`, `styles.css:150`). Texto de 17 px bold precisa de 3:1; o subtítulo de 11 px precisa de 4,5:1.
- **Por quê:** `accessibility.md › Color and effects` — tabela WCAG AA.
- **Correção:** verde de estado `#1a7f37` (branco 5,08:1).

### C4 — Subtítulo do Play a 3,41:1 e gradiente a 3,15:1
- **O quê:** `.play-sub` é branco a 80% sobre `#006efe` (3,41:1, 11 px). O topo do gradiente `#3a91ff` dá 3,15:1 até para o rótulo principal.
- **Por quê:** mesma tabela — texto < 18 pt precisa de 4,5:1.
- **Correção:** fundo sólido accent `#0066e0` (branco 5,28:1), subtítulo 100% opaco a 12 px.

### C5 — Nenhuma resposta a Reduce Motion
- **O quê:** `.g1/.g2/.g3` (`drift` 22–28 s infinito), `.pulse` infinito, `.bar-fill.indeterminate` e `rise` em cada troca de página. Não existe `@media (prefers-reduced-motion)`.
- **Por quê:** `motion.md` / `accessibility.md`: movimento é opcional e precisa respeitar a preferência do sistema (no Windows: *Efeitos de animação* desligado → `prefers-reduced-motion: reduce`).
- **Correção:** remover o movimento decorativo; manter só feedback funcional, que vira transição instantânea ou fade com Reduce Motion.

### C6 — Controles sem nome acessível
- **O quê:** slider de memória, campos de nome/Java/argumentos e segmentados não têm `<label for>` nem `aria-label`; o texto ao lado é só `div`. Segmentados não têm `role="radiogroup"`/`aria-checked`, então o leitor não diz qual opção está ativa. Botão "✕" depende só de `title`.
- **Por quê:** `accessibility.md`, `voiceover.md`: "every icon-only control has a text label for screen readers".
- **Correção:** `<label>` associado a cada controle; segmentados como grupo de rádio com setas do teclado; `aria-label="Dismiss"` no fechar.

---

## Alto

### A1 — Só existe aparência escura
- **O quê:** `color-scheme: dark` fixo (`styles.css:17`), todas as cores calibradas para fundo `#0b0c10`. Com o Windows em modo claro, o app ignora a preferência.
- **Por quê:** `dark-mode.md › Best practices`: "people … generally expect all apps and games to respect their preference" e "Ensure that your app looks good in both appearance modes." A exceção ("only a dark appearance … for immersive media viewing") não cobre um launcher, que é uma tela de configuração/estado, não de mídia.
- **Correção:** tokens semânticos com variantes clara e escura via `prefers-color-scheme`, sem seletor de tema dentro do app.

### A2 — Cores hard-coded fora de tokens
- **O quê:** ~45 valores literais (`rgba(255,255,255,.06)`, `#15161c`, `#2a2c36`, `#a9ccff`, `#ffd59a`, `#6b6d79`…) espalhados no CSS e 3 gradientes inline em `app.js:166-170`.
- **Por quê:** `color.md`: "Avoid hard-coding system color values"; `dark-mode.md`: "Avoid using hard-coded color values or colors that don't adapt."
- **Correção:** arquivo central `ui/tokens.css`; o resto do CSS só lê variáveis.

### A3 — Visual decorativo competindo com o conteúdo (incluindo gradiente roxo)
- **O quê:** três "glows" azul/roxo (`#2a1e6e`) com blur de 90 px animados, fitas SVG, avatar com gradiente azul→`#6b4dff`, barra de progresso azul→`#7a6bff`, cartões de notícia com gradientes radiais roxos/ciano gerados.
- **Por quê:** `branding.md`: a marca "defers to content"; `liquid-glass.md › Color on glass`: "Apply your app's accent color judiciously"; princípio *Simplicity* de `design-principles.md`. Também é exatamente o "gradiente roxo" que você vetou.
- **Correção:** fundo liso semântico; uma única cor de marca (o azul do logo) gasta só no Play, seleção e progresso.

### A4 — Material errado nas camadas
- **O quê:** a sidebar (camada funcional) é um painel opaco a 92% sem `backdrop-filter`; já os elementos de conteúdo (cartões, progresso, campos) usam fundos translúcidos `rgba(…,.78–.9)` sobre o glow — profundidade "de vidro" no lugar errado.
- **Por quê:** `liquid-glass.md › The two layers`: glass só na camada funcional; "Glass on app backgrounds, cards, list rows, or content containers is a defect." `cross-platform.md`: "Prefer the platform's real materials and window chrome over a web imitation."
- **Correção:** no Windows 11, o material de sistema equivalente é **Mica** (Tauri `windowEffects`), aplicado à janela e visível na sidebar; conteúdo em superfícies opacas. Fallback opaco no Windows 10 e com transparência desligada.

### A5 — Contraste de texto auxiliar e de bordas
- **O quê:** `.fineprint #6b6d79` = 3,81:1 a 12 px (precisa 4,5). Bordas de campos, botões e segmentados (`--line-strong`, 14% branco) = 1,44:1; trilho do switch desligado `#2a2c36` = 1,41:1 (componentes não-texto precisam de 3:1, WCAG 1.4.11).
- **Por quê:** `accessibility.md › Color and effects`; Increase Contrast precisa de resposta (`prefers-contrast: more`).
- **Correção:** cores secundárias ≥ 4,5:1 nos dois modos; contorno de controles ≥ 3:1 ou preenchimento distinguível.

### A6 — Botão disabled ilegível
- **O quê:** `.play-btn:disabled { opacity: .55 }` → texto 3,05:1, e é o estado inicial do app ("Please wait").
- **Correção:** estado desabilitado com fundo neutro e rótulo secundário (≥ 4,5:1), não só opacidade.

### A7 — Sem sistema de ícones
- **O quê:** sidebar só com texto, "✕" e "▾" como glifos de fonte, "↗" em link. Sem pesos consistentes.
- **Por quê:** `sidebars.md`: "Consider using familiar symbols to represent items in the sidebar"; `icons.md`: um só idioma visual, peso combinando com o texto.
- **Correção:** um conjunto de ícones vetoriais inline (SVG), traço 1,5 px em 16/20 px — ver nota sobre SF Symbols em N/A.

### A8 — Tipografia sem escala e com fonte de display inexistente no Windows
- **O quê:** 14 tamanhos diferentes (11, 12, 13, 14, 16, 17, 22, 25, 38, 76 px…). `--serif` começa por "Iowan Old Style", que não vem no Windows → cai em Palatino Linotype. Tamanhos em `px`, então não escalam com o zoom de texto.
- **Por quê:** `typography.md`: escala clara, peso e tamanho carregando a hierarquia; desktop padrão 13 pt, mínimo 10 pt; texto que escala com a preferência.
- **Correção:** escala de 8 estilos nomeados em `rem` com **Segoe UI Variable** (Display/Text/Small — tem tamanhos ópticos como o SF Pro).

---

## Médio

| # | Problema | Onde | Referência | Correção |
| --- | --- | --- | --- | --- |
| M1 | Espaçamento fora de grade: 7, 9, 13, 18, 26, 34, 44 px | todo o CSS | `layout.md` (alinhamento e agrupamento consistentes) | Escala 4/8/12/16/24/32/48 |
| M2 | Numeração 01–09 em Settings sugere sequência que não existe | `index.html:150-243` | Lente de craft da skill: "01 / 02 / 03 markers on content that isn't a sequence" | Agrupar em seções nomeadas (Conta, Jogo, Java, Avançado) |
| M3 | Links de texto com 18 px de altura e 0 de padding ("Change", "All patch notes", rodapé) | `.link`, `styles.css:91` | `accessibility.md`: desktop mínimo 20×20, padrão 28×28 | Área de clique ≥ 28 px via padding |
| M4 | Ações (perfil, links externos) no rodapé da sidebar | `index.html:36-50` | `sidebars.md`: "Avoid putting critical information or actions at the bottom of a sidebar" | São secundárias, então podem ficar; mas o nome do jogador também está no Play — remover a duplicação do chip ou torná-lo informativo |
| M5 | `confirm()` nativo do navegador para "Remove" | `app.js:267` | `alerts.md`/`modality.md`: título claro, botão destrutivo nomeado, Cancel | Diálogo próprio (`<dialog>`) com "Remove" destrutivo e "Cancel" padrão; Esc fecha |
| M6 | Toast e progresso não anunciados | `#toast`, `#progress` | `voiceover.md`, `progress-indicators.md` | `role="status"`/`aria-live`, `role="progressbar"` com `aria-valuenow` |
| M7 | Sidebar sem `aria-current` e sem atalhos | `.nav` | `keyboards.md`, `focus-and-selection.md` | `aria-current="page"`, Ctrl+1…5, Ctrl+, para Settings, F5 recarrega status |
| M8 | Sem hover/pressed em vários controles (segmentado, link azul, rows de notícia levantam 2 px) | vários | Lente 2 desktop: "Everything interactive has pointer feedback, a hover state" | Estados padronizados por componente |
| M9 | Altura do log fixa em `calc(100vh - 200px)` | `.log` | `layout.md`: layout segue o espaço disponível | Layout flex que ocupa o resto da página |
| M10 | "PLAY 26.3" em caixa alta, resto em sentence case | `app.js:146` | `writing.md`: capitalização consistente | "Play 26.3" |
| M11 | Seletor de versão com seta "▾" de texto e `option` com fundo hard-coded | `.select` | `pop-up-buttons.md` | Chevron SVG, cores de token, `color-scheme` correto para o menu nativo |

## Baixo

| # | Problema | Correção |
| --- | --- | --- |
| B1 | `stroke: url(#none)` sem efeito (`styles.css:57`) | Sai junto com o fundo decorativo |
| B2 | Classe `.on-other-page` aplicada em JS mas sem CSS | Definir comportamento (aviso também nas outras páginas) ou remover a classe |
| B3 | Scrollbar customizada só para dark | Token por aparência |
| B4 | Selos `.badge` com 3 estilos de cor ad-hoc | Um componente `badge` com variantes semânticas (neutral / accent / caution) |
| B5 | Cartões de notícia idênticos com arte gerada | Lista compacta de releases, sem arte falsa |

---

## O que já funciona (manter)

- **Erros em linguagem humana, dentro da interface** (banner com Try again / Details / Copy) — exatamente o que `feedback.md` pede, em vez de alertas.
- **Linhas de prontidão Java / Minecraft / Toolkit** — conteúdo honesto e útil, é a "tese" da tela Play.
- **Progresso com etapa "Step N of 6" e fração determinada quando disponível** (`progress-indicators.md`).
- **Settings com salvamento automático** e textos de ajuda curtos e diretos.
- **Copy**: botões dizem o que fazem ("Use this version", "Open on GitHub", "Try again").

---

## Regras da HIG que não se aplicam (ou se aplicam diferente) e por quê

| Regra | Por que não se aplica como está | O que faço no lugar |
| --- | --- | --- |
| **SF Pro / SF Symbols** | A licença da Apple só permite usar SF Pro e SF Symbols em apps para plataformas Apple. Distribuir num instalador Windows violaria a licença. | Segoe UI Variable (fonte de sistema do Windows, com tamanhos ópticos) e ícones SVG próprios desenhados no espírito dos SF Symbols (traço uniforme, cantos arredondados, alinhados à linha de base) |
| **Liquid Glass literal** | É um material do sistema da Apple; não existe no Windows. Imitação CSS não responde a "Efeitos de transparência" do Windows. | Mica, o material de sistema do Windows 11 (responde sozinho às preferências de transparência), na janela/sidebar. Glass CSS só em 1–2 elementos flutuantes (toast, diálogo), com fallback opaco |
| **Menu bar global com todos os comandos** | Windows não tem barra de menus global; apps WebView do tipo launcher normalmente não têm menu. | Atalhos de teclado documentados (tooltip mostra o atalho) |
| **Settings em janela separada via menu do app (Cmd-,)** | Sem menu de app no Windows; uma janela extra num launcher de 5 telas é mais atrito. | Settings continua como página da sidebar, com Ctrl+, como atalho |
| **Área de toque 44×44 pt** | É o padrão de **toque** do iOS. Este app é desktop com mouse/teclado; a HIG manda 28×28 padrão e 20×20 mínimo no desktop (`accessibility.md`). | 28 px como padrão de controle, 32–36 px nos controles principais, 44 px+ só no Play. Se quiser suportar telas touch do Windows, eu subo para 40 px — me diga |
| **Dynamic Type** | Não existe no Windows. O "Tamanho do texto" do Windows não chega de forma confiável ao WebView2. | Escala em `rem` + zoom Ctrl+/Ctrl− do WebView; layout testado a 150% |
| **Tab bar** | É navegação de iPhone. Desktop com 5 seções usa sidebar (`sidebars.md`). | Mantém sidebar |
| **Háptica** | Desktop sem atuador háptico. | — |

---

# Re-auditoria após o redesign

Data: 2026-11-17 · Commits `13d572e` → `77a7040` (um por tela, mais fundação e ajustes).

Verificação feita no app real (`tauri dev`, WebView2 157) conectado por CDP: claro e escuro, janela mínima (960×620) e grande (1440×900), navegação só por Tab, setas e Esc, Reduce Motion, Increase Contrast, Reduce Transparency e tema de alto contraste do Windows (emulados), além do contraste calculado de cada token.

## Placar antes / depois

| Área | Antes | Depois | O que mudou |
| --- | :-: | :-: | --- |
| Hierarquia visual | 6 | 9 | Fundo animado removido; Play é o único elemento colorido da tela |
| Tipografia | 5 | 8 | 8 estilos nomeados em `rem` com Segoe UI Variable; serif removido |
| Espaçamento (grade 8 pt) | 4 | 9 | Só 4/8/12/16/24/32/48 |
| Cores semânticas + light/dark | 2 | 9 | `tokens.css` com claro e escuro seguindo o sistema; nenhuma cor literal fora dos tokens |
| Contraste WCAG AA | 5 | 9 | 28 de 28 pares de token aprovados (texto ≥ 4,5:1; bordas e foco ≥ 3:1) |
| Áreas de clique | 6 | 9 | Todo alvo testado ≥ 28 px (segmentos de 24 px dentro de um controle de 28 px, acima do mínimo de 20) |
| Navegação | 6 | 8 | Ícones, `aria-current`, Ctrl+1…5, Ctrl+, e F5 |
| Materiais / Liquid Glass | 3 | 8 | Mica nativo do Windows 11; vidro só no toast e no diálogo; opaco com transparência desligada |
| Ícones | 2 | 8 | 16 símbolos SVG com o mesmo traço e a mesma grade |
| Estados | 4 | 9 | Hover, pressed, disabled legível e um anel de foco único em todos os controles |
| Animação / Reduce Motion | 3 | 9 | Nada de movimento infinito decorativo; Reduce Motion zera transições e o progresso indeterminado |
| Acessibilidade | 3 | 8 | Rótulos, radiogroups, switch focável, `role=status`/`alert`/`progressbar`, diálogo modal |
| **Total** | **49 / 120** | **103 / 120** | |

Critério: a mesma rubrica da primeira auditoria, aplicada por mim. Não é uma medida externa.

## Situação dos achados

| ID | Status |
| --- | --- |
| C1 toggle sem teclado | ✅ Checkbox real, `role="switch"`, Espaço alterna (testado) |
| C2 foco invisível | ✅ `:focus-visible` único; todos os elementos percorridos com Tab mostram o anel |
| C3 "Playing" 2,36:1 | ✅ Verde `#1a7f37`, 5,08:1 |
| C4 Play 3,41 / 3,15:1 | ✅ Fundo sólido `#0066e0`, 5,28:1, subtítulo 100% opaco |
| C5 Reduce Motion | ✅ Animação da página vai a 1 ms; barra indeterminada vira estática (testado) |
| C6 controles sem nome | ✅ `<label>`/`aria-labelledby`/`aria-describedby` em todos |
| A1 só escuro | ✅ Claro e escuro pelo sistema, sem seletor dentro do app |
| A2 cores hard-coded | ✅ Só `tokens.css` contém valores de cor |
| A3 gradiente roxo / glows | ✅ Removidos |
| A4 material na camada errada | ✅ Mica na janela/sidebar, conteúdo opaco |
| A5 contraste auxiliar/bordas | ✅ Ver tabela de tokens |
| A6 disabled ilegível | ✅ Fundo neutro + `--label-2` (≥ 6:1) |
| A7 ícones | ✅ |
| A8 escala tipográfica | ✅ |
| M1–M3, M5–M11 | ✅ |
| M4 ações no rodapé da sidebar | ⚠️ Mantido de propósito: o chip do jogador e os links são secundários e também ficam acessíveis em Play e Settings |
| B1, B3–B5 | ✅ |
| B2 classe `.on-other-page` sem CSS | ⚠️ Mantida: está em `app.js`, que foi tocado só na camada visual. O banner já aparece em qualquer página |

## O que ainda falta (por isso não é 120)

- **Testado só por automação:** não rodei Narrator/NVDA de verdade. A árvore de acessibilidade está correta, mas a leitura em voz não foi ouvida.
- **Zoom (equivalente ao Dynamic Type):** `zoomHotkeysEnabled` está ligado e toda a tipografia está em `rem`, mas o atalho Ctrl+= é tratado pelo host do WebView2 e não dá para acioná-lo por CDP. Precisa de teste manual.
- **Mica visível:** neste PC os *Efeitos de transparência* do Windows estão desligados, então o app mostra (corretamente) a versão opaca. O caminho translúcido foi verificado por emulação.
- **Sidebar não pode ser ocultada** (`sidebars.md › Consider letting people hide the sidebar`). Com 5 itens e janela mínima de 960 px, deixei de fora. Dá para adicionar se você quiser.
- **Slider nativo no modo escuro:** o trilho não preenchido do `<input type=range>` do Chromium é claro demais. É controle nativo, então mantive.
- **Fonte mono:** Cascadia Mono só existe se o Windows Terminal estiver instalado; sem ela, cai para Consolas.
