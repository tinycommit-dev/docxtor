// The sample document the tour opens, as DocScript.
import { msg } from "@docxtor/editor";

export const SAMPLE = msg(`#! Welcome to Docxtor

A free, open-source editor for Word documents. *Built in Rust, made for people, fluent with AI.*

## What makes it different

- **Lossless by design.** Anything Docxtor doesn't understand is preserved and written back untouched, so your files survive the round trip.
- **Measured fidelity.** Every file you open is scored. Unedited files re-save with all of their parts intact.
- **AI that edits like a code editor.** Connect any model — Claude, GPT, Gemini or a local one — and it reads your document's structure by id: every heading, list, table and style.
- **History you can trust.** Every save, checkpoint and AI change is recorded. Compare any two versions and restore in one click.

## Try it

1. Select some text and press **Ask**, or ⌘/ to open the assistant.
2. Use the toolbar or familiar shortcuts: ⌘B, ⌘I, ⌘⌥1 for a heading.
3. Drag the markers on the ruler to change indents.
4. Press ⌘S. Documents in your Library save automatically; files from your disk save back in place.

| Feature | Docxtor | Typical web editor |
|---|---|---|
| Opens .docx directly | Yes | Often converts |
| Keeps unknown content | Yes | Usually dropped |
| Bring your own AI | Any provider | One vendor |
| Price | Free forever | Subscription |

> Documents are the most important software most people use. They deserve an instrument, not an app.

Start writing below, or open your own document.
`);
