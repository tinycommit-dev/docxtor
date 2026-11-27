// Tools > Accessibility check: a navigator panel that lists what would trip up a screen reader or a
// reader with low vision, each with a way to go there and, where the fix is one step, a button for it.
import { h, msg, promptDialog, svg, t, tn, I, type A11yIssue, type A11yKind, type A11ySeverity } from "@docxtor/editor";
import { DOC_EXT } from "../files/fs";
import type { App } from "../main";

/** The engine's English message for each kind, so the text can be translated by kind. */
const MESSAGES: Record<A11yKind, string> = {
  missing_alt: msg("This image has no alt text. Describe it for people who can't see it, or mark it as decorative."),
  heading_skip: msg("A heading level is skipped. Use the next level down so the outline stays in order."),
  no_headings: msg("This long document has no headings. Add headings so readers can find their way around."),
  empty_heading: msg("This heading has no text. Add text or change it to a normal paragraph."),
  no_title: msg("The document has no title. Add one in the document properties."),
  table_header: msg("This table has no header row. Mark the first row as a header so it repeats on every page and screen readers announce it."),
  merged_cells: msg("This table has merged cells, which screen readers handle poorly. Split them if you can."),
  link_text: msg("This link doesn't say where it goes. Use words that describe the destination."),
  contrast: msg("This text is hard to read against its background. Use a darker or lighter colour."),
  fake_list: msg("This looks like a typed list. Use real bullets or numbering instead."),
  blank_lines: msg("Blank lines are used for spacing. Use paragraph spacing or a page break instead."),
  language: msg("No document language is set. Set one so screen readers pronounce the text correctly."),
};

/** The one-step fix each kind offers, as the button says it. */
const FIXES: Partial<Record<A11yKind, string>> = {
  missing_alt: msg("Add alt text"),
  table_header: msg("Make the first row a header"),
  no_title: msg("Add a title"),
  language: msg("Set the language"),
  fake_list: msg("Make it a list"),
  empty_heading: msg("Make it normal text"),
};

const GROUPS: { severity: A11ySeverity; title: () => string; summary: (n: number) => string }[] = [
  { severity: "error", title: () => t("Errors"), summary: (n) => tn(n, "{n} error", "{n} errors") },
  { severity: "warning", title: () => t("Warnings"), summary: (n) => tn(n, "{n} warning", "{n} warnings") },
  { severity: "tip", title: () => t("Tips"), summary: (n) => tn(n, "{n} tip", "{n} tips") },
];

/** The variable part of an issue, in words where the engine gives only a number. */
function detailText(i: A11yIssue): string {
  const d = i.detail ?? "";
  const n = Number(d);
  if (i.kind === "fake_list" && d && Number.isFinite(n)) return tn(n, "{n} item", "{n} items");
  if (i.kind === "blank_lines" && d && Number.isFinite(n)) return tn(n, "{n} blank line", "{n} blank lines");
  const words = /^(\d+) words$/.exec(d);
  if (i.kind === "no_headings" && words) return tn(Number(words[1]), "{n} word", "{n} words");
  return d;
}

/** Text that starts like a typed list item: a dash, bullet or star, or a number with a dot or bracket. */
const MARKER = /^(\s*)([-•*–]|\d{1,2}[.)])(\s+)(?=\S)/;

export class AccessibilityPanel {
  readonly el: HTMLElement;
  private timer = 0;
  private issues: A11yIssue[] | null = null;

  constructor(private app: App) {
    this.el = h("div", { class: "a11y" });
  }

  private get visible(): boolean {
    return this.app.sidebar.open && this.app.sidebar.tab === "accessibility";
  }

  /** Check again soon, if the panel is showing (called after edits). */
  schedule() {
    window.clearTimeout(this.timer);
    if (!this.visible) return;
    this.timer = window.setTimeout(() => this.update(), 600);
  }

  /** Check now. */
  update() {
    window.clearTimeout(this.timer);
    const engine = this.app.fe.engine;
    try {
      this.issues = engine ? engine.accessibility() : null;
    } catch {
      this.issues = null;
    }
    this.render();
  }

