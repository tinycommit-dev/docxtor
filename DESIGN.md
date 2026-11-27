---
version: alpha
name: Docxtor
description: An open-source Word editor where the document sits on a quiet desk, and every mark the editor or the AI makes about it is easy to tell apart from the document itself.
colors:
  canvas: "#fffefb"       # Zapier canvas: top bar, toolbar, menus, dialogs
  canvas-soft: "#f8f4f0"  # Zapier canvas soft: sidebar, insets
  desk: "#efe9e0"         # behind the pages (derived from canvas-soft and mute)
  paper: "#ffffff"        # pages stay white, as Word renders them
  paper-ink: "#201515"    # drawings of the page (list style previews, checklist boxes): ink in both themes
  paper-line: "#c5c0b1"   # greyed-out text lines in those drawings
  paper-ins: "#266a42"    # a tracked insertion on the page: green, underlined, 6:1 on white in both themes
  paper-del: "#b42318"    # a tracked deletion on the page: red, struck through, 6.5:1 on white in both themes
  paper-cite: "#8f8b7f"   # the outline around a citation under the pointer, drawn on the page: 3.4:1 on white in both themes
  paper-range: "rgb(32 21 21 / 0.07)" # the parts of a protected document anyone may edit: a light tint on the white page, with a bracket (paper-cite) at each end
  paper-mark: "#6b675d"   # non-printing characters on the page (¶, space dots, break rules): muted, 5.6:1 on white in both themes
  paper-field: "rgb(32 21 21 / 0.07)" # the tint behind a field code when View, Field codes is on, drawn on the page; the code itself is paper-mark (4.8:1 on the tint)
  paper-spelling: "#d93025" # the wavy underline under a misspelled word: red like Docs', 4.8:1 on white in both themes
  paper-grammar: "#1967d2" # the wavy underline under a grammar problem, dotted for a style hint: blue like Docs', 5.4:1 on white in both themes
  chip-gray-bg: "#eee9df"  # smart chips on the page: a tinted pill with the colour's own text (6.8:1 or better), the same in both themes because pages stay white
  chip-gray-fg: "#36342e"
  chip-blue-bg: "#e0eafb"
  chip-blue-fg: "#17418f"
  chip-green-bg: "#dcf0e3"
  chip-green-fg: "#1a5b35"
  chip-yellow-bg: "#fbeec2"
  chip-yellow-fg: "#654400"
  chip-red-bg: "#fbe2de"
  chip-red-fg: "#951b11"
  chip-purple-bg: "#eae2f6"
  chip-purple-fg: "#55298a"
  chart-1: "#4472c4"      # a chart's series colours when the file gives none: Word's Office palette (a document theme's own accents win)
  chart-2: "#ed7d31"
  chart-3: "#a5a5a5"
  chart-4: "#ffc000"
  chart-5: "#5b9bd5"
  chart-6: "#70ad47"
  paper-gridline: "#d9d5ca" # the gridlines shown on a page (View, Gridlines): a hairline on the white page, not text
  focus-dim: "rgb(32 21 21 / 0.66)" # the veil over everything but the lines in view in Line focus: ink at 66%, over the page and the desk alike
  paper-chart-text: "#595959" # axis, legend and label text drawn on a chart: 7:1 on white in both themes
  paper-chart-grid: "#d9d9d9" # a chart's gridlines: a hairline on the page, not text
  paper-chart-axis: "#bfbfbf" # a chart's axis lines
  ink: "#201515"          # Zapier ink: text, icons, primary buttons
  ink-mid: "#36342e"      # strong secondary text
  muted: "#605d52"        # Zapier body: secondary text (5.5:1 on desk)
  faint: "#6b675d"        # tertiary text (4.7:1 on desk)
  quiet: "#939084"        # Zapier body mid: placeholders, disabled icons only (never body text)
  mute: "#c5c0b1"         # Zapier mute: hairlines only
  signal: "#ff4f00"       # Zapier orange: selection, the AI caret mark, the blank sheet
  signal-text: "#b03600"  # orange that is text (5.2:1 on desk)
  danger: "#b42318"
  ok: "#266a42"
  dark-desk: "#1a1212"
  dark-chassis: "#201515"
  dark-panel: "#2f2a26"   # Zapier ink soft
  dark-panel-hi: "#36342e" # Zapier ink mid
  dark-ink: "#fffefb"
  dark-muted: "#c5c0b1"
  dark-faint: "#a9a497"
