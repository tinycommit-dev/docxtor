// Tools > Compare documents and Combine documents: the open document against another version of it, paragraph by
// paragraph, with the words that were added and removed marked, and the formatting that changed named. Compare treats the
// open document as the newer one (Swap turns it round) and can save the comparison as a document with Word's tracked
// changes. Combine treats the open document as the original and the one you pick as the revised version: the saved
// document is the revised one, with what differs from the original marked, and the changes it already had kept.
//
// How the two are compared is not settled here: the app hands in functions (the comparer an extension registered).
import { h, msg, svg, t, tn, toast, I, modal, type CompareBlock } from "@docxtor/editor";
import { DOC_EXT } from "../files/fs";

export interface CompareSource {
  name: string;
  bytes: Uint8Array;
}

export interface CompareOptions {
  /** `compare` (the default) or `combine`. */
  mode?: "compare" | "combine";
  /** The open document. */
  current: CompareSource;
  /** The version to compare it with. */
  other: CompareSource;
  /** The name tracked changes carry in Word. */
  author: string;
  /** The paragraphs of the newer document against the older one. */
  compare(newer: CompareSource, older: CompareSource): CompareBlock[] | Promise<CompareBlock[]>;
  /** The document with tracked changes: the newer one against the older (Compare), or the revised one against the original (Combine). */
  makeDocument(newer: CompareSource, older: CompareSource, author: string): Uint8Array | Promise<Uint8Array>;
  /** Open the tracked-changes document (bytes, file name). */
  openRedline(bytes: Uint8Array, name: string): Promise<void>;
  /** Called however the dialog ends. */
  onClose?(): void;
}

const KIND_LABEL: Record<CompareBlock["kind"], string> = {
  same: "",
  added: msg("Added"),
  removed: msg("Removed"),
  changed: msg("Edited"),
  formatted: msg("Formatted"),
};

/** What a changed property is called, in words. Properties not listed here read as "formatting". */
const FORMAT_NAMES: Record<string, string> = {
  bold: msg("bold"),
  italic: msg("italic"),
  underline: msg("underline"),
  underlineColor: msg("underline"),
  strike: msg("strikethrough"),
  dstrike: msg("double strikethrough"),
  caps: msg("all caps"),
  smallCaps: msg("small caps"),
  vanish: msg("hidden text"),
  font: msg("font"),
  fontTheme: msg("font"),
  size: msg("size"),
  color: msg("font color"),
  colorTheme: msg("font color"),
  highlight: msg("highlight"),
  shading: msg("shading"),
  shd: msg("shading"),
  vertAlign: msg("superscript or subscript"),
  spacing: msg("character spacing"),
  position: msg("raised or lowered text"),
  scale: msg("character width"),
  kern: msg("kerning"),
  outline: msg("outline"),
  shadow: msg("shadow"),
  emboss: msg("emboss"),
  imprint: msg("engrave"),
  border: msg("text border"),
  em: msg("emphasis mark"),
  noProof: msg("spelling and grammar checking"),
  fx: msg("text effects"),
  rtl: msg("text direction"),
  bidi: msg("text direction"),
  style: msg("style"),
  align: msg("alignment"),
  indLeft: msg("indentation"),
  indRight: msg("indentation"),
  indFirstLine: msg("indentation"),
  indHanging: msg("indentation"),
  indLeftChars: msg("indentation"),
  indRightChars: msg("indentation"),
  indFirstLineChars: msg("indentation"),
  indHangingChars: msg("indentation"),
  mirrorIndents: msg("indentation"),
  spaceBefore: msg("spacing"),
  spaceAfter: msg("spacing"),
  spaceBeforeLines: msg("spacing"),
  spaceAfterLines: msg("spacing"),
  beforeAuto: msg("spacing"),
  afterAuto: msg("spacing"),
  contextualSpacing: msg("spacing"),
  line: msg("line spacing"),
  lineRule: msg("line spacing"),
  numId: msg("list"),
  ilvl: msg("list"),
  outlineLvl: msg("outline level"),
  keepNext: msg("page breaks"),
  keepLines: msg("page breaks"),
  pageBreakBefore: msg("page breaks"),
  widowControl: msg("page breaks"),
  borders: msg("borders"),
  tabs: msg("tab stops"),
  frame: msg("frame"),
};

