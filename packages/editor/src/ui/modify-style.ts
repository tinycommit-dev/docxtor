// Modify style, New style and Create a style from the selection: the dialog that edits a style's definition. It shows
// the name, the type, what it builds on and what follows it, the font and paragraph basics in a row, a live preview and
// a plain description, and a Format menu for the dialogs that go deeper (Font, Paragraph, Tabs, Borders, Language,
// Numbering). Nothing is written until OK, and then it is one `set_style_definition` op: one undo step for every
// paragraph that uses the style.
import type { Op, ParaFmt, ParaPatch, RunFmt, RunPatch, StyleDefinition, StyleDetails, StyleInfo } from "@docxtor/core";
import { t, type ExtensionHost } from "@docxtor/extensions";
import { fileAlign, visualAlign, type VisualAlign } from "../editor/align";
import type { Editor } from "../editor/editor";
import { fontStack, paraSpace, runStyle } from "../editor/render";
import { h, modal, popover, svg } from "./dom";
import { openFont } from "./font";
import type { FormatTarget, RunTarget } from "./format-target";
import { I } from "./icons";
import { openBordersShading } from "./borders-shading";
import { openParagraph } from "./paragraph";
import { applyPara, applyRun, describeStyle, styleTitle } from "./style-preview";
import { fontOptions, openStyleLanguage, openStyleNumbering, type StyleList } from "./style-parts";
import { openTabs } from "./tabs-dialog";
import { colorPalette, colorPatch } from "./swatches";
import { hex } from "../editor/color.ts";

type Kind = "paragraph" | "character" | "linked";

/** What the dialog is changing, and the draft it keeps until OK. */
interface Draft {
  name: string;
  kind: Kind;
  basedOn: string;
  next: string;
  quick: boolean;
  autoUpdate: boolean;
  ppr: ParaPatch;
  rpr: RunPatch;
  lang?: string;
  list?: StyleList;
}

/** The styles the default design for new documents keeps (see the engine's `design_snapshot`). */
const DEFAULT_DESIGN_STYLES = new Set(["Normal", "Title", "Subtitle", "Heading1", "Heading2", "Heading3", "Heading4", "Heading5", "Heading6", "Heading7", "Heading8", "Heading9", "Quote"]);

const LINES: [number, string][] = [
  [1, "1"],
  [1.15, "1.15"],
  [1.5, "1.5"],
  [2, "2"],
];

export interface StyleDialogOptions {
  ed: Editor;
  host: ExtensionHost;
  /** The style to modify; none makes a new style. */
  style?: string;
}

/** The styles a style can build on: its own kind, not itself and not one that builds on it. */
export function baseChoices(styles: StyleInfo[], kind: "paragraph" | "character", self?: string): StyleInfo[] {
  const byId = new Map(styles.map((s) => [s.id, s]));
  const builtOnSelf = (id: string): boolean => {
    for (let cur = byId.get(id), n = 0; cur && n < 32; cur = cur.basedOn ? byId.get(cur.basedOn) : undefined, n++) if (cur.id === self) return true;
    return false;
  };
  return styles.filter((s) => s.kind === kind && s.id !== self && !(self && builtOnSelf(s.id)) && !(s.hidden && !s.quick && s.id !== "DefaultParagraphFont"));
}

