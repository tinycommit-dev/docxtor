# Browsers

Chrome and Edge are supported fully: every browser test runs on Chromium. Firefox and Safari are supported for the core of editing: the tests tagged `@core` run on WebKit (the engine of Safari) and on Firefox in the `browsers-core` CI job, and all 46 of them pass on Chromium, Firefox and WebKit. Outside Chromium the paste tests go through the browser's own clipboard, because Firefox gives a paste event made by a script an empty clipboard.

The paginator's lengths are rounded to the browser's layout unit: 1/64 px in Chrome, 1/60 px in Firefox. The unit is measured once (`measureLayoutUnit` in `editor/layout-units.ts`), so far pages, pagination and notes behave the same in every browser.

**The whole suite.** CI also runs the whole browser suite on Firefox and WebKit. Tests that depend on something only Chromium has are skipped there with a stated reason: `CHROMIUM_ONLY.columns` (28 tests that lay out columns) and `CHROMIUM_ONLY.mathml` (2 tests). Where a browser behaved differently, the difference is either handled in the product (each is in the table below) or the test no longer assumes Chromium (mostly through helpers in `e2e/helpers.ts`).

## The core set

A test is in the core set when its title carries `{ tag: "@core" }`. A tag, not a list of files, because one spec file can hold a core test (typing) and a Chromium-only one (a test of the Chromium input-method protocol); a list of files would have to take both or neither. The set covers: opening a document (the fixtures, and a file from the computer), typing, selecting, deleting, Enter, bold and italic, lists, a table, paste (plain, from Word, Google Docs, a web page, RTF, and the editor's own copy), composing text with an input method, undo and redo, find and replace, autosave to the Library, Save and download as .docx and open the file again, the print copy, the start screen and the Library.

<!-- example: browsers-list -->
```bash
npm run test:e2e:core                                    # the core set on Firefox and WebKit (Chromium runs in `npm run test:e2e`)
npx playwright test --project=webkit --grep @core --list # what is in it, before running it
```

`playwright.config.ts` has the `chromium` project always, and `firefox` and `webkit` only when asked (`DOCXTOR_BROWSERS=firefox,webkit` or `--project=`), so `npm run test:e2e` stays Chromium only. Playwright's Firefox must be the build that matches the installed Playwright (`npx playwright install firefox`); `DOCXTOR_FIREFOX_PATH` points the Firefox project at another executable (builds of another Playwright version don't speak the same protocol, so this is for a matching build kept elsewhere).

Tests that use something only Chromium has skip on the others with the reason: `{ annotation: { type: "chromium-only", description: CHROMIUM_ONLY.clipboard } }` on a test, or `chromiumOnlyFile(CHROMIUM_ONLY.cdp)` for a file (`e2e/helpers.ts`). The clipboard permissions (`clipboard-read`, `clipboard-write`) are granted by the Chromium project alone; the other browsers throw on them.

What a test can't do outside Chromium: drive a real input method (the core set composes with the events an input method makes; Chromium also has the test that uses the DevTools protocol), read or write the system clipboard from the page, or read memory counters. The Playwright core set pastes with paste events carrying each format, and with a real Ctrl+V of the editor's own copy.

## Missing APIs

What the app uses that Firefox or Safari lack or only lately have, and what happens without it.

