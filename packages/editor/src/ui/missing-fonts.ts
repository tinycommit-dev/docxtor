// A strip above the pages when the document uses fonts this browser can't show and that have no
// look-alike with the same letter widths, like Photoshop's "missing fonts". It doesn't block
// anything: the text shows in a similar font until the fonts are replaced or the strip is dismissed.
// Replacing changes the document (one undo step); dismissing leaves it as it is.
import { currentLocale, t, tn } from "@docxtor/extensions";
import type { FontUse } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { fontStack } from "../editor/render";
import { fontStatus, MENU_FONTS } from "../fonts";
import { h, modal } from "./dom";

/** The fonts in `used` the browser can't show. `offered` are the fonts extensions add. */
export function missingFonts(used: FontUse[], offered: string[]): FontUse[] {
  return used.filter((u) => fontStatus(u.family, offered) === "missing");
}

function list(names: string[]): string {
  const shown = names.length > 3 ? [...names.slice(0, 2), t("{n} more", { n: names.length - 2 })] : names;
  try {
    return new Intl.ListFormat(currentLocale(), { type: "conjunction" }).format(shown);
  } catch {
    return shown.join(", ");
  }
}

/** A replacement to suggest: a common font of the same kind. */
function suggest(family: string, choices: string[]): string {
  const mono = /mono|code|courier|consol|typewriter/i.test(family);
  const serif = !mono && /serif|times|garamond|georgia|book|roman|minion|caslon|baskerville|bodoni|palatino|didot|mincho|song/i.test(family) && !/sans/i.test(family);
  const pick = mono ? "Courier New" : serif ? "Georgia" : "Arial";
  return choices.includes(pick) ? pick : (choices[0] ?? "");
}

export class MissingFontsBar {
  readonly el: HTMLElement;
  missing: FontUse[] = [];
  private dismissed = false;
  /** Taken away with its extension: a check the fonts' loading still has to run does nothing. */
  private gone = false;

  /** `offered`: fonts extensions add (they count as available and are offered as replacements). */
  constructor(
    private ed: Editor,
    private offered: () => string[],
  ) {
    this.el = h("div", { class: "fonts-strip", role: "status", hidden: true });
  }

  /** A new document: show the strip again if it needs it, once its own fonts have loaded. */
  reset() {
    this.dismissed = false;
    this.missing = [];
    this.render();
    void document.fonts.ready.then(() => this.check());
  }

  dispose() {
    this.gone = true;
    this.el.remove();
  }

  check() {
    if (this.gone) return;
    this.missing = this.ed.engine ? missingFonts(this.ed.engine.fontsUsed(), this.offered()) : [];
    this.render();
  }

  private render() {
    const n = this.missing.length;
    if (!n || this.dismissed) {
      this.el.hidden = true;
      this.el.replaceChildren();
      return;
    }
    const fonts = list(this.missing.map((m) => m.family));
    const text = tn(n, "{fonts} isn't available on this device, so Docxtor is showing a similar font.", "{n} fonts aren't available on this device: {fonts}. Docxtor is showing similar fonts instead.", { fonts });
    const actions = h("div", { class: "fonts-strip-actions" });
    if (!this.ed.readOnly) {
      const replace = h("button", { class: "btn", type: "button" }, t("Replace fonts…"));
      replace.addEventListener("click", () => this.openReplace());
      actions.append(replace);
    }
    const dismiss = h("button", { class: "btn ghost", type: "button" }, t("Dismiss"));
    dismiss.addEventListener("click", () => {
      this.dismissed = true;
      this.render();
      this.ed.focus();
    });
    actions.append(dismiss);
    this.el.replaceChildren(h("p", {}, text), actions);
    this.el.hidden = false;
  }

  /** The dialog: one row per missing font, each with the font to use instead. */
  openReplace() {
    if (!this.missing.length || this.ed.readOnly) return;
    const missing = new Set(this.missing.map((m) => m.family.toLowerCase()));
    const usable = (f: string) => !missing.has(f.toLowerCase());
    const common = MENU_FONTS.filter(usable);
    const more = [...new Set(this.offered())].filter((f) => usable(f) && !MENU_FONTS.includes(f)).sort((a, b) => a.localeCompare(b));
    const choices = [...common, ...more];

    const rows = this.missing.map((m, i) => {
      const id = `mf-${i}`;
      const select = h("select", { id });
      select.append(h("option", { value: "" }, t("Keep as it is")));
      const group = (label: string, fonts: string[]) => {
        const g = h("optgroup", { label });
        for (const f of fonts) g.append(h("option", { value: f }, f));
        select.append(g);
      };
      group(t("Fonts"), common);
      if (more.length) group(t("More fonts"), more);
      select.value = suggest(m.family, choices);
      const name = h("label", { for: id, class: "mf-name" }, h("span", { style: `font-family:${fontStack(m.family)}` }, m.family), h("span", { class: "mf-use" }, tn(m.chars, "{n} character", "{n} characters")));
      return { family: m.family, select, el: h("div", { class: "mf-row" }, name, select) };
    });

    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const apply = h("button", { class: "btn primary", type: "button" }, t("Replace"));
    const dialog = h(
      "div",
      { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Replace missing fonts") },
      h("div", { class: "dialog-head" }, h("h2", {}, t("Replace missing fonts")), h("p", { class: "sub" }, t("Text in each font changes to the one you pick, everywhere in the document. Undo puts it back."))),
      h("div", { class: "dialog-body mf-rows" }, ...rows.map((r) => r.el)),
      h("div", { class: "dialog-actions" }, cancel, apply),
    );
    const close = modal(dialog);
    cancel.addEventListener("click", close);
    apply.addEventListener("click", () => {
      const ops = rows.filter((r) => r.select.value).map((r) => ({ op: "replace_font", from: r.family, to: r.select.value }));
      close();
      if (ops.length) this.ed.exec(ops);
      this.check();
      this.ed.focus();
    });
    rows[0]?.select.focus();
  }
}