export function openStyleDialog({ ed, host, style }: StyleDialogOptions) {
  const engine = ed.engine;
  if (!engine) return;
  const details: StyleDetails | null = style ? engine.styleDetails(style) : null;
  if (style && !details) return;
  const defaultPara = engine.styles().find((s) => s.kind === "paragraph" && s.id === "Normal")?.id ?? "Normal";
  const kindOf = (): Kind => (details ? (details.link ? "linked" : (details.kind as Kind)) : "paragraph");
  const draft: Draft = {
    name: details ? details.name : "",
    kind: kindOf(),
    basedOn: details ? (details.basedOn ?? "") : defaultPara,
    next: details ? (details.next ?? "") : "",
    quick: details ? details.quick : true,
    autoUpdate: details ? details.autoUpdate : false,
    ppr: {},
    rpr: {},
    lang: undefined,
  };
  const used = engine.fontsUsed().map((f) => f.family);
  const stylesNow = () => engine.styles();
  const baseDetails = (id: string | undefined) => (id ? engine.styleDetails(id) : null);

  // What the style looks like with the draft applied: its base, its own formatting, then the changes.
  const ownRun = (d: StyleDetails): RunFmt => {
    const own = { ...d.ownRun } as RunFmt & { fontTheme?: string; colorTheme?: unknown };
    if (own.fontTheme && d.run.font) own.font = d.run.font;
    if (own.colorTheme && d.run.color) own.color = d.run.color;
    return own;
  };
  const resolved = (): { run: RunFmt; para: ParaFmt } => {
    let run: RunFmt;
    let para: ParaFmt;
    if (details && draft.basedOn === (details.basedOn ?? "")) {
      run = details.run;
      para = details.para;
    } else {
      const base = baseDetails(draft.basedOn || defaultPara);
      run = { ...(base?.run ?? {}), ...(details ? ownRun(details) : {}) };
      para = { ...(base?.para ?? {}), ...(details?.ownPara ?? {}) };
    }
    return { run: applyRun(run, draft.rpr), para: applyPara(para, draft.ppr) };
  };

  const err = h("p", { class: "ms-error help", role: "alert" });
  const name = h("input", { id: "ms-name", type: "text", value: draft.name, autocomplete: "off", spellcheck: "false", placeholder: t("Style name") });
  name.disabled = !!details && !details.custom;
  const kindSel = h("select", { id: "ms-kind", disabled: !!details }, ...([["paragraph", t("Paragraph")], ["character", t("Character")], ["linked", t("Linked (paragraph and character)")]] as [Kind, string][]).map(([v, l]) => h("option", { value: v, selected: v === draft.kind }, l)));
  const basedSel = h("select", { id: "ms-based" });
  const nextSel = h("select", { id: "ms-next" });
  const fontSel = h("select", { id: "ms-font", "aria-label": t("Font") });
  const sizeIn = h("input", { id: "ms-size", type: "number", min: "1", max: "1000", step: "0.5", "aria-label": t("Font size") });
  const toggle = (key: "bold" | "italic" | "underline", glyph: string, title: string, cls: string) => {
    const b = h("button", { class: `tb glyph ${cls}`, type: "button", title, "aria-label": title, "aria-pressed": "false" }, glyph);
    b.addEventListener("click", () => {
      const cur = resolved().run;
      if (key === "underline") draft.rpr.underline = cur.underline && cur.underline !== "none" ? "none" : "single";
      else draft.rpr[key] = !cur[key];
      refresh();
    });
    return b;
  };
  const bold = toggle("bold", "B", t("Bold"), "");
  const italic = toggle("italic", "I", t("Italic"), "i");
  const underline = toggle("underline", "U", t("Underline"), "u");
  const colorBtn = h("button", { class: "tb", type: "button", title: t("Font color"), "aria-label": t("Font color") }, svg(host.icon("textColor") ?? I.textColor), h("span", { class: "color-bar" }));
  colorBtn.addEventListener("click", () => {
    const box = h("div", {});
    const close = popover(colorBtn, box);
    const now = resolved().run.color;
    const set = (patch: RunPatch) => {
      // A colour picked from the palette replaces what an earlier pick in this dialog wrote.
      const { color: _c, themeColor: _t, themeTint: _ti, themeShade: _s, ...rest } = draft.rpr as RunPatch & { themeColor?: string; themeTint?: string; themeShade?: string };
      draft.rpr = { ...rest, ...patch };
      close();
      refresh();
    };
    box.append(
      colorPalette({
        title: t("Font color"),
        theme: engine.themeColors(),
        current: now ?? null,
        automatic: { label: t("Automatic"), on: !now, run: () => set({ color: "auto" }) },
        pick: (c) => set(colorPatch(c)),
        beforeMore: close,
      }).el,
    );
  });
  const aligns: [VisualAlign, string, string][] = [["left", t("Align left"), "alignLeft"], ["center", t("Center"), "alignCenter"], ["right", t("Align right"), "alignRight"], ["both", t("Justify"), "alignJustify"]];
  const alignBtns = aligns.map(([v, title, icon]) => {
    const b = h("button", { class: "tb", type: "button", title, "aria-label": title, "aria-pressed": "false", "data-align": v }, svg(host.icon(icon) ?? (I as Record<string, string>)[icon]));
    b.addEventListener("click", () => {
      draft.ppr.align = fileAlign(v, false);
      refresh();
    });
    return b;
  });
  const lineSel = h("select", { id: "ms-line", "aria-label": t("Line spacing") }, ...LINES.map(([v, l]) => h("option", { value: String(v) }, l)));
  lineSel.addEventListener("change", () => {
    draft.ppr.lineMultiple = Number(lineSel.value);
    delete draft.ppr.line;
    delete draft.ppr.lineRule;
    refresh();
  });
  const beforeIn = h("input", { id: "ms-before", type: "number", min: "0", max: "1584", step: "1", "aria-label": t("Space before (pt)") });
  const afterIn = h("input", { id: "ms-after", type: "number", min: "0", max: "1584", step: "1", "aria-label": t("Space after (pt)") });
  beforeIn.addEventListener("change", () => {
    draft.ppr.spaceBefore = Math.max(0, Math.round((Number(beforeIn.value) || 0) * 20));
    refresh();
  });
  afterIn.addEventListener("change", () => {
    draft.ppr.spaceAfter = Math.max(0, Math.round((Number(afterIn.value) || 0) * 20));
    refresh();
  });
  const paragraphOnly = [...alignBtns, lineSel, beforeIn, afterIn];

  const quick = h("input", { id: "ms-quick", type: "checkbox", checked: draft.quick });
  const auto = h("input", { id: "ms-auto", type: "checkbox", checked: draft.autoUpdate });
  quick.addEventListener("change", () => (draft.quick = quick.checked));
  auto.addEventListener("change", () => (draft.autoUpdate = auto.checked));
  const preview = h("div", { class: "ms-preview", "aria-label": t("Preview") });
  const description = h("p", { class: "ms-description" });

  const fillBased = () => {
    const kind = draft.kind === "character" ? "character" : "paragraph";
    const choices = baseChoices(stylesNow(), kind, details?.id);
    if (draft.basedOn && !choices.some((s) => s.id === draft.basedOn)) draft.basedOn = kind === "paragraph" ? defaultPara : "";
    basedSel.replaceChildren(h("option", { value: "" }, t("(no style)")), ...choices.map((s) => h("option", { value: s.id, selected: s.id === draft.basedOn }, styleTitle(s.name, s.custom))));
    basedSel.value = draft.basedOn;
    const follow = stylesNow().filter((s) => s.kind === "paragraph" && !(s.hidden && !s.quick));
    nextSel.replaceChildren(h("option", { value: "" }, t("(same style)")), ...follow.filter((s) => s.id !== details?.id).map((s) => h("option", { value: s.id, selected: s.id === draft.next }, styleTitle(s.name, s.custom))));
    nextSel.value = draft.next;
    nextSel.disabled = draft.kind === "character";
  };

  const sample = () => t("Sample text");
  const refresh = () => {
    const { run, para } = resolved();
    fontSel.replaceChildren(...fontOptions(used, run.font).map((f) => h("option", { value: f, selected: f === run.font }, f)));
    if (!run.font) fontSel.prepend(h("option", { value: "", selected: true }, t("Default")));
    fontSel.value = run.font ?? "";
    if (document.activeElement !== sizeIn) sizeIn.value = run.size ? String(run.size) : "";
    for (const [b, on] of [[bold, run.bold], [italic, run.italic], [underline, run.underline && run.underline !== "none"]] as [HTMLElement, unknown][]) {
      b.classList.toggle("on", !!on);
      b.setAttribute("aria-pressed", String(!!on));
    }
    (colorBtn.querySelector(".color-bar") as HTMLElement).style.background = hex(run.color) ?? "var(--ink)";
    const va = visualAlign(para.align, para.bidi);
    for (const b of alignBtns) {
      const on = b.dataset.align === va;
      b.classList.toggle("on", on);
      b.setAttribute("aria-pressed", String(on));
    }
    const mult = para.line && (para.lineRule ?? "auto") === "auto" ? Math.round((para.line / 240) * 100) / 100 : 1;
    if (!LINES.some(([v]) => v === mult)) lineSel.append(h("option", { value: String(mult) }, String(mult)));
    lineSel.value = String(mult);
    const sp = paraSpace(para, run);
    if (document.activeElement !== beforeIn) beforeIn.value = String(Math.round((sp.before / 20) * 100) / 100);
    if (document.activeElement !== afterIn) afterIn.value = String(Math.round((sp.after / 20) * 100) / 100);
    for (const c of paragraphOnly) (c as HTMLButtonElement).disabled = draft.kind === "character";
    drawPreview(run, para);
    const basedName = draft.basedOn ? stylesNow().find((s) => s.id === draft.basedOn) : undefined;
    const nextName = draft.next ? stylesNow().find((s) => s.id === draft.next) : undefined;
    description.textContent = describeStyle(run, draft.kind === "character" ? {} : para, {
      basedOn: basedName ? styleTitle(basedName.name, basedName.custom) : undefined,
      next: nextName ? styleTitle(nextName.name, nextName.custom) : undefined,
    });
  };

  const drawPreview = (run: RunFmt, para: ParaFmt) => {
    const grey = (text: string) => h("div", { class: "ms-grey" }, text);
    const css = [runStyle({ ...run, size: run.size ? Math.min(run.size, 28) : undefined })];
    if (!run.font) css.push(`font-family:${fontStack("Calibri")}`);
    const { before, after } = paraSpace(para, run);
    css.push(`margin-top:${Math.min(before / 15, 24)}px`, `margin-bottom:${Math.min(after / 15, 24)}px`);
    const align = { center: "center", right: "right", end: "right", both: "justify" }[para.align ?? ""];
    if (align) css.push(`text-align:${align}`);
    if (para.line && (para.lineRule ?? "auto") === "auto") css.push(`line-height:${(para.line / 240) * 1.2}`);
    if (para.indLeft) css.push(`padding-left:${Math.min(para.indLeft / 15, 80)}px`);
    if (para.indRight) css.push(`padding-right:${Math.min(para.indRight / 15, 80)}px`);
    if (para.indFirstLine) css.push(`text-indent:${Math.min(para.indFirstLine / 15, 60)}px`);
    if (para.indHanging) css.push(`text-indent:-${Math.min(para.indHanging / 15, 60)}px`);
    if (hex(para.shading)) css.push(`background:${hex(para.shading)}`);
    const line = sample();
    const body = h("div", { class: "ms-sample", style: css.filter(Boolean).join(";") }, `${line} ${line} ${line} ${line}`);
    preview.replaceChildren(grey(t("Previous paragraph. Previous paragraph. Previous paragraph.")), body, grey(t("Following paragraph. Following paragraph. Following paragraph.")));
  };

  // The dialogs behind the Format button work on the draft instead of the selection.
  const styleTarget: FormatTarget = {
    state: () => resolved(),
    setPara: (patch, align) => {
      draft.ppr = { ...draft.ppr, ...patch, ...(align ? { align: fileAlign(align, !!patch.bidi) } : {}) };
      if (patch.line !== undefined && patch.lineMultiple === undefined) delete draft.ppr.lineMultiple;
      refresh();
    },
    document: false,
    done: () => undefined,
  };
  const runTarget: RunTarget = {
    state: () => resolved().run,
    setRun: (patch) => {
      draft.rpr = { ...draft.rpr, ...patch };
      refresh();
    },
    defaults: false,
    done: () => undefined,
  };
  const formatMenu = h("button", { class: "btn", type: "button", "aria-haspopup": "menu" }, t("Format"));
  formatMenu.append(svg(host.icon("chevron") ?? I.chevron));
  formatMenu.addEventListener("click", () => {
    const list = h("div", {});
    const close = popover(formatMenu, list);
    const item = (label: string, run: () => void) => {
      const b = h("button", { class: "menu-item", type: "button" }, h("span", {}, label));
      b.addEventListener("click", () => {
        close();
        run();
      });
      list.append(b);
      return b;
    };
    item(t("Font…"), () => openFont(ed, runTarget));
    const para = item(t("Paragraph…"), () => openParagraph(ed, styleTarget));
    const tabs = item(t("Tabs…"), () => openTabs(ed, styleTarget));
    const borders = item(t("Borders and shading…"), () => openBordersShading(ed, styleTarget));
    item(t("Language…"), () => openStyleLanguage({ current: draft.lang ?? details?.lang, apply: (l) => ((draft.lang = l), refresh()), done: () => undefined }));
    const numbering = item(t("Numbering…"), () =>
      openStyleNumbering({
        currentNumId: draft.list ? (draft.list.kind === "existing" ? draft.list.numId : undefined) : (details?.ownPara.numId ?? undefined),
        lists: engine.listDefinitions(),
        apply: (l) => ((draft.list = l), refresh()),
        done: () => undefined,
      }),
    );
    if (draft.kind === "character") for (const b of [para, tabs, borders, numbering]) b.disabled = true;
  });

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, details ? t("OK") : t("Create"));
  // A style of the default design can also be kept for new documents; others go with a template.
  const keepable = !!details && DEFAULT_DESIGN_STYLES.has(details.id);
  const template = h("input", { id: "ms-template", type: "radio", name: "ms-scope", disabled: !keepable });
  const field = (label: string, control: HTMLElement, id: string) => h("div", { class: "field" }, h("label", { for: id }, label), control);
  const title = details ? t("Modify style") : t("New style");
  const dialog = h(
    "div",
    { class: "dialog modify-style", role: "dialog", "aria-modal": "true", "aria-label": title },
    h("div", { class: "dialog-head" }, h("h2", {}, title)),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "pd-grid" }, field(t("Name"), name, "ms-name"), field(t("Style type"), kindSel, "ms-kind"), field(t("Based on"), basedSel, "ms-based"), field(t("Style for the following paragraph"), nextSel, "ms-next")),
      h(
        "div",
        { class: "field" },
        h("label", {}, t("Formatting")),
        h("div", { class: "ms-row", role: "toolbar", "aria-label": t("Formatting") }, fontSel, sizeIn, bold, italic, underline, colorBtn, h("span", { class: "sep" }), ...alignBtns, h("span", { class: "sep" }), lineSel, h("span", { class: "ms-break" }), h("label", { class: "ms-mini", for: "ms-before" }, t("Before")), beforeIn, h("label", { class: "ms-mini", for: "ms-after" }, t("After")), afterIn),
      ),
      preview,
      description,
      h("div", { class: "ms-checks" }, h("label", { class: "check", for: "ms-quick" }, quick, h("span", {}, t("Add to the style gallery"))), h("label", { class: "check", for: "ms-auto" }, auto, h("span", {}, t("Update automatically")))),
      h(
        "div",
        { class: "ms-checks ms-scopes", role: "radiogroup", "aria-label": t("Apply to") },
        h("label", { class: "check", for: "ms-here" }, h("input", { id: "ms-here", type: "radio", name: "ms-scope", checked: true }), h("span", {}, t("Only in this document"))),
        h("label", { class: "check", for: "ms-template" }, template, h("span", {}, t("New documents based on this template"))),
      ),
      h(
        "p",
        { class: "help ms-scope" },
        keepable
          ? t("New documents start with this style too: it joins the design new documents start with, as Design, Set as default does.")
          : t("Only Normal, Title, Subtitle, Headings and Quote can be kept for new documents this way. To keep other styles, save the document as a template."),
      ),
      err,
    ),
    h("div", { class: "dialog-actions" }, formatMenu, cancel, ok),
  );

  // -- events -----------------------------------------------------------------
  name.addEventListener("input", () => (draft.name = name.value));
  kindSel.addEventListener("change", () => {
    draft.kind = kindSel.value as Kind;
    fillBased();
    refresh();
  });
  basedSel.addEventListener("change", () => {
    draft.basedOn = basedSel.value;
    refresh();
  });
  nextSel.addEventListener("change", () => {
    draft.next = nextSel.value;
    refresh();
  });
  fontSel.addEventListener("change", () => {
    draft.rpr.font = fontSel.value;
    refresh();
  });
  sizeIn.addEventListener("change", () => {
    const v = parseFloat(sizeIn.value);
    if (v > 0 && v < 1000) draft.rpr.size = v;
    refresh();
  });

  const definition = (): StyleDefinition => {
    const def: StyleDefinition = {};
    const nm = draft.name.trim();
    if (!details || nm !== details.name) def.name = nm;
    if (!details) def.kind = draft.kind;
    if (!details || draft.basedOn !== (details.basedOn ?? "")) def.basedOn = draft.basedOn;
    if (!details || draft.next !== (details.next ?? "")) def.next = draft.next;
    if (!details || draft.quick !== details.quick) def.quick = draft.quick;
    if (!details || draft.autoUpdate !== details.autoUpdate) def.autoUpdate = draft.autoUpdate;
    if (Object.keys(draft.ppr).length) def.ppr = draft.ppr;
    if (Object.keys(draft.rpr).length) def.rpr = draft.rpr;
    if (draft.lang !== undefined) def.lang = draft.lang;
    return def;
  };

  const close = modal(dialog, () => ed.focus());
  fillBased();
  refresh();
  if (!details) {
    name.value = draft.name = engine.styles().some((s) => s.name === "Style1") ? "" : "Style1";
    name.select();
  }
  cancel.addEventListener("click", close);
  const submit = () => {
    err.textContent = "";
    if (!draft.name.trim()) {
      err.textContent = t("Give the style a name.");
      name.focus();
      return;
    }
    const def = definition();
    const fail = (m: string) => (err.textContent = m);
    // A list the style should join is made first, in the same undo step as the style.
    let merge = false;
    const list = draft.list;
    if (list) {
      let numId: number | undefined;
      if (list.kind === "existing") numId = list.numId;
      else if (list.kind === "new") {
        const r = ed.exec({ op: "define_list", def: { levels: [{ format: list.format, text: list.text, left: 720, hanging: 360 }] } }, { onError: fail });
        if (!r) return;
        numId = r[0]?.count;
        merge = true;
      }
      def.ppr = { ...(def.ppr ?? {}), num: [numId ?? 0, 0] };
    }
    const op: Op = { op: "set_style_definition", ...(details ? { style: details.id } : {}), def };
    const res = ed.exec(op, { merge, select: ed.sel, keepExtra: true, onError: fail });
    if (!res) return;
    close();
    ed.focus();
    if (keepable && template.checked) void host.execute("design.setAsDefault", { quiet: true }).catch(() => undefined);
  };
  ok.addEventListener("click", submit);
  dialog.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT" && (e.target as HTMLInputElement).type !== "checkbox") {
      e.preventDefault();
      submit();
    }
  });
}

/** The text of a style's name for dialogs that list styles. */
export function styleName(s: { name: string; custom?: boolean }): string {
  return styleTitle(s.name, s.custom);
}