typography:
  ui:
    fontFamily: Inter
    fontSize: 13px
    lineHeight: 1.45
    fontFeatureSettings: "cv11"
  heading:
    fontFamily: Inter
    fontSize: clamp(28px, 3.4vw, 40px)
    fontWeight: 650
    letterSpacing: -0.03em
  label:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 500
  numbers:
    fontFamily: Inter
    fontVariantNumeric: tabular-nums
rounded:
  sm: 6px   # toolbar keys, inputs
  md: 12px  # buttons, menus, panels
  lg: 16px  # dialogs
dialog:
  s: 440px    # confirmations, one or two fields
  m: 600px    # most dialogs (the default)
  l: 800px    # two columns, a preview, long lists
  xl: 1040px  # galleries and side-by-side views
  height: min(720px, 100dvh - 48px)     # every dialog; xl may grow to min(880px, 100dvh - 48px)
spacing:
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.canvas}"
    rounded: "{rounded.md}"
  button:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.ink}"
    border: 1px hairline
    rounded: "{rounded.md}"
---

## Overview

Docxtor is for students, writers and people who work with AI agents. It opens a Word file, edits it on real pages and gives it back intact. The look comes from the copy editor's desk: the document is in ink on white paper, and the interface is the desk around it. The palette is Zapier's (cream canvas, warm ink, one orange) and the face is Inter. Both are fixed parts of the design.

The signature is the **proofreader's caret ‸**. It marks everything the AI does: the Assistant button, the panel header, the "thinking" state, and a small caret in the page margin beside each paragraph the AI changed, the way a copy editor marks a change.

## Colors

