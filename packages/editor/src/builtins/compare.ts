// Docxtor's own way of comparing two versions of a document, as a built-in extension: the engine tells the words,
// paragraphs and formatting apart, writes the differences as Word's tracked changes, and combines a revised version
// with its original. It is one contribution to `ctx.comparers`; the app's Compare and Combine dialogs use the latest
// comparer that can do the job, so another extension can bring a different way of comparing.
import { Engine } from "@docxtor/core";
import { defineExtension, msg, type Extension } from "@docxtor/extensions";
import { REVIEW_ICONS } from "../ui/review-icons";

/** Run something with an engine opened on some .docx bytes, and let it go again. */
function withEngine<T>(bytes: Uint8Array, use: (engine: Engine) => T): T {
  const engine = Engine.open(bytes, "document.docx");
  try {
    return use(engine);
  } finally {
    engine.dispose();
  }
}

export function compareDocuments(): Extension {
  return defineExtension({
    id: "docxtor.compare",
    name: msg("Compare and combine"),
    description: msg("Compares two versions of a document word by word and by formatting, and combines a revised version with its original as tracked changes."),
    activate(ctx) {
      ctx.icons.register({ id: "combine", svg: REVIEW_ICONS.combine });
      ctx.comparers.register({
        id: "docxtor.compare",
        label: msg("Docxtor comparison"),
        compare: (newer, older) => withEngine(newer, (e) => e.compareView(older)),
        redline: (newer, older, author) => withEngine(newer, (e) => e.redline(older, author)),
        combine: (revised, original, author) => withEngine(revised, (e) => e.combine(original, author)),
      });
    },
  });
}