  private render() {
    const again = h("button", { class: "tb", type: "button", title: t("Check again"), "aria-label": t("Check again") }, svg(I.refresh));
    again.addEventListener("click", () => this.update());
    const head = h("div", { class: "panel-title" }, h("span", {}, t("Accessibility")), this.issues ? again : null);
    if (!this.issues) {
      this.el.replaceChildren(head, h("div", { class: "empty-note" }, t("Open a document to check it.")));
      return;
    }
    if (!this.issues.length) {
      this.el.replaceChildren(
        head,
        h(
          "div",
          { class: "a11y-clear", role: "status" },
          h("b", {}, t("All clear")),
          h("p", {}, t("The check found nothing to fix. It looks at images, headings, tables, links, colour contrast, lists, the title and the language, so read the document through as a screen reader would.")),
        ),
      );
      return;
    }
    const counts = GROUPS.map((g) => ({ g, items: this.issues!.filter((i) => i.severity === g.severity) })).filter((x) => x.items.length);
    const summary = h("p", { class: "a11y-summary", role: "status" }, counts.map((x) => x.g.summary(x.items.length)).join(", "));
    const sections = counts.map(({ g, items }) =>
      h(
        "section",
        { class: "a11y-group", "data-severity": g.severity, "aria-label": g.title() },
        h("h3", {}, g.title(), h("span", { class: "a11y-count" }, String(items.length))),
        h("ul", { class: "a11y-list" }, ...items.map((i) => this.item(i))),
      ),
    );
    this.el.replaceChildren(head, summary, ...sections);
  }

  private item(i: A11yIssue): HTMLElement {
    const msgText = MESSAGES[i.kind] ? t(MESSAGES[i.kind]) : i.message;
    const detail = detailText(i);
    const actions = h("div", { class: "a11y-actions" });
    if (i.p !== undefined) {
      const go = h("button", { class: "btn sm", type: "button" }, t("Go to"));
      go.addEventListener("click", () => void this.goTo(i));
      actions.append(go);
    }
    const fix = FIXES[i.kind];
    if (fix) {
      const b = h("button", { class: "btn sm", type: "button", disabled: this.app.fe.readOnly }, t(fix));
      b.addEventListener("click", () => void this.fix(i));
      actions.append(b);
    }
    return h("li", { class: "a11y-issue", "data-kind": i.kind }, h("p", { class: "a11y-msg" }, msgText), detail ? h("p", { class: "a11y-detail" }, detail) : null, actions.children.length ? actions : null);
  }

  /** Scroll to the problem and select it (an image gets its frame). */
  async goTo(i: A11yIssue) {
    const ed = this.app.editor;
    if (i.p === undefined) return;
    if (i.kind === "missing_alt" && i.o !== undefined && (await this.app.fe.command<boolean>("image.select", i.p, i.o).then((r) => r.status === "done" && r.value))) return;
    ed.scrollToParagraph(i.p);
    if (i.o !== undefined) {
      const length = i.kind === "link_text" ? (i.detail?.length ?? 0) : 0;
      ed.select({ anchor: { p: i.p, o: i.o }, focus: { p: i.p, o: i.o + length } });
    }
    ed.focus();
  }

  private async fix(i: A11yIssue) {
    const ed = this.app.editor;
    const engine = this.app.fe.engine;
    if (!engine || i.p === undefined && i.kind !== "no_title" && i.kind !== "language") return;
    switch (i.kind) {
      case "missing_alt":
        await this.goTo(i);
        await this.app.fe.command("image.altText");
        break;
      case "table_header":
        ed.exec({ op: "table_props", at: i.p!, header: true });
        break;
      case "no_title": {
        const stem = (engine.fileName ?? "").replace(DOC_EXT, "");
        const title = await promptDialog({ title: t("Add a title"), label: t("Title"), value: engine.docProps().title ?? stem, ok: t("Save") });
        if (title) ed.exec({ op: "set_doc_props", title });
        break;
      }
      case "language":
        await this.app.fe.command("file.language");
        break;
      case "empty_heading":
        ed.select({ anchor: { p: i.p!, o: 0 }, focus: { p: i.p!, o: 0 } });
        ed.setStyle("Normal");
        break;
      case "fake_list":
        this.makeList(i.p!, Number(i.detail) || 1);
        break;
    }
    this.update();
  }

  /** Turn the run of typed items starting at `first` into a real list, taking the typed markers out. */
  private makeList(first: number, count: number) {
    const ed = this.app.editor;
    const engine = this.app.fe.engine!;
    const order = engine.renderAll().order;
    const text = (p: number) => engine.textRange({ p, o: 0 }, { p, o: Number.MAX_SAFE_INTEGER });
    const ids: number[] = [];
    for (let k = Math.max(0, order.indexOf(first)); k < order.length && ids.length < count; k++) {
      const m = MARKER.exec(text(order[k]));
      if (!m) break;
      ids.push(order[k]);
    }
    if (!ids.length) return;
    const numbered = /^\s*\d/.test(text(ids[0]));
    ed.exec(
      ids.map((p) => {
        const m = MARKER.exec(text(p))!;
        return { op: "delete" as const, from: { p, o: 0 }, to: { p, o: m[0].length } };
      }),
    );
    ed.select({ anchor: { p: ids[0], o: 0 }, focus: { p: ids[ids.length - 1], o: 0 } });
    ed.toggleList(numbered ? "decimal" : "bullet");
  }
}