- **Desk, canvas, canvas soft:** the desk sits behind the pages so white paper reads as paper. Chrome is canvas, the sidebar is canvas soft. Regions are separated by tone first, hairlines second.
- **Ink (#201515):** all text and icons, and every primary button. 14.8:1 on the desk.
- **Paper ink and paper line:** things drawn on or as the page (list style previews, checklist boxes) stay ink on white in both themes, because the page itself never turns dark.
- **Paper mark:** the non-printing characters (¶, dots for spaces, tab and line break arrows, the dotted rules of page, column and section breaks) are the document's marks, not chrome. They sit on the white page in both themes, so they use one muted warm grey (#6b675d, 5.6:1 on white) rather than the themed `muted`.
- **Paper gridline and text boundaries:** View, Gridlines draws a hairline grid on the white page in one pale warm grey (`paper-gridline`, a line and not text), in both themes. View, Text boundaries draws the text area, the columns, table cells and text boxes with a dotted line in `paper-mark`, like the break rules. Neither is printed.
- **Focus dim:** Line focus lays a veil of ink at 66% over everything except the lines in view, so the lines keep their own colours and contrast. It moves without animation when the person prefers reduced motion. The bar over it is an ordinary floating panel.
- **Paper cite:** a citation in the text is one unit. Under the pointer, and while its popover is open, it gets a hairline outline and the page hover tint, in one muted warm grey (#8f8b7f, 3.4:1 on white, a line and not text) on the white page in both themes. It is not orange: orange stays the selection.
- **Paper range and content controls:** the parts of a protected document anyone may edit are tinted (`paper-range`, 7% ink on white in both themes) with a 2px bracket in `paper-cite` at each end, as Word does. A content control is outlined in `paper-cite` (or the colour the file gives it) while the caret is in it or the pointer is over it, with the `paper-hover` tint; placeholder text uses the document's own grey and `paper-mark` when the file gives none; a legacy form field's blank is shaded with `paper-hover`. The tab above a control is a floating thing like the image strip: `panel-hi`, a soft shadow and 11px text. None of this is orange: orange stays the selection.
- **Paper spelling:** the wavy underline under a misspelled word is the document's mark too, drawn on the white page in both themes, so it has one red of its own (#d93025, 4.8:1 on white). It marks "this may be wrong" and nothing else; it is not `danger`, which is for interface errors.
- **Paper grammar:** the underline under a grammar or style problem is the document's mark too, drawn on the white page in both themes, so it has one blue of its own (#1967d2, 5.4:1 on white). It is wavy for a grammar problem and dotted for a style hint, so the two differ by shape and not only by colour, and blue next to spelling's red keeps the two kinds of problem apart. Like paper-spelling it says "this may be wrong" and nothing else.
- **Smart chips:** a person, date, file, place or drop-down chip is a rounded pill drawn on the white page in both themes, with a small icon in front and a ring that is its own text colour at a quarter strength. People, dates, files and places are `chip-gray`; a drop-down chip takes the colour of the choice (`chip-blue`, `chip-green`, `chip-yellow`, `chip-red`, `chip-purple`, `chip-gray`), and the choice is also its words, so colour is never the only signal. The pill never uses orange. Under the pointer or with the caret in it, the ring turns `paper-cite`, like a content control's outline. The @ menu and the cards that open from a chip are floating panels, so they follow the theme.
- **Table design tiles:** the gallery draws each of Word's table styles on `paper` with the colours the engine resolved for the document's theme (a table's colours are the document's, like a chart's), and the text lines in `paper-ink` or the style's own text colour. Tiles keep their paper in both themes; the table's current design is ringed in `ink`.
- **Chart colours:** a chart is drawn on the white page in both themes, as Word prints it. A series takes the colour its file gives it (a theme colour is resolved against the document's theme); a series with none takes Word's Office palette (`chart-1` to `chart-6`, then the same colours darker and lighter). Chart text, gridlines and axes are Word's greys (`paper-chart-text` 7:1 on white; the lines are hairlines, not text). Chart text is the document's font at Word's sizes (9 pt labels, 14 pt title). The new chart types (waterfall, histogram, box and whisker, funnel, treemap, sunburst) follow the same rules: increase, decrease and total take `chart-1`, `chart-2` and `chart-3`, branches of a treemap or sunburst take one palette colour each and their leaves lighter shades of it, and a name written on a bar, tile or arc is `paper-ink` or `paper`, whichever contrasts more with that fill (a label outside a bar keeps the chart text grey with a paper halo).
- **Style, theme and style set tiles:** the style gallery, the Styles panel, the theme, colour and font galleries, and the style set gallery draw each choice on `paper` in its own fonts and colours (a style's look comes from the document, not the interface), with text in `paper-ink` where the style names no colour. Tiles keep their paper in both themes and the current choice is ringed in `ink`, like the table design tiles. The previews in Modify style are on `paper` too.
- **Paper ins and paper del:** a document with Word's tracked changes shows insertions in green with an underline and deletions in red, struck through, on the white page in both themes (6:1 and 6.5:1 on white). The comparison view in the app uses `ok` and `danger`, which follow the theme, with the same underline and strike-through so colour is never the only signal.
- **Mail merge:** Mailings, Highlight merge fields tints merge fields with `paper-field` (the tint behind a field code), on the white page in both themes. The previews in the address block and greeting line dialogs, and the envelope drawn in the Envelopes dialog, are on `paper` with `paper-ink` text and keep their paper in both themes, like the style tiles. The strip over the pages while a record is previewed is a floating panel like the header and footer strip.
- **Muted / faint:** secondary and tertiary text, both above 4.5:1 on every surface in both themes. `quiet` (#939084) never carries readable text. On the shading of a chosen, hovered or focused line (a menu line, a list row), secondary text uses `ink-2`: `muted` and `faint` fall to 4.2:1 to 4.4:1 on the dark theme's selection shading. `e2e/a11y.spec.ts` (axe, both themes, every dialog) holds all of this.
- **Orange (#ff4f00) has three jobs only:** the selection wash, the AI caret mark, and the blank "New document" sheet. Orange is never a button fill and never an underline on a control. Orange text uses `signal-text`.
- **Dark mode:** built from Zapier's ink family: ink is the chassis, ink soft and ink mid are surfaces, canvas is text. Pages stay white.

## Typography

Inter everywhere, in sentence case. Labels are 12px medium; nothing in the interface is smaller than 11px. Numbers use tabular figures. There is no monospace in the chrome; `--font-code` exists only for code. The one loud moment is the start screen heading ("Your documents"), 650 weight with tight tracking. Document content keeps its own fonts (Calibri, Cambria and their metric-compatible substitutes) because fidelity comes first.

## Layout

The editor: top bar, one-row toolbar, navigator on the left, pages in the middle, assistant on the right, status bar. The start screen is a desk of documents shown as pages: a blank sheet for a new document, then recent documents with their real first lines.

Dialogs come in four widths (s, m, l, xl: 440, 600, 800 and 1040px, never wider than the window) and one height limit (720px, or the window less 48px). The title and the buttons stay in place and only the middle scrolls; a dialog with tabs keeps its height when a shorter tab is chosen. A new dialog picks a size; it doesn't get a width of its own.

## Risky actions

Ask first, and say what changes, when an action can't be undone or changes the whole document (a header on every page, every shortcut you set). The question names the thing ("Remove the header?"), says the consequence in plain words, and its button names the action ("Remove header", never "OK"), drawn as dangerous, with Cancel focused. A local change the person sees happen (a table deleted) is done at once and offers Undo in the notice. Small edits get neither: asking every time teaches people to stop reading. Opening something else never quietly leaves the document: it opens in a new tab and says so in its name (Help, Open the tour in a new tab), as Google Docs does. Every warning is written in one place (`packages/editor/src/builtins/cautions.ts`, `web/src/cautions.ts`).

## Elevation & Depth

Flat. Pages have a 1px hairline, not a drop shadow. Only floating things (menus, popovers, the selection bubble, toasts) get a soft shadow, because they genuinely float over content.

## Shapes

6px for toolbar keys and inputs, 12px for buttons, menus and panels, 16px for dialogs, fully round for status pills. Pages have a 2px radius only on start-screen thumbnails.

## Components

- Primary button: ink fill, canvas text. Secondary: canvas with a hairline.
- Toolbar active state (bold on, current alignment): the whole key is filled with a neutral ink tint (`--selected`), like a pressed key. No underline.
- Save status in words: "Saved in your Library", "Unsaved changes", "Couldn't save".
- Status bar in words: "Page 3 of 12", "1,204 words", "100.0% round-trip fidelity".
- Header/footer editing: an orange dashed frame and a strip that names what you're editing ("Header on page 1").

## Icons

Docxtor draws its own stroke icons: a 24px grid, 1.75 stroke, round caps and joins, no fills except small dots, drawn in `currentColor` so they take the text colour (muted in menus, ink when active). They live in `packages/editor/src/ui/icons.ts` and reach the interface through the `docxtor.icons` extension, so the menus and the toolbar look icons up by id and an icon pack can replace any of them. Every line of the menu bar and the right-click menu has an icon slot; a setting that's on shows a check mark in that slot instead of its icon. New commands get an icon in `COMMAND_ICONS` (in `packages/editor/src/builtins/icons.ts` or `web/src/builtins.ts`); if none fits, leave the slot empty rather than reuse one that means something else.

## Do's and Don'ts

- Do add a token to `packages/editor/src/styles/tokens.css` (on `.docxtor`) before using a colour. Never inline a colour in a rule. Themes from extensions override these tokens and nothing else.
- Do keep orange to its three jobs.
- Don't change: Inter, the Zapier palette (both are fixed parts of the design; the scanner is told via `avoid-ai-design-ignore-file: SD1, T1`).
- Don't use: tracked ALL-CAPS labels, monospace labels or readouts, `A · B · C` metadata strings, `→` glued to labels, decorative 01/02/03 numbering, one accented headline word, LED status dots, inset "instrument" wells, dot-grid textures, orange button fills.
