// The Format menu of the find bar: the formatting text must have to be found, and the formatting the replacement
// gets, set in small dialogs for font, paragraph, style, highlight and language, as Word's Find and Replace does.
// "Find formatting" is a FindFormat (the engine matches runs and paragraphs against it); "Replace formatting" is the
// run and paragraph patches and the language of the engine's `replace_text` op.
import type { FindFormat, ParaPatch, RunPatch, StyleInfo } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { h, modal } from "./dom";
import { languageChoices, languageName } from "./language";

export type FormatKind = "font" | "paragraph" | "style" | "highlight" | "language";
export type FormatSide = "find" | "replace";

/** All the formatting of the find bar. */
export interface FormatSet {
  find: FindFormat;
  run: RunPatch;
  para: ParaPatch;
  lang?: string;
}

export const emptyFormats = (): FormatSet => ({ find: {}, run: {}, para: {} });

export const HIGHLIGHT_NAMES: [string, string][] = [
  ["yellow", msg("Yellow")],
  ["green", msg("Bright green")],
  ["cyan", msg("Turquoise")],
  ["magenta", msg("Pink")],
  ["blue", msg("Blue")],
  ["red", msg("Red")],
  ["darkBlue", msg("Dark blue")],
  ["darkCyan", msg("Teal")],
  ["darkGreen", msg("Green")],
  ["darkMagenta", msg("Violet")],
  ["darkRed", msg("Dark red")],
  ["darkYellow", msg("Dark yellow")],
  ["darkGray", msg("Dark gray")],
  ["lightGray", msg("Light gray")],
  ["black", msg("Black")],
];

const highlightName = (id: string) => {
  const name = HIGHLIGHT_NAMES.find(([n]) => n === id)?.[1];
  return name ? t(name) : id;
};

