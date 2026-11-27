// Define new bullet, Define new number format and Define new multilevel list: one dialog in three shapes.
// A bullet is any character (or a picture) in any font; a number format has a style, the text around the
// number, where it starts and how it looks; a multilevel list sets each of nine levels, with the numbers of
// earlier levels in the text (1.1.1), restart rules, legal numbering and a heading style linked to the level.
// The result is a `define_list` op: a new definition, written as Word writes it.
import type { LevelDef, ListDef } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { METRIC, UNIT_TW } from "../editor/ruler";
import { fontStack } from "../editor/render";
import { MENU_FONTS } from "../fonts";
import { h, modal } from "./dom";
import { openSpecialCharacters } from "./special-chars";
import { swatchGrid } from "./swatches";
import { TEXT_COLORS } from "./toolbar";
import { hex } from "../editor/color.ts";

export type DefineMode = "bullet" | "number" | "multilevel";

/** The number styles, grouped by script. Each shows the first three numbers it prints, from the engine. */
const STYLE_GROUPS: { title: string | null; formats: string[] }[] = [
  {
    title: null,
    formats: [
      "decimal", "decimalZero", "upperRoman", "lowerRoman", "upperLetter", "lowerLetter", "ordinal", "cardinalText", "ordinalText", "hex", "chicago", "numberInDash", "decimalEnclosedCircle",
      "decimalEnclosedFullstop", "decimalEnclosedParen", "decimalFullWidth",
    ],
  },
  { title: msg("Japanese"), formats: ["japaneseCounting", "japaneseDigitalTenThousand", "aiueo", "iroha"] },
  { title: msg("Chinese"), formats: ["chineseCounting", "chineseCountingThousand", "chineseLegalSimplified", "ideographDigital", "ideographTraditional", "ideographZodiac"] },
  { title: msg("Korean"), formats: ["koreanDigital", "koreanCounting", "ganada", "chosung"] },
  { title: msg("Hebrew and Arabic"), formats: ["hebrew1", "hebrew2", "arabicAlpha", "arabicAbjad"] },
  { title: msg("Hindi and Thai"), formats: ["hindiVowels", "hindiConsonants", "hindiNumbers", "thaiLetters", "thaiNumbers", "thaiCounting"] },
  { title: msg("Vietnamese and Russian"), formats: ["vietnameseCounting", "russianLower", "russianUpper"] },
];

/** Bullets people reach for, one click each; any other character goes in the box beside them. avoid-ai-design-ignore: I2 (bullet characters of the document, not interface icons) */
const BULLETS = ["•", "◦", "▪", "■", "●", "○", "◆", "✓", "➢", "★", "❖", "–"];

const unit = (tw: number) => String(Math.round((tw / UNIT_TW) * 100) / 100);
const twips = (v: string) => Math.round((parseFloat(v) || 0) * UNIT_TW);

/** The levels of the outline a new multilevel list starts from: 1, 1.1, 1.1.1 with the numbers hanging at the margin. */
function outlineLevels(): LevelDef[] {
  return Array.from({ length: 9 }, (_, i) => ({
    format: "decimal",
    text: `${Array.from({ length: i + 1 }, (_, k) => `%${k + 1}`).join(".")}.`,
    start: 1,
    align: "left" as const,
    suffix: "tab" as const,
    left: 432 + 216 * i,
    hanging: 432 + 216 * i,
  }));
}

/** A level's definition made into nine by repeating the last one, further in. */
function nineLevels(levels: LevelDef[]): LevelDef[] {
  const out = levels.map((l) => ({ ...l }));
  while (out.length < 9) {
    const last = out[out.length - 1];
    out.push({ ...last, style: undefined, left: (last.left ?? 720) + 720 });
  }
  return out.slice(0, 9);
}