/** The names of what changed, once each, in words. */
export function formatNames(names: string[]): string[] {
  const out: string[] = [];
  for (const n of names) {
    const label = t(FORMAT_NAMES[n] ?? msg("formatting"));
    if (!out.includes(label)) out.push(label);
  }
  return out;
}

const stem = (name: string) => name.replace(DOC_EXT, "");

/** Runs of neighbouring changed paragraphs count as one change, the way Word counts revisions. */
function groupChanges(blocks: CompareBlock[]): number[][] {
  const groups: number[][] = [];
  let run: number[] | null = null;
  blocks.forEach((b, i) => {
    if (b.kind === "same") run = null;
    else if (run) run.push(i);
    else groups.push((run = [i]));
  });
  return groups;
}

/** What formatting changed in a paragraph, for the line under it: the properties of its words and of the paragraph. */
function formatNote(b: CompareBlock): string {
  const words = formatNames(b.parts.filter((p) => p.t === "fmt").flatMap((p) => p.fmt ?? []));
  const para = formatNames(b.paraFmt ?? []);
  const lines: string[] = [];
  if (words.length) lines.push(t("Formatting changed: {what}", { what: words.join(", ") }));
  if (para.length) lines.push(t("Paragraph formatting changed: {what}", { what: para.join(", ") }));
  return lines.join(" ");
}