const hex = (v: string) => v.trim().replace(/^#/, "").toUpperCase();
const isHex = (v: string) => /^[0-9A-F]{6}$/.test(v);

/** A tri-state: leave it alone (or "any" when finding), on, off. */
type Tri = "" | "on" | "off";
const triOf = (v: boolean | undefined): Tri => (v === undefined ? "" : v ? "on" : "off");
const boolOf = (v: string): boolean | undefined => (v === "on" ? true : v === "off" ? false : undefined);

function triSelect(label: string, value: Tri, side: FormatSide) {
  const sel = h("select", { "aria-label": label }, h("option", { value: "" }, side === "find" ? t("Any") : t("Leave as it is")), h("option", { value: "on" }, t("On")), h("option", { value: "off" }, t("Off")));
  sel.value = value;
  return { label, sel };
}

function field(label: string, control: HTMLElement) {
  return h("div", { class: "field", style: "margin:0" }, h("label", {}, label), control);
}

/** Styles Docxtor can add to a file when they are asked for, shown even before the file has them. */
const BUILT_IN: Record<"paragraph" | "character", [string, string][]> = {
  paragraph: [["Title", "Title"], ["Subtitle", "Subtitle"], ["Heading1", "Heading 1"], ["Heading2", "Heading 2"], ["Heading3", "Heading 3"], ["Heading4", "Heading 4"], ["Heading5", "Heading 5"], ["Heading6", "Heading 6"], ["Quote", "Quote"], ["ListParagraph", "List Paragraph"]],
  character: [["Emphasis", "Emphasis"], ["Strong", "Strong"]],
};

/** The styles a dialog offers: the ones the document shows in its gallery plus the quick ones, then the built-in ones it lacks. */
function usable(styles: StyleInfo[], kind: "paragraph" | "character"): { id: string; name: string }[] {
  const own = styles.filter((s) => s.kind === kind && (!s.hidden || s.quick));
  const have = new Set(styles.flatMap((s) => [s.id.toLowerCase(), s.name.toLowerCase()]));
  const extra = BUILT_IN[kind].filter(([id, name]) => !have.has(id.toLowerCase()) && !have.has(name.toLowerCase())).map(([id, name]) => ({ id, name }));
  return [...own.map((s) => ({ id: s.id, name: s.name })), ...extra].sort((a, b) => a.name.localeCompare(b.name));
}

/** What the formatting says in a few words, for the line under the field. */
export function describeFind(f: FindFormat, styles: StyleInfo[], lang?: string): string {
  const out: string[] = [];
  const on = (v: boolean | undefined, yes: string, no: string) => v !== undefined && out.push(v ? yes : no);
  on(f.bold, t("Bold"), t("Not bold"));
  on(f.italic, t("Italic"), t("Not italic"));
  on(f.underline, t("Underline"), t("No underline"));
  on(f.strike, t("Strikethrough"), t("No strikethrough"));
  on(f.smallCaps, t("Small caps"), t("No small caps"));
  on(f.caps, t("All caps"), t("No all caps"));
  if (f.vertAlign) out.push(f.vertAlign === "superscript" ? t("Superscript") : f.vertAlign === "subscript" ? t("Subscript") : t("Baseline"));
  if (f.font) out.push(f.font);
  if (f.size) out.push(t("{n} pt", { n: f.size }));
  if (f.color) out.push(f.color.toLowerCase() === "auto" ? t("Automatic colour") : `#${f.color}`);
  if (f.highlight) out.push(f.highlight === "none" ? t("No highlight") : t("{name} highlight", { name: highlightName(f.highlight) }));
  if (f.style !== undefined) out.push(f.style ? t("Style {name}", { name: styles.find((s) => s.id === f.style)?.name ?? f.style }) : t("No character style"));
  if (f.paraStyle !== undefined) out.push(t("Style {name}", { name: styles.find((s) => s.id === f.paraStyle)?.name ?? f.paraStyle }));
  if (f.align) out.push(t("Aligned {side}", { side: alignName(f.align) }));
  if (f.lang ?? lang) out.push(languageName((f.lang ?? lang)!));
  return out.join(", ");
}

const alignName = (a: string) => (a === "center" ? t("centre") : a === "right" ? t("right") : a === "justify" ? t("justified") : t("left"));

export function describeReplace(run: RunPatch, para: ParaPatch, lang: string | undefined, styles: StyleInfo[]): string {
  const out: string[] = [];
  const on = (v: boolean | undefined, yes: string, no: string) => v !== undefined && out.push(v ? yes : no);
  on(run.bold, t("Bold"), t("Not bold"));
  on(run.italic, t("Italic"), t("Not italic"));
  if (run.underline !== undefined) out.push(run.underline === "none" ? t("No underline") : t("Underline"));
  on(run.strike, t("Strikethrough"), t("No strikethrough"));
  on(run.smallCaps, t("Small caps"), t("No small caps"));
  on(run.caps, t("All caps"), t("No all caps"));
  if (run.vertAlign) out.push(run.vertAlign === "superscript" ? t("Superscript") : run.vertAlign === "subscript" ? t("Subscript") : t("Baseline"));
  if (run.font) out.push(run.font);
  if (run.size) out.push(t("{n} pt", { n: run.size }));
  if (run.color) out.push(run.color.toLowerCase() === "auto" ? t("Automatic colour") : `#${run.color}`);
  if (run.highlight) out.push(run.highlight === "none" ? t("No highlight") : t("{name} highlight", { name: highlightName(run.highlight) }));
  if (run.style) out.push(t("Style {name}", { name: styles.find((s) => s.id === run.style)?.name ?? run.style }));
  if (para.style) out.push(t("Style {name}", { name: styles.find((s) => s.id === para.style)?.name ?? para.style }));
  if (para.align) out.push(t("Aligned {side}", { side: alignName(para.align) }));
  if (para.spaceBefore !== undefined) out.push(t("{n} pt before", { n: para.spaceBefore / 20 }));
  if (para.spaceAfter !== undefined) out.push(t("{n} pt after", { n: para.spaceAfter / 20 }));
  if (para.lineMultiple !== undefined) out.push(t("Line spacing {n}", { n: para.lineMultiple }));
  if (lang) out.push(languageName(lang));
  return out.join(", ");
}

export function hasFormats(f: FormatSet, side: FormatSide): boolean {
  if (side === "find") return Object.keys(f.find).length > 0;
  return Object.keys(f.run).length > 0 || Object.keys(f.para).length > 0 || !!f.lang;
}

const TITLES: Record<FormatKind, [string, string]> = {
  font: [msg("Find font"), msg("Replace with font")],
  paragraph: [msg("Find paragraph"), msg("Replace with paragraph")],
  style: [msg("Find style"), msg("Replace with style")],
  highlight: [msg("Find highlight"), msg("Replace with highlight")],
  language: [msg("Find language"), msg("Replace with language")],
};

/**
 * Ask for one kind of formatting. `done` gets the changed set; a field left on its first choice means "no
 * condition" when finding and "leave it" when replacing.
 */
export function openFormatDialog(kind: FormatKind, side: FormatSide, current: FormatSet, styles: StyleInfo[], done: (next: FormatSet) => void) {
  const f = side === "find" ? current.find : {};
  const r = current.run;
  const p = current.para;
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const clear = h("button", { class: "btn ghost", type: "button" }, t("Clear"));
  const title = t(TITLES[kind][side === "find" ? 0 : 1]);
  const body = h("div", { class: "dialog-body" });
  let collect: (next: FormatSet) => void = () => {};

  if (kind === "font") {
    const val = <K extends keyof FindFormat>(k: K) => (side === "find" ? f[k] : (r as Record<string, unknown>)[k as string]);
    const flags: { key: "bold" | "italic" | "strike" | "smallCaps" | "caps"; label: string }[] = [
      { key: "bold", label: t("Bold") },
      { key: "italic", label: t("Italic") },
      { key: "strike", label: t("Strikethrough") },
      { key: "smallCaps", label: t("Small caps") },
      { key: "caps", label: t("All caps") },
    ];
    const tris = flags.map((x) => ({ ...x, ...triSelect(x.label, triOf(val(x.key) as boolean | undefined), side) }));
    const under = triSelect(t("Underline"), side === "find" ? triOf(f.underline) : r.underline === undefined ? "" : r.underline === "none" ? "off" : "on", side);
    const vert = h(
      "select",
      { "aria-label": t("Position") },
      h("option", { value: "" }, side === "find" ? t("Any") : t("Leave as it is")),
      h("option", { value: "superscript" }, t("Superscript")),
      h("option", { value: "subscript" }, t("Subscript")),
      h("option", { value: "baseline" }, t("Normal")),
    );
    vert.value = (val("vertAlign") as string | undefined) ?? "";
    const font = h("input", { type: "text", value: (val("font") as string | undefined) ?? "", placeholder: side === "find" ? t("Any font") : t("Leave as it is"), "aria-label": t("Font"), spellcheck: "false" });
    const size = h("input", { type: "number", min: "1", max: "400", step: "0.5", value: String(val("size") ?? ""), placeholder: side === "find" ? t("Any size") : t("Leave as it is"), "aria-label": t("Size (points)") });
    const colorVal = (val("color") as string | undefined) ?? "";
    const color = h("input", { type: "text", value: colorVal ? (colorVal.toLowerCase() === "auto" ? "auto" : `#${colorVal}`) : "", placeholder: side === "find" ? t("Any colour, like #ff0000") : t("Leave as it is, or #ff0000"), "aria-label": t("Colour"), spellcheck: "false" });
    const error = h("p", { class: "dialog-error", role: "alert", hidden: true });
    body.append(
      h("div", { class: "ff-grid" }, field(t("Font"), font), field(t("Size (points)"), size), field(t("Colour"), color), field(t("Position"), vert)),
      h("div", { class: "ff-grid" }, ...[...tris, { ...under, key: "underline" }].map((x) => field(x.label, x.sel))),
      error,
    );
    collect = (next) => {
      const target = (side === "find" ? next.find : next.run) as Record<string, unknown>;
      for (const x of tris) {
        const v = boolOf(x.sel.value);
        if (v === undefined) delete target[x.key];
        else target[x.key] = v;
      }
      const u = boolOf(under.sel.value);
      if (side === "find") {
        if (u === undefined) delete next.find.underline;
        else next.find.underline = u;
      } else if (u === undefined) delete next.run.underline;
      else next.run.underline = u ? "single" : "none";
      for (const [k, v] of [
        ["vertAlign", vert.value],
        ["font", font.value.trim()],
      ] as const) {
        if (v) target[k] = v;
        else delete target[k];
      }
      const n = Number(size.value);
      if (n > 0) target.size = n;
      else delete target.size;
      const c = color.value.trim();
      if (!c) delete target.color;
      else target.color = c.toLowerCase() === "auto" ? "auto" : hex(c);
    };
    const check = () => {
      const c = color.value.trim();
      const bad = !!c && c.toLowerCase() !== "auto" && !isHex(hex(c));
      error.hidden = !bad;
      error.textContent = bad ? t("A colour is six hex digits, like #ff0000, or auto.") : "";
      ok.disabled = bad;
    };
    color.addEventListener("input", check);
  } else if (kind === "paragraph") {
    const align = h(
      "select",
      { "aria-label": t("Alignment") },
      h("option", { value: "" }, side === "find" ? t("Any") : t("Leave as it is")),
      h("option", { value: "left" }, t("Left")),
      h("option", { value: "center" }, t("Centre")),
      h("option", { value: "right" }, t("Right")),
      h("option", { value: "justify" }, t("Justified")),
    );
    align.value = (side === "find" ? f.align : p.align) ?? "";
    const before = h("input", { type: "number", min: "0", step: "1", value: p.spaceBefore === undefined ? "" : String(p.spaceBefore / 20), placeholder: t("Leave as it is"), "aria-label": t("Space before (points)") });
    const after = h("input", { type: "number", min: "0", step: "1", value: p.spaceAfter === undefined ? "" : String(p.spaceAfter / 20), placeholder: t("Leave as it is"), "aria-label": t("Space after (points)") });
    const line = h(
      "select",
      { "aria-label": t("Line spacing") },
      h("option", { value: "" }, t("Leave as it is")),
      h("option", { value: "1" }, t("Single")),
      h("option", { value: "1.15" }, "1.15"),
      h("option", { value: "1.5" }, "1.5"),
      h("option", { value: "2" }, t("Double")),
    );
    line.value = p.lineMultiple === undefined ? "" : String(p.lineMultiple);
    body.append(h("div", { class: "ff-grid" }, field(t("Alignment"), align), ...(side === "replace" ? [field(t("Space before (points)"), before), field(t("Space after (points)"), after), field(t("Line spacing"), line)] : [])));
    collect = (next) => {
      if (side === "find") {
        if (align.value) next.find.align = align.value;
        else delete next.find.align;
        return;
      }
      if (align.value) next.para.align = align.value;
      else delete next.para.align;
      const num = (el: HTMLInputElement, k: "spaceBefore" | "spaceAfter") => {
        if (el.value.trim() === "") delete next.para[k];
        else next.para[k] = Math.max(0, Math.round(Number(el.value) * 20));
      };
      num(before, "spaceBefore");
      num(after, "spaceAfter");
      if (line.value) next.para.lineMultiple = Number(line.value);
      else delete next.para.lineMultiple;
    };
  } else if (kind === "style") {
    const paras = usable(styles, "paragraph");
    const chars = usable(styles, "character");
    const pick = h("select", { "aria-label": t("Style") }, h("option", { value: "" }, side === "find" ? t("Any style") : t("Leave as it is")));
    if (paras.length) pick.append(h("optgroup", { label: t("Paragraph styles") }, ...paras.map((s) => h("option", { value: `p:${s.id}` }, s.name))));
    if (chars.length) pick.append(h("optgroup", { label: t("Character styles") }, ...chars.map((s) => h("option", { value: `c:${s.id}` }, s.name))));
    const was = side === "find" ? (f.paraStyle ? `p:${f.paraStyle}` : f.style ? `c:${f.style}` : "") : p.style ? `p:${p.style}` : r.style ? `c:${r.style}` : "";
    pick.value = was;
    body.append(field(t("Style"), pick));
    collect = (next) => {
      const [k, id] = pick.value.split(":");
      if (side === "find") {
        delete next.find.paraStyle;
        delete next.find.style;
        if (k === "p") next.find.paraStyle = id;
        if (k === "c") next.find.style = id;
      } else {
        delete next.para.style;
        delete next.run.style;
        if (k === "p") next.para.style = id;
        if (k === "c") next.run.style = id;
      }
    };
  } else if (kind === "highlight") {
    const pick = h(
      "select",
      { "aria-label": t("Highlight") },
      h("option", { value: "" }, side === "find" ? t("Any") : t("Leave as it is")),
      h("option", { value: "none" }, t("No highlight")),
      ...HIGHLIGHT_NAMES.map(([id, name]) => h("option", { value: id }, t(name))),
    );
    pick.value = (side === "find" ? f.highlight : r.highlight) ?? "";
    body.append(field(t("Highlight"), pick));
    collect = (next) => {
      const target = (side === "find" ? next.find : next.run) as Record<string, unknown>;
      if (pick.value) target.highlight = pick.value;
      else delete target.highlight;
    };
  } else {
    const was = side === "find" ? f.lang : current.lang;
    const pick = h("select", { "aria-label": t("Language") }, h("option", { value: "" }, side === "find" ? t("Any language") : t("Leave as it is")), ...languageChoices(was).map((c) => h("option", { value: c.tag }, c.label)));
    pick.value = was ?? "";
    body.append(field(t("Language"), pick));
    collect = (next) => {
      if (side === "find") {
        if (pick.value) next.find.lang = pick.value;
        else delete next.find.lang;
      } else next.lang = pick.value || undefined;
    };
  }

  const dialog = h(
    "div",
    { class: "dialog ff-dialog", role: "dialog", "aria-modal": "true", "aria-label": title, style: "width:min(var(--dialog-m),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, title), h("p", { class: "sub" }, side === "find" ? t("Only text that has all of this is found.") : t("Put on the replacement, or on the found text when the replacement is empty."))),
    body,
    h("div", { class: "dialog-actions" }, clear, cancel, ok),
  );
  const close = modal(dialog);
  const finish = (reset: boolean) => {
    const next: FormatSet = { find: { ...current.find }, run: { ...current.run }, para: { ...current.para }, lang: current.lang };
    if (reset) {
      // Clear only the settings this dialog owns.
      body.querySelectorAll("select").forEach((s) => ((s as HTMLSelectElement).value = ""));
      body.querySelectorAll("input").forEach((i) => ((i as HTMLInputElement).value = ""));
    }
    collect(next);
    close();
    done(next);
  };
  ok.addEventListener("click", () => finish(false));
  clear.addEventListener("click", () => finish(true));
  cancel.addEventListener("click", close);
  dialog.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !ok.disabled && e.target instanceof HTMLInputElement) {
      e.preventDefault();
      finish(false);
    }
  });
}
