# Extensions for design and content

Cover pages, smart chips, themes, pictures, icons and reusable parts. Part of the [extensions guide](index.md).

## Cover pages

Insert, Cover page lists Docxtor's own designs, then every snippet in the "Cover pages" gallery of any snippet store (`ctx.snippets.stores`), and the document's own building blocks of that gallery. So an extension adds cover pages by registering a snippet store whose snippets name that gallery; choosing one puts its fragment in a Cover Pages content control at the start of the document, as Word does. Docxtor's designs come from the engine (`Engine.pageGallery("cover")`, `insert_cover_page`), which also writes the headers and footers of Insert, Header and Footer (`insert_page_part`).

## Smart chips and the @ menu

Smart chips are content controls with a tag that starts with `docxtor:chip:` (`person`, `date`, `file`, `place`, `dropdown`, or a kind of your own), so Word shows their text. What fills them is contribution points; Docxtor's own are registered through them like anyone else's:

| Point | What it adds |
|---|---|
| `atMenu` | Lines of the @ menu: `{ id, label, order?, items(query) }`, each line `{ id, label, detail?, icon?, section?, keywords?, run() }`. When `run` is called the @ and the typed text are gone, so the caret is where the line's thing goes. `ExtensionHost.atMenuSections(query)` lists them in sections. Docxtor's: People, Dates, Files, Drop-downs, Building blocks, Insert. |
| `people` | Where person chips' people come from: `{ id, label, search(query), remember?(person) }`. Docxtor's is local: you (Tools, User information), the document's author and last editor, people already in chips, and people added by hand (kept on this device). A server's accounts or a directory is another provider. |
| `files` | Documents file chips point to: `{ id, label, search(query) }` returning `{ title, url, detail?, modified? }`. The app registers the recent documents; an address starting with `#` opens in the app. |
| `dropdowns` | Ready-made drop-down chips: `{ id, label, options: [{ label, color? }] }` (colours: gray, blue, green, yellow, red, purple). Docxtor's: Status and Project status. |
| `buildingBlocks` | Docs-style building blocks: `{ id, label, description?, category?, icon?, markdown }`, Markdown with tokens that become chips: `{{date}}`, `{{person}}`, `{{status:Done}}`, `{{project-status:On track}}`, `{{dropdown:id:Option}}`, `{{chip:kind:Text}}`. Docxtor's: Meeting notes, Email draft, Product roadmap, Project assets, Content tracker, Review tracker, Decision log, Launch content tracker. |
| `linkPreviews` | A page's title and description for the card a link shows: `{ id, label, matches?(url), preview(url, signal) }`. Docxtor ships none: looking a page up sends its address to the web, so a provider is its own extension, off by default, that says so. The card works without one (the address, its site, and for a link inside the document the target's text). |

## Themes and style sets

Design, Themes changes the colour scheme and the font scheme in the document's theme part (`theme1.xml`). Everything that names a theme colour or a theme font follows: text and heading colours, table designs, paragraph borders and fills, shapes, charts. Docxtor's own themes (Word's: Office, Facet, Integral, Ion, Wisp, Wood Type and the rest), its colour schemes and its font pairs are registered through `ctx.documentThemes` by the built-in `docxtor.design` extension (`packages/editor/src/builtins/design.ts`), so the galleries list yours beside them.

<!-- example: ext-document-theme -->
```ts
ctx.documentThemes.register({
  id: "acme.brand",
  label: "Acme",
  colors: { dk2: "1B2A41", lt2: "ECE9E1", accent1: "C8102E", accent2: "0A5C8C", accent3: "6C7A89", accent4: "F2A900", accent5: "2E7D32", accent6: "7B1FA2" },
  fonts: { major: "Georgia", minor: "Verdana" },
});
ctx.documentThemes.register({ id: "acme.mono", label: "Acme mono", fonts: { major: "Consolas", minor: "Verdana" } }); // only a font pair
```

A theme with `colors` and `fonts` appears under Themes, one with only `colors` under Colors and one with only `fonts` under Fonts. Choosing it is one `set_theme` op, so it is one undo step and the assistant and the CLI can do the same. Colours must be six hex digits; the engine refuses anything else with a message.

A style set is a list of style definitions applied together:

<!-- example: ext-style-set -->
```ts
ctx.styleSets.register({
  id: "acme.report",
  label: "Acme report",
  description: "Red headings, 12 pt body",
  styles: '<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="360" w:after="120"/></w:pPr><w:rPr><w:b/><w:color w:val="C8102E" w:themeColor="accent1"/><w:sz w:val="36"/></w:rPr></w:style>',
  spacing: { after: 160, line: 276 },
});
```

For a style the document already has, only the paragraph and character formatting (and the following style and the gallery flag) are replaced; its name and what it is based on stay. A style it lacks is added. Naming theme colours in `w:color` (`w:themeColor="accent1"`) lets the set follow the document's theme. The gallery draws each set's title, headings and body text in the document's own theme with `Engine.styleSetPreview`. `spacing` is in twips (`before`, `after`) and 240ths of a line (`line`).

The newest `Set as default` design is kept in the extension state the host provides (the app keeps it in this browser) and new documents start with it: `Engine.defaultDesign` holds it for `Engine.blank`.

## Picture sources

Insert, Image lists every picture source. Docxtor's own two are plain registrations in `packages/editor/src/builtins/pictures.ts`: **From your computer** (a file chooser) and **From a URL** (the browser fetches the address; the dialog says so, and says what to do when the site does not let other pages read its pictures). An extension adds a line by registering one more.

<!-- example: ext-picture-source -->
```ts
ctx.pictures.sources.register({
  id: "acme.stock",
  label: t("From Acme stock photos…"),
  icon: "image",
  note: t("Sends your search words to acme.example."), // anything that leaves the device is said plainly
  async pick({ ui, signal }) {
    const word = await ui.prompt(t("Search Acme stock photos"));
    if (!word) return null;
    const res = await fetch(`https://acme.example/photo?q=${encodeURIComponent(word)}`, { signal });
    if (!res.ok) throw new Error(t("Acme did not answer."));
    return { bytes: new Uint8Array(await res.arrayBuffer()), name: `${word}.jpg`, alt: word };
  },
});
```

`pick` runs when the person chooses the line; throw an `Error` with a message fit to show and the editor shows it. The picture goes in at the caret, as large as it is and no wider than the page, with `name` (or `alt`) as its alt text. TIFF, EMF and WMF are accepted: the engine shows them as PNG or SVG. A source that sends anything off the device is its own extension, off by default, with a `privacy` line (see "Sending text away" below).

## Icon packs

Every icon is looked up by id (`bold`, `table`, `chartPie`…). An icon pack is a named set of them:

<!-- example: ext-icons -->
```ts
import { defineExtension } from "@docxtor/extensions";

