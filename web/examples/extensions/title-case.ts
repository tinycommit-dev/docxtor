// Example extension: Title Case for the selection, with a toolbar button and a shortcut.
// It edits through ops, so the change is one undo step like any other.
import { defineExtension } from "@docxtor/extensions";

const SMALL = new Set(["a", "an", "and", "as", "at", "but", "by", "for", "in", "nor", "of", "on", "or", "the", "to", "up", "via", "vs"]);

export function titleCase(text: string): string {
  const words = text.split(/(\s+)/);
  const last = words.length - 1 - [...words].reverse().findIndex((w) => w.trim());
  return words
    .map((w, i) => {
      if (!w.trim()) return w;
      const lower = w.toLowerCase();
      if (i !== 0 && i !== last && SMALL.has(lower)) return lower;
      // Leave words with capitals inside them alone: iPhone, McDonald, NASA.
      if (/[A-Z]/.test(w.slice(1))) return w;
      return lower.charAt(0).toUpperCase() + lower.slice(1);
    })
    .join("");
}

export default defineExtension({
  id: "example.title-case",
  name: "Title case",
  version: "1.0.0",
  publisher: "Docxtor examples",
  description: "Capitalise the selected text as a headline.",
  activate(ctx) {
    ctx.commands.register(
      "titleCase.apply",
      () => {
        const sel = ctx.document.selection();
        if (!sel?.text.trim()) return ctx.ui.notify("Select some text first.");
        if (sel.from.p !== sel.to.p) return ctx.ui.notify("Select text within one paragraph.");
        ctx.document.exec([
          { op: "delete", from: sel.from, to: sel.to },
          { op: "insert_text", at: sel.from, text: titleCase(sel.text) },
        ]);
        ctx.document.select(sel.from, { p: sel.from.p, o: sel.from.o + sel.text.length });
      },
      { title: "Title case", category: "Format", keybinding: "Mod+Alt+T", when: () => !!ctx.document.engine, ai: true },
    );
    ctx.toolbar.register({ id: "titleCase", command: "titleCase.apply", label: "Aa", group: "text" });
  },
});
