# Extensions for mail merge, citations and fields

Where recipients come from and how messages are sent, how citations are formatted and found, and new field types. Part of the [extensions guide](index.md).

## Mail merge

Mailings (the built-in extension `docxtor.mailMerge`, `packages/editor/src/builtins/mail-merge.ts`) works on a document that has merge fields (`MERGEFIELD`, `ADDRESSBLOCK`, `GREETINGLINE` and the rules) and a list of recipients. The settings are written as Word writes them (`w:mailMerge` in `settings.xml`, linked to the file the list came from). The list itself is kept in a custom XML part of Docxtor's own, so the merge still works when the file is opened again; Word ignores the part. Merging is a pure function of the document and one record (`Document::merge_to_docx` in the engine), so the editor, the assistant and `docxtor merge` give the same result.

Two contribution points feed the dialogs. Both are plain registrations: turning the extension off takes them away.

A **data source** is where recipients come from. Docxtor's CSV and Excel readers are data sources like any other; the engine reads the files (`Engine.readRecipients`).

<!-- example: ext-mail-merge-source -->
```ts
ctx.mailMerge.dataSources.register({
  id: "acme.sheets",
  label: t("From an Acme sheet…"),
  icon: "mmRecipients",
  note: t("Sends the sheet's address to acme.example."), // anything that leaves the device is said plainly
  async pick({ ui, signal }) {
    const id = await ui.prompt(t("Sheet address"));
    if (!id) return null;
    const res = await fetch(`https://acme.example/sheets/${encodeURIComponent(id)}.json`, { signal });
    if (!res.ok) throw new Error(t("Acme did not answer."));
    const sheet = await res.json();
    return { columns: sheet.header, rows: sheet.rows, name: sheet.title };
  },
});
```

`pick` runs when the person chooses the line; throw an `Error` with a message fit to show and the editor shows it. The list goes into the document as it is (a header row of column names, and a record of text values for each recipient). Give `kind: "csv"` or `kind: "xlsx"` (and `table` for a sheet) when the list is a file Word can link to; otherwise the document keeps it only inside the file. A source that sends anything off the device is its own extension, off by default, with a `privacy` line (see "Sending text away" below).

A **label product** is a kind of label sheet. Sizes are in `unit` (`in` by default, or `mm` or `pt`), as the maker gives them; the engine builds the table from them as Word does (a column for each label across and a narrow one for each gap, rows exactly as tall as the vertical pitch, page margins that put the first label where the sheet has it).

<!-- example: ext-label-product -->
```ts
ctx.mailMerge.labelProducts.register({
  id: "acme.40x20", vendor: "Acme", name: "40 x 20 mm address labels", unit: "mm",
  pageWidth: 210, pageHeight: 297, top: 13, left: 5, labelWidth: 40, labelHeight: 20, pitchH: 40, pitchV: 20, across: 5, down: 14,
});
```

A product whose pitch is smaller than its label, or whose counts aren't whole numbers, is refused with a message. `ExtensionHost.labelProductGroups()` lists the products by vendor for a dialog of your own.

A **mail sender** sends the messages of an email merge. It gets every message at once, as plain data (`{ to, subject, html?, text?, attachments?, record }`, merge fields filled in), and reports each one with `ctx.progress(index, { ok: true })` or `{ ok: false, error }` as it goes; one not reported counts as sent when `send` resolves, and as failed when it throws. `ctx.signal` is aborted when the person stops. `formats` lists what it can send (`html`, `text`, `docx`, `pdf`); a format it can't is changed to its first, and the dialog says so. Docxtor's own senders need no server: Save as email files (a .zip with one .eml file for each recipient, MIME with HTML and text, names and subjects encoded for any language) and Open in my mail app (one `mailto:` at a time, plain text, with a warning above 10 messages). A sender that goes through a mail service (SMTP, Gmail, Outlook) is its own extension, off by default, that says in `note` where the messages go.

<!-- example: ext-mail-sender -->
```ts
ctx.mailMerge.mailSenders.register({
  id: "acme.smtp", label: "Send with Acme mail", note: "Sends each message through mail.acme.example.", formats: ["html", "text"],
  async send(messages, ctx) {
    for (const [i, m] of messages.entries()) ctx.progress(i, (await acme.send(m)) ? { ok: true } : { ok: false, error: "Refused" });
  },
});
```

The merge itself is ops (`set_mail_merge`, `set_merge_recipients`, `insert_merge_field`, `update_labels`, `make_labels`, `insert_envelope`), so an extension can drive it with `ctx.document.exec`, and the assistant has `start_mail_merge`, `insert_merge_field`, `mail_merge_info` and `preview_merge_record`.

## Citation styles and sources

Two contribution points feed the Citations panel. Both are plain registrations, so an extension can add a style or a place sources come from, and turning the extension off takes it away. Docxtor's own DOI, BibTeX and Zotero lookups, and its CSL styles, are extensions of this kind (`web/src/ui/citations/sources.ts`, `styles.ts` and `web/src/zotero.ts`).

A **style** has an `id` the document remembers, a `label` for the picker and, for a CSL style, its XML. A function loads a large style only when it is picked or when a document that uses it is opened. The five styles Docxtor formats itself (`apa`, `mla`, `chicago`, `ieee`, `harvard`) are registered without `csl`.

<!-- example: ext-citation-style -->
```ts
ctx.citations.styles.register({
  id: "acme.nature",
  label: t("Nature"),
  group: t("Numbered"),
  csl: () => fetch("/styles/nature.csl").then((r) => r.text()),
});
```

A style whose `class` is `note` (Chicago notes and bibliography) turns citations into footnotes with a `CITATION` field, so Word can still edit them. Changing between a note style and any other converts the citations in one undo step. Styles from the [Zotero style repository](https://github.com/citation-style-language/styles) are CC BY-SA 3.0: keep their licence line with them.

A **source provider** finds sources. `search(query, signal)` returns results as the person types; `resolve(input, signal)` turns something typed, pasted or read from a file (a DOI, BibTeX text) into sources. Throw an `Error` with a message fit to show, and the dialog shows it. Sources the document already has are listed but left out.

<!-- example: ext-citation-source -->
```ts
ctx.citations.sources.register({
  id: "acme.library",
  label: t("From the library catalogue…"),
  title: t("Add from the catalogue"),
  inputLabel: t("Search the catalogue"),
  note: t("Searches are sent to catalogue.example.org."),
  async search(query, signal) {
    const res = await fetch(`https://catalogue.example.org/api?q=${encodeURIComponent(query)}`, { signal });
    if (!res.ok) throw new Error(t("The catalogue didn't answer. Try again later."));
    return (await res.json()).items.map((item: { id: string; title: string; author: string; year: number }) => ({ id: item.id, source: { type: "book", title: item.title, authors: [{ family: item.author }], year: item.year, tag: "" } }));
  },
});
```

`resolve` with `review: true` opens the one source it finds in the form to check before adding; with `multiline` it shows a text box, and `files` adds a file chooser. The DOI provider and the BibTeX provider are the examples of each. Provider-specific settings go in `settings`; the Zotero provider keeps its user ID and API key with `ctx.state`, which stays in the browser.

## Fields

Fields are a contribution point too. `ctx.fields.register({ name, category, title, description, options, evaluate })` adds a name to Insert, Field. Docxtor's own fields (`DATE`, `AUTHOR`, `PAGE`, `REF`, `STYLEREF`, formulas and the rest, in `packages/editor/src/builtins/field-types.ts`) are registered the same way by the built-in extension `docxtor.fields`, and the engine works out their results; an extension that adds a field says how to work its out.

- `name` is the field name in capitals (`WEATHER`), or `=` for a formula. `category` is one of the dialog's groups (`Date and time`, `Document information`, `Numbering`, `Links and references`, `Equations and formulas`, `User information`, `Document automation`) or a name of your own.
- `options` is the form: each `{ id, label, kind: "text" | "choice" | "toggle", write, switch?, value?, choices?, default?, placeholder?, help? }`. `write` says how the value goes into the instruction: `arg` (a word after the name), `raw` (as typed), `switch` (`\x`), `switch-value` (`\x "value"`), `format` (`\* value`), `date` (`\@ "picture"`) or `number` (`\# "picture"`). `buildInstruction(type, values)`, `readValues(type, instr)` and `parseInstruction(instr)` in `@docxtor/extensions` write and read them.
- `evaluate(call)` gets `{ instr, code, args, switches, formats, datePicture, numberPicture, engine, now }` and returns the result as plain text (or a promise of it); null leaves the result as it is. It runs on Update fields (F9), after editing a field's code and after inserting one, as part of the same undo step, and for a field in the text only. The latest registered type with a name wins; a failing evaluator is reported and the result stays.
- Docxtor's engine takes care of the field's `\#` and `\*` switches for its own fields; an extension's evaluator receives them parsed and applies what it wants.

<!-- example: ext-field -->
```ts
ctx.fields.register({
  name: "WEATHER",
  category: "Document automation",
  title: "Weather",
  description: "The weather in a city.",
  options: [{ id: "city", label: "City", kind: "text", write: "arg" }],
  evaluate: async (call) => `Sunny in ${call.args[0]}`,
});
```

The editor tells the engine what it can't know (the time, the person's name from Tools, User information, the file's name and size, the number of pages) with `FieldInput` on the `update_fields`, `insert_field` and `set_field_code` ops. Fields are updated when a file opens (`open: true`: dates, fields marked dirty, and every field when the file asks with `w:updateFields`), and before printing and every download (`quiet: true`: no undo step when nothing changes); the second is a setting, Tools, Update fields before printing.