| API | Where | Without it |
| --- | --- | --- |
| `showOpenFilePicker`, `showSaveFilePicker`, `showDirectoryPicker`, `getAsFileSystemHandle` (File System Access; Chromium only) | `web/src/files/fs.ts`, `web/src/main.ts`, `files-panel.ts` | Guarded (`canPickFiles`, `canUseFolders`). Open uses the file input and keeps a copy in the Library; Save as and Open a folder are not offered; Save downloads the .docx. |
| `document.caretRangeFromPoint` (Chrome, Safari; Firefox has `caretPositionFromPoint`) | drag and drop of text and pictures, the line focus, WordArt editing | Fixed: one helper, `caretRangeAt` in `editor/selection.ts`, uses either. |
| `AbortSignal.any` (Firefox 124, Safari 17.4) | `packages/extensions/src/net.ts` (every request's time limit) | Fixed: the two signals are joined by hand. |
| `queryLocalFonts` (Chromium) | `pdf/fontload.ts` | Guarded: installed fonts are not listed, so the PDF uses an open font of the same metrics and says so. |
| `EyeDropper` (Chromium) | `ui/swatches.ts` | Guarded: the eyedropper button is not shown. |
| `navigator.clipboard.read` with rich types (Chromium, Safari, Firefox 127) | Edit, Paste and Paste special (`editor/clipboard.ts`) | Guarded: a notice says to press the paste key, which always works. |
| CSS `field-sizing` (Chromium) | the document name field | Guarded: measured in script (`CSS.supports`). |
| Layout unit: 1/64 px cut toward zero (Chrome, Safari), 1/60 px rounded (Firefox) | the paginator's lengths (`editor/layout-units.ts`) | Fixed: measured once with a probe (`measureLayoutUnit`); with Chrome's unit in Firefox, the far pages of long documents and incremental layouts would be off by fractions of a pixel. |
| CSS `column-wrap` and `column-height` (Chromium only) | sections with two or more columns (`COLUMNS` in `editor/layout-units.ts`) | A section with columns shows as one column, with the columns' setting kept in the file; the tests that lay columns out are skipped outside Chromium (`CHROMIUM_ONLY.columns`). |
| Selecting cells: Firefox makes one range per cell (`<tr>:0` to `<tr>:1`) when a drag or Shift+click crosses cells; Chrome and Safari give one range from text to text | table selection (`readSelection` in `editor/selection.ts`) | Fixed: a selection of cell ranges reads as from the first cell's first paragraph to the last cell's last. |
| A right-click moves the caret to the point in Chrome and Safari, not in Firefox | the right-click menu (`ui/context-menu.ts`) | Fixed: the menu puts the caret where it lands, unless it lands in what is selected. |
| `getComputedStyle(el).userSelect` is the parent's `none` in Chrome, `auto` in Firefox and Safari | clicks on the interface round the text (`keepFocusOnFrame` in `ui/dom.ts`) | Fixed: the nearest value that is not `auto` decides. |
| Tab skips buttons and links in Safari (its "Press Tab to highlight each item" is off by default; Option+Tab visits everything) | dialogs (`trapTab` in `ui/dom.ts`) | Fixed: a Tab that leaves a dialog for the page behind it is brought back to the dialog's first or last stop. The keyboard tests press Option+Tab in WebKit (`tabKey` in `e2e/helpers.ts`). |
| `Intl.Segmenter` breaks "www.example.com" and "e.g" at the dots in Chrome and keeps each whole in Firefox and Safari | spelling (`spelling/words.ts`) | Fixed: a word with a full stop between letters is split there, so what is flagged does not depend on the browser. |
| Home and End scroll on a Mac in Firefox and Safari and leave the caret (Chrome moves it); Command with the arrow is the Mac's key for the line | the editing area (the browser's own keys) | Not changed: it is the platform's convention. Tests that mean the start or end of the line press `LINE_START` and `LINE_END` (`e2e/helpers.ts`). |
| Mouse positions are whole pixels in Firefox and Safari (Chrome keeps fractions) | none in the product | Tests that drag by a fraction of a length drag a whole number of pixels. |
| A canvas that an SVG picture with a `foreignObject` or a picture from outside was drawn on can't be read in Safari ("canvas has been tainted") | the PDF copy of such a picture (`pdf/images.ts`) | Handled: the picture becomes a transparent dot, so the file stays valid; Safari logs the exception it was handed, which the browser tests leave out (`e2e/helpers.ts`). |
| `-webkit-box-reflect` (Chrome, Safari; not Firefox) | the reflection of a picture (picture effects) | Not drawn in Firefox; the picture shows without it. The test checks it outside Firefox. |
| MathML layout differs: Firefox stretches a bar or a fence where Chrome does not, and Safari sets a fraction's bar lower than the math axis | equations in the PDF copy (`pdf/`), which follows what the browser drew | Not changed; two tests that read the bars are skipped outside Chromium (`CHROMIUM_ONLY.mathml`). |
| Shift+right-click sends the page nothing in Firefox (`dom.event.contextmenu.shift_suppresses_event`); its own menu opens | the right-click menu's "Shift+right-click for your browser's menu" line | As intended; the test expects no event in Firefox. |
| `focus({ preventScroll: true })` on the editing area still scrolls to the browser's remembered selection in Safari (the caret where the user last typed, perhaps pages away) | clicking or right-clicking a shape or picture after the keyboard was somewhere else (`focusRoot` in `editor/editor.ts`) | Fixed: `focusRoot` puts the scroll offsets back. |
| Safari selects the word under a right-click just before the `contextmenu` event, whatever the mouse-down did (Chrome and Firefox leave the selection when the mouse-down is cancelled) | a right-click on one of several selected ranges (`ui/context-menu.ts`) | Fixed: the menu writes the real selection back before Safari's selection event reaches the editor. |
| Delete or Backspace on a selected floating shape or picture gives Safari no `beforeinput` (nothing in the line to delete) | `onKeyDown` in `editor/editor-input.ts` | Fixed: a selection of only objects is deleted by the editor as the event would. |
| A rectangle under a transform (the zoom) is rounded to Firefox's unit on the screen, from the window's top: a block's position differs by up to 1/zoom of a unit when the editor sits lower down the page; exact at zoom 1 | the paginator's own check (`editor/layout-check.ts`, tests and development only) | Not changed in the product (the editor measures from the flow top each time and nothing adds up); the check allows that much in Firefox when zoomed. |
| Shift+Arrow over a picture, shape or other object (one unit) does not move the selection in Firefox | `onKeyDown` in `editor/editor-input.ts` | Fixed: when nothing moved, the editor extends the selection by the object. |
| Backspace in an empty first paragraph (Delete in the last) gives Safari no `beforeinput` and it takes the paragraph out of the page itself | `onKeyDown` in `editor/editor-input.ts` | Fixed: the editor does what it does for the key elsewhere. |
| A box that can't be edited (the document in Viewing) can't be focused without a tab index in Firefox and Safari | `applyReadOnly` in `editor/editor.ts` | Fixed: the flow has `tabindex="-1"` while it is locked. |
| Shift+click on a row of the Selection pane stretches the text selection to the row in Safari | `ui/selection-pane.ts` | Fixed: the key's own selecting is held back on a row. |
| Firefox's print dialog holds the page until it is closed; headless Chromium lets go at once | `window.print` | The accessibility sweep stubs it in Firefox (`e2e/a11y.spec.ts`). |
| Names from the browser's language data: Safari says "English (US)" where Chrome and Firefox say "English (United States)" | the Language dialog (`ui/language.ts`) | Not changed; the test accepts both. |
| A double-click on an element that the first click replaced: Firefox sends no `dblclick` (the second click has `detail` 2); Chrome and Safari send it to the new element | lists that draw themselves again when an item is chosen (`onChoose` in `ui/dom.ts`) | Fixed: the second click is the double-click. |
| Painting many masked elements (the spelling wave is a CSS mask) on every frame is slow in Safari | the spelling layer (`.sp-layer` in `editor.css`) | Fixed: the layer is its own compositing layer, and typing redraws only the marks that move. |
| `requestIdleCallback` (not in Safari) | `core/engine.ts`, `editor/spell-layer.ts` | Guarded: a timer. |
| CSS Custom Highlight API (`CSS.highlights`; Firefox 140, Safari 17.2) | find (`ui/find.ts`), protection ranges (`ui/protection.ts`) | Guarded: no highlight is painted, the find bar's list and count still work. |
| `CompressionStream`, `DecompressionStream`, `OffscreenCanvas`, `Intl.Segmenter`, `structuredClone`, `navigator.locks`, `BroadcastChannel`, `:has()`, `color-mix()`, `@container` | PDF writer, spelling, charts, document lock | In every Firefox and Safari from 2023 on; not guarded. |
| `launchQueue` (installed web app opening files) | `web/src/offline.ts` | Guarded: only the installed Chrome app uses it. |