export default defineExtension({
  id: "acme.icons",
  name: "Acme icons",
  activate(ctx) {
    ctx.iconPacks.register({
      id: "acme",
      label: "Acme",
      icons: [
        { id: "bold", svg: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">…</svg>' },
        { id: "table", svg: "<svg …>…</svg>" },
      ],
    });
  },
});
```

- Docxtor's own icons are the pack `docxtor`, and `docxtor-light` (the same drawings with a lighter line) ships with it. View, Icons lists every pack and remembers the choice. The icons are 24×24, draw in `currentColor`, and are listed in `packages/editor/src/ui/icons.ts` and `ui/chart-icons.ts`.
- A pack needs only the icons it redraws. The ones it leaves out come from the `docxtor` pack.
- Order of precedence, last to first: an icon registered on its own with `ctx.icons` (it wins over any pack), the chosen pack, the `docxtor` pack. Within each, the most recently added wins. Registering a pack under an id that already exists adds to that pack.
- A pack whose extension is turned off stops being offered, and Docxtor's icons show instead. The choice is kept, so the pack is back when the extension is.
- In code: `host.listIconPacks()`, `host.iconPack` (the chosen id), `host.setIconPack(id)` and `host.onDidChangeIcons(fn)` (called once, in a microtask, for a run of changes). `DocxtorEditor` has `iconPacks()`, `currentIconPack` and `setIconPack(id)`, the `iconPack` option and, on `<docxtor-editor>`, an `icon-pack` attribute. The app remembers the choice itself, as it does the theme; an embedding page does that on its own.

## Shared lists and quick parts

Two things belong to the person, not to one document: a list of sources every document can cite, and the pieces of text they save to insert again (Word's building blocks and AutoText). Both are contribution points, so where they are kept is a plug. Docxtor's own keep them in the browser (`web/src/stores.ts`: the extensions `docxtor.library.local` and `docxtor.snippets.local`); a self-hosted deployment or a team can register one that keeps them on a server or a shared drive. The editor never touches storage itself.

Both hold the whole list: `list()` reads it and `save(all)` writes it back, so a store can be a folder, a file or an API. Say in `note` where it is kept and who can read it; anything that leaves the device is its own extension, off by default.

<!-- example: ext-stores -->
```ts
ctx.snippets.stores.register({
  id: "acme.team-parts",
  label: "Team parts",
  note: "Kept on the team drive. Everyone on the team can read them.",
  list: () => fetch("/parts.json").then((r) => r.json()),
  save: (all) => fetch("/parts.json", { method: "PUT", body: JSON.stringify(all) }).then(() => undefined),
});
ctx.citations.libraries.register({ id: "acme.shared-sources", label: "Team sources", list, save });
```

A `Snippet` is `{ id, name, gallery, category?, description?, fragment, text, created }`. `fragment` is what `Engine.copyFragment` makes from a selection (runs, paragraph properties, styles, lists, tables, pictures and links), so a snippet carries its formatting; `gallery` is `Quick parts`, `AutoText` or a name the person chose. The commands are `quickParts.save` (Save selection to Quick parts), `quickParts.open` (the gallery and organiser: insert, edit name, gallery, category and description, delete) and `quickParts.expandAutoText` (the AutoText named by the word before the caret takes its place). The `Source` of a library is the same `Source` the Citations panel uses.

A document can also bring building blocks of its own: the glossary part of a .docx or .dotx (`word/glossary/document.xml`). The engine reads them without changing the file (`Engine.buildingBlocks()`, `Engine.buildingBlockFragment(index)`, the `insert_building_block` op) and the gallery lists them beside the saved ones, to insert but not change. Content controls that came from a gallery (`w:docPartObj`: Word's tables of contents, cover pages, bibliographies) are listed by `Engine.docPartControls()` and stay as they are on saving.