export function openCompare(o: CompareOptions) {
  const combine = o.mode === "combine";
  let swapped = false;
  let changesOnly = false;
  let blocks: CompareBlock[] = [];
  let groups: number[][] = [];
  let at = -1;
  let rows: HTMLElement[] = [];
  let turn = 0;
  let author = o.author;

  // Combine: the one picked is the revised version, whatever the person does. Compare: the open document is the newer one.
  const newer = () => (combine ? o.other : swapped ? o.other : o.current);
  const older = () => (combine ? o.current : swapped ? o.current : o.other);

  const sub = h("p", { class: "sub" });
  const count = h("span", { class: "cmp-count", role: "status" });
  const position = h("span", { class: "cmp-pos", "aria-live": "polite" });
  const prev = h("button", { class: "btn sm", type: "button" }, svg(I.chevron), t("Previous change"));
  const next = h("button", { class: "btn sm", type: "button" }, svg(I.chevron), t("Next change"));
  prev.classList.add("up");
  const swap = h("button", { class: "btn sm", type: "button", "aria-pressed": "false", title: t("Compare the other way round: the other document counts as the newer one") }, t("Swap"));
  const onlyBox = h("input", { type: "checkbox", id: "cmp-only" });
  const only = h("label", { class: "check cmp-only", for: "cmp-only" }, onlyBox, h("span", {}, t("Changes only")));
  const who = h("input", { type: "text", id: "cmp-author", class: "cmp-author", value: author, autocomplete: "off", spellcheck: "false" });
  const whoLabel = h("label", { class: "cmp-who", for: "cmp-author" }, h("span", {}, combine ? t("Label unmarked changes with") : t("Mark changes as")), who);
  const list = h("div", { class: "cmp-list", role: "list", tabindex: "0", "aria-label": t("Paragraphs, with changes marked") });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Close"));
  const tracked = h("button", { class: "btn primary", type: "button" }, combine ? t("Open the combined document") : t("Open as a document with tracked changes"));
  const title = combine ? t("Combine documents") : t("Compare documents");
  const dialog = h(
    "div",
    { class: "dialog cmp-dialog", role: "dialog", "aria-modal": "true", "aria-label": title },
    h("div", { class: "dialog-head" }, h("h2", {}, title), sub),
    h("div", { class: "cmp-bar" }, h("div", { class: "cmp-status" }, count, position), h("div", { class: "cmp-nav" }, prev, next, combine ? null : swap, only, whoLabel)),
    h("div", { class: "cmp-body" }, list),
    h("div", { class: "dialog-actions" }, cancel, tracked),
  );

  const part = (p: CompareBlock["parts"][number]) => {
    if (p.t === "same") return document.createTextNode(p.text);
    if (p.t === "fmt") {
      const el = h("span", { class: "fmt", title: t("Formatting changed: {what}", { what: formatNames(p.fmt ?? []).join(", ") }) }, p.text);
      return el;
    }
    return h(p.t, {}, p.text);
  };

  async function render() {
    const nw = newer();
    const od = older();
    sub.textContent = combine
      ? t("{revised} is the revised version and {original} the original. What differs is marked as changes by the name below. Tracked changes in the revised version keep their authors.", { revised: `“${stem(nw.name)}”`, original: `“${stem(od.name)}”` })
      : t("{newer} is the newer version and {older} the older one. Added words are underlined, removed words crossed out, and words with other formatting are dotted underlined.", { newer: `“${stem(nw.name)}”`, older: `“${stem(od.name)}”` });
    const mine = ++turn;
    let result: CompareBlock[];
    try {
      result = await o.compare(nw, od);
    } catch (e) {
      if (mine !== turn) return;
      toast(t("Couldn't compare these documents: {reason}", { reason: (e as Error).message }), 5000);
      blocks = [];
      groups = [];
      list.replaceChildren(h("div", { class: "empty-note" }, t("These documents couldn't be compared.")));
      count.textContent = "";
      position.textContent = "";
      tracked.disabled = true;
      return;
    }
    if (mine !== turn) return;
    tracked.disabled = false;
    blocks = result;
    groups = groupChanges(blocks);
    at = -1;
    rows = [];
    const shown = changesOnly ? blocks.map((b, i) => [b, i] as const).filter(([b]) => b.kind !== "same") : blocks.map((b, i) => [b, i] as const);
    const group = new Map<number, number>();
    groups.forEach((g, gi) => g.forEach((i) => group.set(i, gi)));
    list.replaceChildren(
      ...(blocks.length
        ? shown.map(([b, i]) => {
            const gi = group.get(i);
            const label = KIND_LABEL[b.kind];
            const body = b.parts.length ? b.parts.map(part) : [b.kind === "same" ? " " : "¶"];
            const note = formatNote(b);
            const row = h(
              "div",
              { class: `cmp-row ${b.kind}${b.table ? " table" : ""}`, role: "listitem", "data-kind": b.kind, "data-change": gi === undefined ? undefined : String(gi) },
              h("span", { class: "cmp-tag" }, label ? t(label) : ""),
              h("span", { class: "cmp-text" }, ...body, note ? h("span", { class: "cmp-note" }, note) : null),
            );
            rows.push(row);
            return row;
          })
        : [h("div", { class: "empty-note" }, t("Both documents are empty."))]),
    );
    if (blocks.length && !groups.length) list.prepend(h("div", { class: "cmp-same", role: "status" }, t("No differences. The two documents read the same and look the same.")));
    count.textContent = groups.length ? tn(groups.length, "{n} change", "{n} changes") : t("No changes");
    position.textContent = "";
    prev.disabled = next.disabled = !groups.length;
    list.scrollTop = 0;
  }

  /** Move to change `to` (wrapping), mark its paragraphs and scroll to the first. */
  function go(to: number) {
    if (!groups.length) return;
    at = (to + groups.length) % groups.length;
    for (const r of rows) r.classList.remove("current");
    const mine = rows.filter((r) => r.dataset.change === String(at));
    for (const r of mine) r.classList.add("current");
    mine[0]?.scrollIntoView({ block: "center" });
    position.textContent = t("Change {n} of {total}", { n: at + 1, total: groups.length });
  }

  prev.addEventListener("click", () => go(at < 0 ? groups.length - 1 : at - 1));
  next.addEventListener("click", () => go(at + 1));
  swap.addEventListener("click", () => {
    swapped = !swapped;
    swap.setAttribute("aria-pressed", String(swapped));
    swap.classList.toggle("on", swapped);
    void render();
  });
  onlyBox.addEventListener("change", () => {
    changesOnly = onlyBox.checked;
    void render();
  });
  who.addEventListener("input", () => (author = who.value));
  tracked.addEventListener("click", async () => {
    tracked.disabled = true;
    try {
      const bytes = await o.makeDocument(newer(), older(), author.trim() || o.author);
      const name = combine ? t("Combination of {a} and {b}", { a: stem(o.current.name), b: stem(o.other.name) }) : t("Comparison of {a} and {b}", { a: stem(o.current.name), b: stem(o.other.name) });
      close();
      await o.openRedline(bytes, `${name}.docx`);
    } catch (e) {
      tracked.disabled = false;
      toast(combine ? t("Couldn't combine these documents: {reason}", { reason: (e as Error).message }) : t("Couldn't make the tracked-changes copy: {reason}", { reason: (e as Error).message }), 5000);
    }
  });

  const close = modal(dialog, o.onClose);
  cancel.addEventListener("click", close);
  void render().then(() => next.focus());
}