export function openDefineList(ed: Editor, opts: { mode: DefineMode; fonts: string[] }) {
  const mode = opts.mode;
  const multi = mode === "multilevel";
  const lang = ed.engine.pageSetup().lang;
  const current = ed.format();
  const existing = multi && current?.listNum !== undefined ? ed.engine.listDefinition(current.listNum) : null;
  let levels: LevelDef[] =
    mode === "bullet"
      ? [{ format: "bullet", text: "•", align: "left", suffix: "tab", left: 720, hanging: 360 }]
      : mode === "number"
        ? [{ format: "decimal", text: "%1.", start: 1, align: "left", suffix: "tab", left: 720, hanging: 360 }]
        : nineLevels(existing?.levels ?? outlineLevels());
  // Pictures chosen in this dialog: the file waits here until OK, so cancelling leaves nothing in the document.
  const picked = new Map<number, { file: File; url: string; w: number; h: number }>();
  let cur = 0;
  const lv = () => levels[cur];
  const set = (patch: Partial<LevelDef>) => {
    // A setting that was cleared goes away rather than stay as an empty value.
    levels[cur] = Object.fromEntries(Object.entries({ ...levels[cur], ...patch }).filter(([, v]) => v !== undefined)) as unknown as LevelDef;
    paint();
  };
  const count = multi ? 9 : 1;
  const isBullet = () => lv().format === "bullet";

  // ---- controls ----

  const levelTabs = h("div", { class: "seg dl-levels", role: "radiogroup", "aria-label": t("Level") });
  if (multi) {
    for (let i = 0; i < 9; i++) {
      const b = h("button", { type: "button", role: "radio", "aria-checked": "false", "aria-label": t("Level {n}", { n: i + 1 }) }, String(i + 1));
      b.addEventListener("click", () => {
        cur = i;
        paint();
      });
      levelTabs.append(b);
    }
  }

  // Bullet: a character (or a picture) and the font it is drawn in.
  const bulletKeys = h("div", { class: "dl-bullets", role: "group", "aria-label": t("Bullet character") });
  for (const c of BULLETS) {
    const b = h("button", { type: "button", class: "dl-bullet", "aria-pressed": "false", "data-c": c, "aria-label": c }, c);
    b.addEventListener("click", () => set({ format: "bullet", text: c, picture: undefined, picture_size: undefined }));
    bulletKeys.append(b);
  }
  const bulletInput = h("input", { type: "text", id: "dl-bullet", maxlength: "4", autocomplete: "off", spellcheck: "false", style: "width:100%;min-width:0" });
  bulletInput.addEventListener("input", () => {
    const text = bulletInput.value;
    if (text) set({ format: "bullet", text, picture: undefined, picture_size: undefined });
  });
  const moreSymbols = h("button", { class: "btn", type: "button" }, t("More symbols…"));
  moreSymbols.addEventListener("click", () => {
    void openSpecialCharacters(ed, { recent: { get: () => [], set: () => undefined }, pick: (c) => set({ format: "bullet", text: c, picture: undefined, picture_size: undefined }) });
  });
  const file = h("input", { type: "file", accept: "image/png,image/jpeg,image/gif,image/webp", hidden: true, "aria-label": t("Bullet picture") });
  const choosePicture = h("button", { class: "btn", type: "button" }, t("Picture…"));
  choosePicture.addEventListener("click", () => file.click());
  file.addEventListener("change", () => {
    const f = file.files?.[0];
    file.value = "";
    if (!f) return;
    const url = URL.createObjectURL(f);
    const img = new Image();
    img.onload = () => {
      // The bullet is about as tall as a line of text, whatever shape the picture is.
      const scale = 10 / Math.max(img.naturalWidth, img.naturalHeight, 1);
      const w = Math.max(1, Math.round(img.naturalWidth * scale * 10) / 10);
      const hh = Math.max(1, Math.round(img.naturalHeight * scale * 10) / 10);
      const before = picked.get(cur);
      if (before) URL.revokeObjectURL(before.url);
      picked.set(cur, { file: f, url, w, h: hh });
      set({ format: "bullet", picture: `picked:${cur}`, picture_size: [w, hh] });
    };
    img.onerror = () => URL.revokeObjectURL(url);
    img.src = url;
  });
  const removePicture = h("button", { class: "btn", type: "button" }, t("Use a character instead"));
  removePicture.addEventListener("click", () => set({ picture: undefined, picture_size: undefined, text: lv().text || "•" }));
  const pictureNote = h("span", { class: "help dl-picture-note" });
  const bulletField = h(
    "div",
    { class: "field" },
    h("label", {}, t("Bullet character")),
    bulletKeys,
    h("div", { class: "dl-row-inputs" }, h("div", { class: "field", style: "margin:0;flex:0 0 72px" }, h("label", { for: "dl-bullet", class: "dl-sub" }, t("Other")), bulletInput), moreSymbols, choosePicture, file),
    pictureNote,
    removePicture,
  );

  // Number: its style, the text around the count, and where it starts.
  const styleSelect = h("select", { id: "dl-style", "aria-label": t("Number style") });
  for (const g of STYLE_GROUPS) {
    const parent = g.title ? h("optgroup", { label: t(g.title) }) : styleSelect;
    for (const f of g.formats) {
      const sample = [1, 2, 3].map((n) => ed.engine.formatNumber(n, f, lang)).join(", ");
      parent.append(h("option", { value: f }, sample));
    }
    if (g.title) styleSelect.append(parent);
  }
  styleSelect.append(h("option", { value: "none" }, t("None")));
  styleSelect.addEventListener("change", () => set({ format: styleSelect.value }));
  const textInput = h("input", { type: "text", id: "dl-text", autocomplete: "off", spellcheck: "false", style: "width:100%;min-width:0" });
  textInput.addEventListener("input", () => set({ text: textInput.value }));
  const fromLevel = h("select", { id: "dl-from", "aria-label": t("Include the number from level") });
  fromLevel.addEventListener("change", () => {
    const n = fromLevel.value;
    if (!n) return;
    const at = textInput.selectionStart ?? textInput.value.length;
    const end = textInput.selectionEnd ?? at;
    textInput.value = `${textInput.value.slice(0, at)}%${n}${textInput.value.slice(end)}`;
    fromLevel.value = "";
    set({ text: textInput.value });
    textInput.focus();
    textInput.setSelectionRange(at + 2, at + 2);
  });
  const startInput = h("input", { type: "number", id: "dl-start", min: "0", step: "1", style: "width:100%;min-width:0" });
  startInput.addEventListener("input", () => set({ start: Math.max(0, Math.round(parseFloat(startInput.value) || 0)) }));
  const legal = h("input", { type: "checkbox", id: "dl-legal" });
  legal.addEventListener("change", () => set({ legal: legal.checked || undefined }));
  const restart = h("select", { id: "dl-restart", "aria-label": t("Restart the list after") });
  restart.addEventListener("change", () => set({ restart: restart.value === "" ? undefined : Number(restart.value) }));
  const link = h("select", { id: "dl-link", "aria-label": t("Link level to style") });
  link.addEventListener("change", () => set({ style: link.value || undefined }));
  const numberField = h(
    "div",
    {},
    h("div", { class: "field" }, h("label", { for: "dl-style" }, t("Number style")), styleSelect),
    h(
      "div",
      { class: "field" },
      h("label", { for: "dl-text" }, t("Number format")),
      textInput,
      multi ? h("div", { class: "dl-row-inputs" }, h("label", { for: "dl-from", class: "dl-sub" }, t("Include the number from level")), fromLevel) : null,
      h("span", { class: "help" }, multi ? t("%1 stands for the number of level 1, %2 for level 2, and so on.") : t("%1 stands for the number. Add text around it, like Step %1:")),
    ),
    h("div", { class: "field" }, h("label", { for: "dl-start" }, t("Start at")), startInput),
  );
  const multiField = h(
    "div",
    {},
    h("div", { class: "field" }, h("label", { for: "dl-restart" }, t("Restart the list after")), restart),
    h("label", { class: "check", for: "dl-legal" }, legal, h("span", {}, t("Legal style numbering (1, 2, 3 for earlier levels)"))),
    h("div", { class: "field" }, h("label", { for: "dl-link" }, t("Link level to style")), link),
  );

  // Font of the label.
  const fonts = Array.from(new Set([...MENU_FONTS, ...opts.fonts])).sort((a, b) => a.localeCompare(b));
  const fontSelect = h("select", { id: "dl-font", "aria-label": t("Font") }, h("option", { value: "" }, t("Same as the text")), ...fonts.map((f) => h("option", { value: f }, f)));
  fontSelect.addEventListener("change", () => set({ font: fontSelect.value || undefined }));
  const bold = h("input", { type: "checkbox", id: "dl-bold" });
  const italic = h("input", { type: "checkbox", id: "dl-italic" });
  bold.addEventListener("change", () => set({ bold: bold.checked || undefined }));
  italic.addEventListener("change", () => set({ italic: italic.checked || undefined }));
  const colors = swatchGrid(TEXT_COLORS, null, (c) => set({ color: c }));
  const autoColor = h("button", { type: "button", "aria-pressed": "false" }, t("Automatic"));
  autoColor.addEventListener("click", () => set({ color: undefined }));
  const fontField = h(
    "div",
    { class: "dl-fontrow" },
    h("div", { class: "field" }, h("label", { for: "dl-font" }, t("Font")), fontSelect, h("div", { class: "dl-checks" }, h("label", { class: "check", for: "dl-bold" }, bold, h("span", {}, t("Bold"))), h("label", { class: "check", for: "dl-italic" }, italic, h("span", {}, t("Italic"))))),
    h("div", { class: "field" }, h("label", {}, t("Color")), colors.el, h("div", { class: "seg bs-one" }, autoColor)),
  );

  // Position.
  const alignSeg = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Alignment") });
  const aligns: ["left" | "center" | "right", string][] = [["left", t("Left")], ["center", t("Center")], ["right", t("Right")]];
  for (const [id, label] of aligns) {
    const b = h("button", { type: "button", role: "radio", "aria-checked": "false", "data-v": id }, label);
    b.addEventListener("click", () => set({ align: id }));
    alignSeg.append(b);
  }
  const numberAt = h("input", { type: "number", min: "0", step: METRIC ? "0.1" : "0.05", id: "dl-at", style: "width:100%;min-width:0" });
  const textAt = h("input", { type: "number", min: "0", step: METRIC ? "0.1" : "0.05", id: "dl-indent", style: "width:100%;min-width:0" });
  // The number sits at "aligned at", the text at "text indent": the engine stores the indent and how far the number hangs.
  numberAt.addEventListener("input", () => {
    const left = lv().left ?? 720;
    set({ hanging: Math.max(0, left - twips(numberAt.value)) });
  });
  textAt.addEventListener("input", () => {
    const left = Math.max(0, twips(textAt.value));
    const at = (lv().left ?? 720) - (lv().hanging ?? 0);
    set({ left, hanging: Math.max(0, left - Math.max(0, at)) });
  });
  const suffixSeg = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Follow the number with") });
  const suffixes: ["tab" | "space" | "nothing", string][] = [["tab", t("Tab")], ["space", t("Space")], ["nothing", t("Nothing")]];
  for (const [id, label] of suffixes) {
    const b = h("button", { type: "button", role: "radio", "aria-checked": "false", "data-v": id }, label);
    b.addEventListener("click", () => set({ suffix: id }));
    suffixSeg.append(b);
  }
  const tabAt = h("input", { type: "number", min: "0", step: METRIC ? "0.1" : "0.05", id: "dl-tab", style: "width:100%;min-width:0" });
  tabAt.addEventListener("input", () => set({ tab: tabAt.value === "" ? undefined : Math.max(0, twips(tabAt.value)) }));
  const unitName = METRIC ? t("cm") : t("inches");
  const positionField = h(
    "div",
    {},
    h("div", { class: "field" }, h("label", {}, t("Alignment")), alignSeg),
    h(
      "div",
      { class: "dl-two" },
      h("div", { class: "field" }, h("label", { for: "dl-at" }, mode === "bullet" ? t("Bullet position ({unit})", { unit: unitName }) : t("Number position ({unit})", { unit: unitName })), numberAt),
      h("div", { class: "field" }, h("label", { for: "dl-indent" }, t("Text indent ({unit})", { unit: unitName })), textAt),
    ),
    h("div", { class: "field" }, h("label", {}, t("Follow the number with")), suffixSeg),
    h("div", { class: "field" }, h("label", { for: "dl-tab" }, t("Tab stop at ({unit})", { unit: unitName })), tabAt),
  );

  // Preview: the labels of the levels in play, drawn at half size.
  const preview = h("div", { class: multi ? "dl-preview" : "dl-preview dl-short", role: "img", "aria-label": t("Preview of the list") });
  const error = h("p", { class: "dl-error", role: "alert", hidden: true });

  // ---- painting ----

  // `bump` counts on from the start (the second line of a one-level preview shows the second number).
  const labelText = (i: number, bump = 0): string => {
    const l = levels[i];
    if (l.format === "bullet") return l.text;
    return l.text.replace(/%([1-9])/g, (_, k: string) => {
      const at = Number(k) - 1;
      const src = levels[at] ?? l;
      return ed.engine.formatNumber((src.start ?? 1) + (at === i ? bump : 0), l.legal ? "decimal" : src.format, lang);
    });
  };
  const pictureUrl = (l: LevelDef, i: number): string | null => {
    if (!l.picture) return null;
    const p = picked.get(i);
    if (l.picture.startsWith("picked:")) return p?.url ?? null;
    return ed.engine.mediaUrl(l.picture);
  };

  const paint = () => {
    const l = lv();
    levelTabs.querySelectorAll("button").forEach((b, i) => {
      b.classList.toggle("on", i === cur);
      b.setAttribute("aria-checked", String(i === cur));
    });
    const bul = isBullet();
    const pic = !!l.picture;
    bulletField.hidden = !bul;
    numberField.hidden = bul;
    bulletKeys.querySelectorAll<HTMLButtonElement>(".dl-bullet").forEach((b) => {
      const on = !pic && b.dataset.c === l.text;
      b.classList.toggle("on", on);
      b.setAttribute("aria-pressed", String(on));
    });
    if (document.activeElement !== bulletInput) bulletInput.value = pic ? "" : l.text;
    pictureNote.textContent = pic ? t("The bullet is a picture.") : "";
    pictureNote.hidden = !pic;
    removePicture.hidden = !pic;
    if (!bul) {
      styleSelect.value = [...styleSelect.options].some((o) => o.value === l.format) ? l.format : "decimal";
      if (document.activeElement !== textInput) textInput.value = l.text;
      startInput.value = String(l.start ?? 1);
    }
    // Numbers of earlier levels and of this one can be named in the text.
    if (multi) {
      fromLevel.replaceChildren(h("option", { value: "" }, t("Choose a level")), ...Array.from({ length: cur + 1 }, (_, i) => h("option", { value: String(i + 1) }, t("Level {n}", { n: i + 1 }))));
      // The first level has nothing above it to restart after. Below it: after level k or higher; the usual rule,
      // any higher level, is the empty choice.
      restart.disabled = cur === 0;
      if (cur === 0) restart.replaceChildren(h("option", { value: "" }, t("Never")));
      else {
        const choices: [string, string][] = [["0", t("Never")]];
        for (let k = 1; k < cur; k++) choices.push([String(k), t("Level {n}", { n: k })]);
        restart.replaceChildren(h("option", { value: "" }, t("Any higher level")), ...choices.map(([v, text]) => h("option", { value: v }, text)));
        restart.value = l.restart === undefined || l.restart === cur ? "" : String(l.restart);
      }
      legal.checked = !!l.legal;
      legal.disabled = bul;
      const styles = ed.engine.styles().filter((s) => s.kind === "paragraph" && !s.hidden);
      const headings = [1, 2, 3, 4, 5, 6].map((n) => `Heading ${n}`);
      const named = new Set(headings.map((x) => x.replace(/\s/g, "").toLowerCase()));
      const others = styles.filter((s) => !named.has(s.id.toLowerCase()) && !named.has(s.name.replace(/\s/g, "").toLowerCase())).slice(0, 30);
      link.replaceChildren(
        h("option", { value: "" }, t("None")),
        ...headings.map((x, i) => h("option", { value: x }, t("Heading {n}", { n: i + 1 }))),
        ...others.map((s) => h("option", { value: s.id }, s.name)),
      );
      // The file names a style by its id (Heading2); the choices say Heading 2.
      const flat = (s: string) => s.replace(/\s/g, "").toLowerCase();
      const linked = [...link.options].find((o) => o.value && flat(o.value) === flat(l.style ?? ""))?.value ?? l.style ?? "";
      if (linked && ![...link.options].some((o) => o.value === linked)) link.append(h("option", { value: linked }, linked));
      link.value = linked;
      // A style can be linked to one level of a list.
      link.title = t("A style can be linked to one level.");
    }
    fontSelect.value = l.font && [...fontSelect.options].some((o) => o.value === l.font) ? l.font : "";
    if (l.font && fontSelect.value === "") fontSelect.append(h("option", { value: l.font, selected: true }, l.font));
    bold.checked = !!l.bold;
    italic.checked = !!l.italic;
    colors.select(l.color ?? null);
    autoColor.classList.toggle("on", !l.color);
    autoColor.setAttribute("aria-pressed", String(!l.color));
    alignSeg.querySelectorAll("button").forEach((b) => {
      const on = (l.align ?? "left") === (b as HTMLElement).dataset.v;
      b.classList.toggle("on", on);
      b.setAttribute("aria-checked", String(on));
    });
    suffixSeg.querySelectorAll("button").forEach((b) => {
      const on = (l.suffix ?? "tab") === (b as HTMLElement).dataset.v;
      b.classList.toggle("on", on);
      b.setAttribute("aria-checked", String(on));
    });
    const left = l.left ?? 720;
    const hang = l.hanging ?? 0;
    if (document.activeElement !== numberAt) numberAt.value = unit(Math.max(0, left - hang));
    if (document.activeElement !== textAt) textAt.value = unit(left);
    if (document.activeElement !== tabAt) tabAt.value = l.tab === undefined ? "" : unit(l.tab);
    tabAt.disabled = (l.suffix ?? "tab") !== "tab";

    // The preview: one line per level in play, the label where the list puts it and a line of text where the text starts.
    preview.replaceChildren();
    const shown = multi ? Array.from({ length: count }, (_, i) => i) : [0, 0, 0];
    shown.forEach((i, row) => {
      const lev = levels[i];
      const at = Math.max(0, (lev.left ?? 720) - (lev.hanging ?? 0)) / 30;
      const textX = (lev.left ?? 720) / 30;
      const url = pictureUrl(lev, i);
      const face = url ? h("img", { src: url, alt: "", style: `width:${lev.picture_size?.[0] ?? 9}pt;height:${lev.picture_size?.[1] ?? 9}pt` }) : h("span", {}, labelText(i, multi ? 0 : row));
      const style = [
        lev.font ? `font-family:${fontStack(lev.font)}` : "",
        lev.bold ? "font-weight:700" : "",
        lev.italic ? "font-style:italic" : "",
        hex(lev.color) ? `color:${hex(lev.color)}` : "",
        lev.align === "right" ? "transform:translateX(-100%)" : lev.align === "center" ? "transform:translateX(-50%)" : "",
      ].filter(Boolean);
      const lbl = h("span", { class: "dl-label", style: `left:${at}px;${style.join(";")}` }, face);
      const text = h("span", { class: "dl-sample", style: `left:${textX}px` }, t("Sample text"));
      if (i === cur && multi) lbl.classList.add("dl-current");
      preview.append(h("div", { class: "dl-line", "data-row": String(row) }, lbl, text));
    });
    // A long number pushes the text along, as it does on the page.
    if (!preview.isConnected) return;
    preview.querySelectorAll<HTMLElement>(".dl-line").forEach((line) => {
      const lbl = line.querySelector<HTMLElement>(".dl-label")!;
      const text = line.querySelector<HTMLElement>(".dl-sample")!;
      const room = lbl.getBoundingClientRect().right - line.getBoundingClientRect().left + 6;
      if (room > text.offsetLeft) text.style.left = `${room}px`;
    });
  };

  // ---- the dialog ----

  const title = mode === "bullet" ? t("Define new bullet") : mode === "number" ? t("Define new number format") : t("Define new multilevel list");
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog dl-dialog", role: "dialog", "aria-modal": "true", "aria-label": title },
    h("div", { class: "dialog-head" }, h("h2", {}, title)),
    h(
      "div",
      { class: "dialog-body" },
      multi ? h("div", { class: "field" }, h("label", {}, t("Level")), levelTabs) : null,
      h(
        "div",
        { class: "dl-grid" },
        h("div", { class: "dl-col" }, bulletField, numberField, multi ? multiField : null),
        h("div", { class: "dl-col" }, positionField, h("div", { class: "field" }, h("label", {}, t("Preview")), preview)),
      ),
      fontField,
      error,
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  paint();
  const close = modal(dialog, () => {
    for (const p of picked.values()) URL.revokeObjectURL(p.url);
  });
  // Drawn again now it is on the page, where the preview can measure its labels.
  paint();
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    // A number needs a count in its text.
    for (let i = 0; i < count; i++) {
      const l = levels[i];
      if (l.format !== "bullet" && l.format !== "none" && !l.text.includes("%")) {
        cur = i;
        paint();
        error.textContent = t("The number format needs a number in it: write %{n} where it goes.", { n: i + 1 });
        error.hidden = false;
        textInput.focus();
        return;
      }
      if (l.format === "bullet" && !l.picture && !l.text) {
        cur = i;
        paint();
        error.textContent = t("Choose a character or a picture for the bullet.");
        error.hidden = false;
        return;
      }
    }
    void finish();
  });

  async function finish() {
    const ids = ed.selectedParagraphs();
    const out: LevelDef[] = [];
    for (let i = 0; i < count; i++) {
      const l = { ...levels[i] };
      const pic = l.picture?.startsWith("picked:") ? picked.get(Number(l.picture.slice(7))) : null;
      if (pic) {
        const bytes = new Uint8Array(await pic.file.arrayBuffer());
        const ext = (pic.file.name.split(".").pop() || pic.file.type.split("/")[1] || "png").toLowerCase().replace("jpeg", "jpg").replace("svg+xml", "svg");
        l.picture = ed.engine.addMedia(ext, bytes);
        l.picture_size = [pic.w, pic.h];
        l.text = "";
      }
      if (l.format === "bullet" && !l.picture) l.picture = undefined;
      out.push(l);
    }
    const def: ListDef = { levels: out };
    // The engine says what is wrong in plain words; the dialog stays open to fix it.
    const res = ed.exec({ op: "define_list", ids, def }, {
      onError: (message) => {
        error.textContent = message;
        error.hidden = false;
      },
    });
    if (!res) return;
    close();
    ed.focus();
  }
}
