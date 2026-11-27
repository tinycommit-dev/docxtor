import { openHistoryList } from "./history-list";
import { visualAlign, type VisualAlign } from "../editor/align";
import type { Editor } from "../editor/editor";
import { fontStack } from "../editor/render";
import { fontStatus, MENU_FONTS } from "../fonts";
import type { FormatState } from "@docxtor/core";
import { msg, t, type ExtensionHost, type ToolbarItem } from "@docxtor/extensions";
import { coalesce, h, popover, svg } from "./dom";
import { I, type IconName } from "./icons";
import { PARAGRAPH_ICONS } from "./paragraph-icons";
import { BULLET_PRESETS, NUMBER_PRESETS } from "./list-styles";
import { colorPalette, colorPatch } from "./swatches";
import { openStyleGallery } from "./style-gallery";
import { styleTitle } from "./style-preview";
import { hex } from "../editor/color.ts";

const STYLES: { id: string; label: string; css: string }[] = [
  { id: "Normal", label: msg("Normal text"), css: "font-size:13px" },
  { id: "Title", label: msg("Title"), css: "font:500 22px var(--display)" },
  { id: "Subtitle", label: msg("Subtitle"), css: "font-size:15px;color:var(--muted)" },
  { id: "Heading 1", label: msg("Heading 1"), css: "font-size:19px;font-weight:600" },
  { id: "Heading 2", label: msg("Heading 2"), css: "font-size:16px;font-weight:600" },
  { id: "Heading 3", label: msg("Heading 3"), css: "font-size:14px;font-weight:600" },
  { id: "Heading 4", label: msg("Heading 4"), css: "font-size:13px;font-weight:600;font-style:italic" },
  { id: "Quote", label: msg("Quote"), css: "font-style:italic;color:var(--ink-2);border-left:2px solid var(--line-2);padding-left:8px" },
  { id: "Code", label: msg("Code"), css: "font:12px ui-monospace,Menlo,monospace;background:var(--hover);padding:2px 6px;border-radius:4px" },
];

const FONTS = MENU_FONTS;

const ALIGNS = [
  { id: "left", icon: "alignLeft", title: msg("Align left"), command: "format.alignLeft" },
  { id: "center", icon: "alignCenter", title: msg("Center"), command: "format.alignCenter" },
  { id: "right", icon: "alignRight", title: msg("Align right"), command: "format.alignRight" },
  { id: "both", icon: "alignJustify", title: msg("Justify"), command: "format.justify" },
];

const DIRECTIONS = [
  { id: "ltr", icon: "textLtr", title: msg("Left-to-right"), command: "format.ltr" },
  { id: "rtl", icon: "textRtl", title: msg("Right-to-left"), command: "format.rtl" },
] as const;

/** The colours of the Text color popover (hex), which Borders and shading offers too. */
export const TEXT_COLORS = [
  "000000", "434343", "666666", "999999", "B7B7B7", "D9D9D9", "EFEFEF", "FFFFFF",
  "C0392B", "E67E22", "F1C40F", "27AE60", "16A085", "2980B9", "8E44AD", "D35400",
  "922B21", "A04000", "9A7D0A", "196F3D", "0E6655", "1F4E79", "5B2C6F", "6E2C00",
];

/** A place in the document a link can point at. */
interface LinkTarget {
  kind: "bookmark" | "heading";
  /** The bookmark's name (a link to it is `#name`). */
  name: string;
  label: string;
}

/** Word's sixteen highlight colours: the name the file uses, the colour, and the name people see. */
const HIGHLIGHTS: [string, string, string][] = [
  ["yellow", "#ffff00", msg("Yellow")], ["green", "#00ff00", msg("Bright green")], ["cyan", "#00ffff", msg("Turquoise")], ["magenta", "#ff00ff", msg("Pink")],
  ["blue", "#0000ff", msg("Blue")], ["red", "#ff0000", msg("Red")], ["darkBlue", "#000080", msg("Dark blue")], ["darkCyan", "#008080", msg("Teal")],
  ["darkGreen", "#008000", msg("Green")], ["darkMagenta", "#800080", msg("Violet")], ["darkRed", "#800000", msg("Dark red")], ["darkYellow", "#808000", msg("Dark yellow")],
  ["darkGray", "#808080", msg("Gray 50%")], ["lightGray", "#c0c0c0", msg("Gray 25%")], ["black", "#000000", msg("Black")], ["white", "#ffffff", msg("White")],
];

export class Toolbar {
  el: HTMLElement;
  private btns: Record<string, HTMLButtonElement> = {};
  private styleLabel!: HTMLElement;
  private fontLabel!: HTMLElement;
  private alignIcon!: HTMLElement;
  private sizeInput!: HTMLInputElement;
  private colorBar!: HTMLElement;
  private hlBar!: HTMLElement;
  private state: FormatState | null = null;
  /** Where contributed buttons go, by group. */
  private slots: Record<string, HTMLElement> = {};
  private extGroup!: HTMLElement;
  private askGroup: HTMLElement | null = null;
  private contributed: { el: HTMLButtonElement; item: ToolbarItem }[] = [];

  constructor(
    private ed: Editor,
    /** Ask the AI about the selection; without it the toolbar has no Assistant group. */
    private ask: (() => void) | null = null,
    /** Extensions add buttons through it. */
    private host: ExtensionHost | null = null,
  ) {
    this.el = h("div", { class: "toolbar", role: "toolbar", "aria-label": t("Formatting") });
    // Keep the editor selection when clicking toolbar buttons.
    this.el.addEventListener("mousedown", (e) => {
      if (!(e.target as HTMLElement).closest("input")) e.preventDefault();
    });
    this.build();
    // An icon pack turned on or off or chosen: draw the toolbar's icons again.
    host?.onDidChangeIcons(() => {
      for (const el of Array.from(this.el.querySelectorAll<SVGElement>("svg[data-icon]"))) el.replaceWith(svg(this.icon(el.dataset.icon as IconName)));
    });
  }

  /** Buttons whose tooltip shows a command's shortcut, refreshed as commands are registered. */
  private tips: { b: HTMLButtonElement; label: string; command: string }[] = [];

  /** `label` is already translated; `command` names the command whose shortcut the tooltip shows. */
  /** An icon from the shared set (extensions can replace it), tagged so it can be redrawn. */
  private icon(id: IconName): string {
    return (this.host?.icon(id) ?? I[id]).replace("<svg", `<svg data-icon="${id}"`);
  }

  private btn(key: string, content: string | Node, title: string, onClick: (b: HTMLButtonElement) => void, cls = "", command?: string): HTMLButtonElement {
    const b = h("button", { class: `tb ${cls}`, title, "aria-label": title, type: "button" });
    if (command) this.tips.push({ b, label: title, command });
    if (typeof content === "string" && content.startsWith("<svg")) b.append(svg(content));
    else b.append(content);
    b.addEventListener("click", () => onClick(b));
    this.btns[key] = b;
    return b;
  }

  /** The narrow chevron key beside a list key, which opens its gallery of styles. */
  private moreBtn(key: string, title: string, open: (b: HTMLButtonElement) => void): HTMLButtonElement {
    const b = this.btn(key, this.icon("chevron"), title, open, "tb-more");
    b.firstElementChild?.classList.add("caret");
    b.setAttribute("aria-haspopup", "menu");
    b.setAttribute("aria-expanded", "false");
    return b;
  }

  private build() {
    const e = this.ed;

    // Style picker
    this.styleLabel = h("span", { class: "label" }, t("Normal text"));
    const styleBtn = this.btn("style", this.styleLabel, t("Paragraph style"), (b) => this.stylePopover(b), "style-pick");
    styleBtn.setAttribute("aria-label", `${t("Paragraph style")}: ${this.styleLabel.textContent}`);
    styleBtn.append(svg(this.icon("chevron")));
    styleBtn.lastElementChild!.classList.add("caret");

    this.fontLabel = h("span", { class: "label" }, "Calibri");
    const fontBtn = this.btn("font", this.fontLabel, t("Font"), (b) => this.fontPopover(b), "font-pick");
    fontBtn.setAttribute("aria-label", `${t("Font")}: ${this.fontLabel.textContent}`);
    fontBtn.append(svg(this.icon("chevron")));
    fontBtn.lastElementChild!.classList.add("caret");

    this.sizeInput = h("input", { type: "text", inputmode: "decimal", "aria-label": t("Font size"), value: "11" });
    this.sizeInput.addEventListener("keydown", (ev) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        const v = parseFloat(this.sizeInput.value);
        if (v > 0 && v < 1000) e.setRun({ size: v });
        e.focus();
      }
    });
    const step = (d: 1 | -1) => e.stepFontSize(d);
    const sizeBox = h(
      "div",
      { class: "size-box" },
      this.btn("szdn", this.icon("minus"), t("Decrease font size"), () => step(-1)),
      this.sizeInput,
      this.btn("szup", this.icon("plus"), t("Increase font size"), () => step(1)),
    );

    this.colorBar = h("span", { class: "color-bar", style: "background:#000" });
    const colorBtn = this.btn("color", this.icon("textColor"), t("Text color"), (b) => this.colorPopover(b));
    colorBtn.append(this.colorBar);
    this.hlBar = h("span", { class: "color-bar", style: "background:transparent" });
    const hlBtn = this.btn("hl", this.icon("highlight"), t("Highlight"), (b) => this.highlightPopover(b));
    hlBtn.append(this.hlBar);

    const fileInput = h("input", { type: "file", accept: "image/*", style: "display:none" });
    fileInput.addEventListener("change", () => {
      const f = fileInput.files?.[0];
      if (f) void e.insertImage(f);
      fileInput.value = "";
    });

    // Alignment: one key showing the current alignment, opening all four (saves three keys).
    this.alignIcon = h("span", { class: "align-now" }, svg(this.icon("alignLeft")));
    const alignBtn = this.btn("align", this.alignIcon, t("Align"), (b) => this.alignPopover(b));
    alignBtn.append(svg(this.icon("chevron")));
    alignBtn.lastElementChild!.classList.add("caret");

    const group = (key: string, label: string, ...controls: Node[]) => {
      const box = h("div", { class: "tgroup-controls" }, ...controls);
      this.slots[key] = box;
      return h("div", { class: `tgroup tgroup-${key}`, role: "group", "aria-label": label }, box);
    };
    // The format painter: a click picks the formatting up for one use; a double-click keeps it on until Escape or another click.
    const painter = this.btn("painter", this.icon("brush"), t("Format painter"), () => undefined, "", "format.painter");
    painter.setAttribute("aria-pressed", "false");
    painter.addEventListener("click", (ev) => {
      if (ev.detail < 2) e.painter.toggle();
    });
    painter.addEventListener("dblclick", () => e.painter.arm(true));
    const painting = () => {
      painter.classList.toggle("on", e.painter.on);
      painter.setAttribute("aria-pressed", String(e.painter.on));
    };
    e.canvas.addEventListener("docxtor-painter", painting);
    e.life.defer(() => e.canvas.removeEventListener("docxtor-painter", painting));
    const ask = this.ask ? this.btn("ask", h("span", { class: "lamp" }), t("Ask AI about the selection"), () => this.ask?.(), "ask", "view.assistant") : null;
    ask?.append(t("Ask"));
    this.el.append(
      group(
        "edit",
        t("Edit"),
        this.btn("undo", this.icon("undo"), t("Undo"), () => e.undo(), "", "edit.undo"),
        this.moreBtn("undoList", t("Undo history"), (b) => openHistoryList(e, b, "undo")),
        this.btn("redo", this.icon("redo"), t("Redo"), () => e.redo(), "", "edit.redo"),
        this.moreBtn("redoList", t("Redo history"), (b) => openHistoryList(e, b, "redo")),
        painter,
      ),
      group("style", t("Style"), styleBtn),
      group("type", t("Type"), fontBtn, sizeBox),
      group(
        "text",
        t("Text"),
        this.btn("bold", "B", t("Bold"), () => e.toggleMark("bold"), "glyph", "format.bold"),
        this.btn("italic", "I", t("Italic"), () => e.toggleMark("italic"), "glyph i", "format.italic"),
        this.btn("underline", "U", t("Underline"), () => e.toggleMark("underline"), "glyph u", "format.underline"),
        this.btn("strike", "S", t("Strikethrough"), () => e.toggleMark("strike"), "glyph s", "format.strike"),
        colorBtn,
        hlBtn,
        this.btn("clear", this.icon("clear"), t("Clear formatting"), () => e.clearFormatting(), "", "format.clear"),
      ),
      group(
        "paragraph",
        t("Paragraph"),
        alignBtn,
        this.btn("spacing", this.icon("lineSpacing"), t("Line and paragraph spacing"), (b) => this.spacingPopover(b)),
        h("div", { class: "sep" }),
        this.btn("bullet", this.icon("bullets"), t("Bulleted list"), () => e.toggleList("bullet"), "", "format.bullets"),
        this.moreBtn("bulletStyles", t("Bullet styles"), (b) => this.listGallery(b, "bullet")),
        this.btn("decimal", this.icon("numbers"), t("Numbered list"), () => e.toggleList("decimal"), "", "format.numbers"),
        this.moreBtn("numberStyles", t("Numbering styles"), (b) => this.listGallery(b, "number")),
        this.btn("checklist", this.icon("checklist"), t("Checklist"), () => e.toggleChecklist(), "", "format.checklist"),
        this.btn("outdent", this.icon("outdent"), t("Decrease indent"), () => e.indent(-1), "", "format.outdent"),
        this.btn("indent", this.icon("indent"), t("Increase indent"), () => e.indent(1), "", "format.indent"),
      ),
      group(
        "insert",
        t("Insert"),
        this.btn("link", this.icon("link"), t("Link"), (b) => this.linkPopover(b), "", "insert.link"),
        this.btn("image", this.icon("image"), t("Image"), () => fileInput.click()),
        this.btn("table", this.icon("table"), t("Table"), (b) => this.tablePopover(b)),
        this.btn("pagebreak", this.icon("pageBreak"), t("Page break"), () => e.insertPageBreak(), "", "insert.pageBreak"),
        this.btn("footnote", this.icon("footnote"), t("Footnote"), () => e.insertFootnote(), "", "insert.footnote"),
        this.btn("hf", this.icon("headerFooter"), t("Header and footer"), (b) => this.headerFooterPopover(b)),
        this.btn("toc", this.icon("toc"), t("Table of contents"), (b) => this.tocPopover(b)),
        fileInput,
      ),
    );
    this.extGroup = group("extensions", t("Extensions"));
    if (ask) this.el.append((this.askGroup = group("assistant", t("Assistant"), ask)));
    if (this.host) {
      const render = coalesce(() => this.renderContributions());
      const subs = [this.host.toolbar.onChange(render), this.host.commands.onChange(render)];
      e.life.defer(() => subs.forEach((d) => d.dispose()));
      this.renderContributions();
    }
  }

  /** Buttons extensions added: in the group they asked for, or in an Extensions group before Assistant. */
  private renderContributions() {
    const host = this.host;
    if (!host) return;
    for (const { b, label, command } of this.tips) {
      const key = host.shortcut(command);
      b.title = key ? `${label} (${key})` : label;
    }
    for (const c of this.contributed) c.el.remove();
    this.contributed = [];
    for (const item of host.toolbar.list()) {
      const cmd = host.command(item.command);
      const title = item.title ?? cmd?.title ?? item.label ?? item.id;
      const key = host.shortcut(item.command);
      const b = h("button", { class: "tb", type: "button", title: key ? `${title} (${key})` : title, "aria-label": title });
      if (item.icon?.trim().startsWith("<svg")) b.append(svg(item.icon));
      else b.append(item.label ?? item.icon ?? title);
      b.addEventListener("click", () => void host.execute(item.command, ...(item.args ?? [])).catch(() => undefined));
      (this.slots[item.group ?? "extensions"] ?? this.slots.extensions).append(b);
      this.contributed.push({ el: b, item });
    }
    const hasOwn = this.slots.extensions.childElementCount > 0;
    if (hasOwn && !this.extGroup.isConnected) this.el.insertBefore(this.extGroup, this.askGroup);
    else if (!hasOwn) this.extGroup.remove();
    this.updateContributed();
  }

  private updateContributed() {
    for (const { el, item } of this.contributed) {
      const cmd = this.host?.command(item.command);
      el.disabled = !cmd || !this.host!.enabled(cmd);
      if (item.isActive) {
        let on = false;
        try {
          on = item.isActive(this.state);
        } catch {
          /* a broken extension shouldn't break the toolbar */
        }
        el.classList.toggle("on", on);
      }
    }
  }

  update(f: FormatState | null) {
    this.state = f;
    const on = (k: string, v: boolean) => this.btns[k]?.classList.toggle("on", v);
    const r = f?.run ?? {};
    on("bold", !!r.bold);
    on("italic", !!r.italic);
    on("underline", !!r.underline && r.underline !== "none");
    on("strike", !!r.strike);
    const now = ALIGNS.find((a) => a.id === this.alignment()) ?? ALIGNS[0];
    if (this.alignIcon.dataset.align !== now.id) {
      this.alignIcon.dataset.align = now.id;
      this.alignIcon.replaceChildren(svg(this.icon(now.icon as IconName)));
    }
    on("bullet", f?.list === "bullet");
    on("decimal", f?.list === "decimal");
    on("checklist", f?.list === "checklist");
    on("link", !!f?.link);
    on("table", !!f?.inTable);
    const styleId = f?.style ?? "Normal";
    const known = STYLES.find((s) => s.id.replace(/\s/g, "") === styleId.replace(/\s/g, ""));
    const styles = this.ed.engine?.styles() ?? [];
    const info = styles.find((s) => s.id === styleId);
    // Text with a character style shows that style's name, as Word's style box does.
    const charInfo = f?.charStyle ? styles.find((s) => s.id === f.charStyle) : undefined;
    // Written only when different: a keystroke asks for all of this, and a write (even of the same text) makes the browser style and lay out again.
    const text = (el: HTMLElement, v: string) => {
      if (el.textContent !== v) el.textContent = v;
    };
    const css = (el: HTMLElement, prop: "fontFamily" | "background", v: string) => {
      if (el.style[prop] !== v) el.style[prop] = v;
    };
    text(this.styleLabel, charInfo ? styleTitle(charInfo.name, charInfo.custom) : known ? t(known.label) : info ? styleTitle(info.name, info.custom) : styleId);
    text(this.fontLabel, r.font ?? t("Default"));
    // A name that holds the words on the key ("Font: Calibri"), so voice control ("click Calibri") finds it (WCAG 2.5.3).
    const named = (b: HTMLElement | undefined, title: string, label: HTMLElement) => {
      const now = `${title}: ${label.textContent}`;
      if (b && b.getAttribute("aria-label") !== now) b.setAttribute("aria-label", now);
    };
    named(this.btns.style, t("Paragraph style"), this.styleLabel);
    named(this.btns.font, t("Font"), this.fontLabel);
    css(this.fontLabel, "fontFamily", fontStack(r.font) ?? "");
    const size = r.size ? String(r.size) : "";
    if (document.activeElement !== this.sizeInput && this.sizeInput.value !== size) this.sizeInput.value = size;
    css(this.colorBar, "background", hex(r.color) ?? "var(--ink)");
    const hl = HIGHLIGHTS.find(([n]) => n === r.highlight)?.[1];
    css(this.hlBar, "background", hl ?? hex(r.shading) ?? "transparent");
    this.btns.undo.disabled = this.btns.undoList.disabled = !this.ed.engine?.raw.canUndo();
    this.btns.redo.disabled = this.btns.redoList.disabled = !this.ed.engine?.raw.canRedo();
    this.updateContributed();
  }

  /** The style gallery: every style drawn as it looks, then the commands that make and change styles (see style-gallery.ts). */
  private stylePopover(anchor: HTMLElement) {
    if (this.host) openStyleGallery({ ed: this.ed, host: this.host, state: this.state }, anchor);
  }

  private fontPopover(anchor: HTMLElement) {
    const list = h("div", {});
    const close = popover(anchor, list);
    const used = new Set<string>((this.ed.engine?.fontsUsed() ?? []).map((u) => u.family).filter((f) => !FONTS.includes(f)));
    const offered = (this.host?.fonts.list() ?? []).map((f) => f.family);
    const add = (f: string) => {
      // Like Figma: a font this device can't show (and that has no look-alike) says so.
      const missing = fontStatus(f, offered) === "missing";
      const item = h(
        "button",
        { class: "menu-item", type: "button", style: `font-family:${fontStack(f)};font-size:14px` },
        f,
        missing ? h("span", { class: "hint", style: "font-family:var(--font-ui)" }, t("Not available")) : null,
      );
      if (this.state?.run.font === f) item.classList.add("active");
      item.addEventListener("click", () => {
        close();
        this.ed.setRun({ font: f });
      });
      list.append(item);
    };
    FONTS.forEach(add);
    // Fonts extensions added (Google Fonts, a company's own), alphabetically.
    const more = (this.host?.fonts.list() ?? [])
      .filter((f) => f.menu !== false && !FONTS.includes(f.family))
      .map((f) => f.family)
      .filter((f, i, all) => all.indexOf(f) === i)
      .sort((a, b) => a.localeCompare(b));
    for (const f of more) used.delete(f);
    if (more.length) {
      list.append(h("div", { class: "menu-sep" }), h("div", { class: "menu-title" }, t("More fonts")));
      more.forEach(add);
    }
    if (used.size) {
      list.append(h("div", { class: "menu-sep" }), h("div", { class: "menu-title" }, t("In this document")));
      [...used].slice(0, 12).forEach(add);
    }
  }

  private colorPopover(anchor: HTMLElement) {
    const now = this.state?.run.color;
    const auto = !now || now === "auto";
    const box = h("div", {});
    const close = popover(anchor, box);
    box.append(
      colorPalette({
        title: t("Text color"),
        theme: this.ed.engine?.themeColors() ?? null,
        current: auto ? null : now,
        automatic: {
          label: t("Automatic"),
          on: auto,
          run: () => {
            close();
            this.ed.setRun({ color: "auto" });
          },
        },
        pick: (c) => {
          close();
          this.ed.setRun(colorPatch(c));
        },
        beforeMore: close,
      }).el,
    );
  }

  private highlightPopover(anchor: HTMLElement) {
    const grid = h("div", { class: "swatches" });
    const close = popover(anchor, h("div", {}, h("div", { class: "menu-title" }, t("Highlight")), grid));
    for (const [name, css, label] of HIGHLIGHTS) {
      const s = h("button", { class: "swatch", type: "button", title: t(label), "aria-label": t(label), "aria-pressed": String(this.state?.run.highlight === name), style: `background:${css}` });
      if (this.state?.run.highlight === name) s.classList.add("on");
      s.addEventListener("click", () => {
        close();
        this.ed.setRun({ highlight: name });
      });
      grid.append(s);
    }
    const none = h("button", { class: "menu-item", type: "button" }, t("No highlight"));
    none.addEventListener("click", () => {
      close();
      this.ed.setRun({ highlight: "none" });
    });
    grid.parentElement!.append(none);
  }

  /** Open the link editor for the current selection. */
  openLink() {
    this.linkPopover(this.btns.link);
  }

  private linkPopover(anchor: HTMLElement) {
    const input = h("input", { type: "url", placeholder: "https://…", value: this.state?.link ?? "", style: "width:280px;height:32px;border:1px solid var(--line-2);border-radius:8px;padding:0 10px;background:var(--surface-2)" });
    const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
    const remove = h("button", { class: "btn ghost", type: "button" }, t("Remove"));
    const targets = this.linkTargets();
    const box = h("div", { style: "display:flex;flex-direction:column;gap:8px;padding:6px" }, input);
    // Places in this document: bookmarks, and headings (which have a hidden bookmark once a table of contents numbers them).
    let pick = (_target: LinkTarget) => {};
    if (targets.length) {
      const list = h("div", { class: "link-targets" });
      for (const [title, kind] of [[t("Bookmarks"), "bookmark"], [t("Headings"), "heading"]] as const) {
        const group = targets.filter((x) => x.kind === kind);
        if (!group.length) continue;
        list.append(h("div", { class: "menu-title" }, title));
        for (const target of group) {
          const b = h("button", { class: "menu-item", type: "button", "data-link-target": target.name }, h("span", { class: "menu-label" }, target.label));
          b.addEventListener("click", () => pick(target));
          list.append(b);
        }
      }
      box.append(list);
    }
    box.append(h("div", { style: "display:flex;gap:6px;justify-content:flex-end" }, this.state?.link ? remove : null, apply));
    const close = popover(anchor, box);
    pick = (target) => {
      close();
      this.ed.setLink(`#${target.name}`, target.label);
      this.ed.focus();
    };
    setTimeout(() => input.focus());
    const go = () => {
      const url = input.value.trim();
      close();
      if (url) this.ed.setLink(/^[a-z]+:|^#/i.test(url) ? url : `https://${url}`);
      this.ed.focus();
    };
    apply.addEventListener("click", go);
    remove.addEventListener("click", () => {
      close();
      this.ed.removeLink();
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") go();
    });
  }

  /** The places in this document a link can go to. */
  private linkTargets(): LinkTarget[] {
    const out: LinkTarget[] = this.ed.bookmarks().map((b) => ({ kind: "bookmark", name: b.name, label: b.name }));
    const seen = new Set<string>();
    for (const p of this.ed.flow.querySelectorAll<HTMLElement>(".fp[data-bm][data-style]")) {
      if (!/^Heading[1-6]$/.test(p.dataset.style ?? "")) continue;
      const name = p.dataset.bm!.split(" ").find((n) => n.startsWith("_Toc"));
      const label = Array.from(p.childNodes)
        .filter((n) => !(n instanceof HTMLElement && (n.classList.contains("lbl") || n.classList.contains("pgsp"))))
        .map((n) => n.textContent)
        .join("")
        .trim();
      if (!name || !label || seen.has(name)) continue;
      seen.add(name);
      out.push({ kind: "heading", name, label });
    }
    return out;
  }

  private headerFooterPopover(anchor: HTMLElement) {
    const e = this.ed;
    const list = h("div", {});
    const close = popover(anchor, list);
    const page = () => Math.max(0, e.currentPage() - 1);
    const item = (label: string, hint: string, fn: () => void) => {
      const b = h("button", { class: "menu-item", type: "button" }, label, hint ? h("span", { class: "hint" }, hint) : null);
      b.addEventListener("click", () => {
        close();
        fn();
      });
      list.append(b);
    };
    item(t("Edit header"), t("double-click the top margin"), () => e.enterStory("header", page()));
    item(t("Edit footer"), t("double-click the bottom margin"), () => e.enterStory("footer", page()));
    list.append(h("div", { class: "menu-sep" }));
    item(t("Add page numbers"), t("centred in the footer"), () => e.addPageNumbers(page()));
  }

  private tocPopover(anchor: HTMLElement) {
    const e = this.ed;
    const list = h("div", {});
    const close = popover(anchor, list);
    const item = (label: string, fn: () => void, disabled = false) => {
      const b = h("button", { class: "menu-item", type: "button", disabled }, label);
      b.addEventListener("click", () => {
        close();
        fn();
      });
      list.append(b);
    };
    item(t("Insert table of contents"), () => e.insertToc(), !!e.story);
    item(t("Update table of contents"), () => e.updateToc(), !e.hasToc());
  }

  private tablePopover(anchor: HTMLElement) {
    if (this.state?.inTable) {
      const list = h("div", {});
      const close = popover(anchor, list);
      const items: [string, string][] = [
        ["table.rowAbove", msg("Insert row above")],
        ["table.rowBelow", msg("Insert row below")],
        ["table.columnLeft", msg("Insert column left")],
        ["table.columnRight", msg("Insert column right")],
        ["", ""],
        ["table.deleteRow", msg("Delete row")],
        ["table.deleteColumn", msg("Delete column")],
        ["table.deleteTable", msg("Delete table")],
      ];
      for (const [a, label] of items) {
        if (!a) {
          list.append(h("div", { class: "menu-sep" }));
          continue;
        }
        const it = h("button", { class: "menu-item", type: "button" }, t(label));
        it.addEventListener("click", () => {
          close();
          void this.ed.command(a).catch(() => undefined);
        });
        list.append(it);
      }
      // A table can sit inside a cell, and the table's properties have a dialog of their own.
      list.append(h("div", { class: "menu-sep" }));
      for (const [id, label] of [["insert.tableDialog", msg("Insert table in cell…")], ["table.properties", msg("Table properties…")]] as const) {
        const it = h("button", { class: "menu-item", type: "button" }, t(label));
        it.addEventListener("click", () => {
          close();
          this.host?.execute(id);
        });
        list.append(it);
      }
      return;
    }
    const grid = h("div", { class: "grid-pick" });
    const label = h("div", { class: "grid-label" }, t("Insert table"));
    const more = h("button", { class: "menu-item", type: "button" }, t("Insert table…"));
    more.addEventListener("click", () => {
      close();
      this.host?.execute("insert.tableDialog");
    });
    const close = popover(anchor, h("div", {}, grid, label, more));
    const cells: HTMLElement[] = [];
    for (let r = 0; r < 8; r++)
      for (let c = 0; c < 8; c++) {
        const s = h("span", {});
        s.addEventListener("mouseenter", () => {
          cells.forEach((x, i) => x.classList.toggle("hl", Math.floor(i / 8) <= r && i % 8 <= c));
          label.textContent = `${r + 1} × ${c + 1}`;
        });
        s.addEventListener("click", () => {
          close();
          this.ed.insertTable(r + 1, c + 1);
        });
        cells.push(s);
        grid.append(s);
      }
  }

  /** The current paragraph's alignment as it shows on screen (a right-to-left paragraph reads left and right the other way round). */
  private alignment(): string {
    return visualAlign(this.state?.para.align, this.state?.para.bidi);
  }

  private alignPopover(anchor: HTMLElement) {
    const row = h("div", { class: "align-row", role: "group", "aria-label": t("Align") });
    const close = popover(anchor, row);
    const now = this.alignment();
    for (const a of ALIGNS) {
      const key = this.host?.shortcut(a.command);
      const title = key ? `${t(a.title)} (${key})` : t(a.title);
      const b = h("button", { class: `tb${a.id === now ? " on" : ""}`, type: "button", title, "aria-label": t(a.title), "aria-pressed": String(a.id === now) }, svg(this.icon(a.icon as IconName)));
      b.addEventListener("click", () => {
        close();
        this.ed.setAlign(a.id as VisualAlign);
      });
      row.append(b);
    }
    // The direction the paragraph's text runs in, beside the alignments.
    row.append(h("span", { class: "align-sep", "aria-hidden": "true" }));
    for (const d of DIRECTIONS) {
      const on = d.id === "rtl" ? !!this.state?.para.bidi : !this.state?.para.bidi;
      const key = this.host?.shortcut(d.command);
      const title = key ? `${t(d.title)} (${key})` : t(d.title);
      const b = h("button", { class: `tb${on ? " on" : ""}`, type: "button", title, "aria-label": t(d.title), "aria-pressed": String(on) }, svg(this.host?.icon(d.icon) ?? PARAGRAPH_ICONS[d.icon]));
      b.addEventListener("click", () => {
        close();
        this.ed.setDirection(d.id === "rtl");
      });
      row.append(b);
    }
    row.querySelector<HTMLElement>(".tb.on")?.focus();
  }

  /**
   * A 3 by 2 gallery of list styles, each tile a small drawing of three nested levels. The tile for
   * the style the caret is in shows as selected; arrow keys move between tiles, Enter applies one.
   */
  private listGallery(anchor: HTMLButtonElement, kind: "bullet" | "number") {
    const presets = kind === "bullet" ? BULLET_PRESETS : NUMBER_PRESETS;
    const grid = h("div", { class: "list-gallery", role: "group", "aria-label": kind === "bullet" ? t("Bullet styles") : t("Numbering styles") });
    const viaKeys = document.activeElement === anchor;
    let applied = false;
    const close = popover(anchor, grid, {
      onClose: () => {
        anchor.setAttribute("aria-expanded", "false");
        if (!applied) viaKeys ? anchor.focus() : this.ed.focus();
      },
    });
    anchor.setAttribute("aria-expanded", "true");
    const current = this.state?.listStyle;
    const tileFor = (title: string, face: HTMLElement, on: boolean, run: () => unknown) => {
      const tile = h("button", { class: "list-tile", type: "button", role: "menuitemradio", "aria-checked": String(on), "aria-label": title, title, tabindex: on ? 0 : -1 }, face);
      tile.addEventListener("click", () => {
        applied = true;
        close();
        void Promise.resolve(run()).catch(() => undefined).then(() => this.ed.focus());
      });
      grid.append(tile);
      return tile;
    };
    const tiles = presets.map((p) => {
      const face = h(
        "span",
        { class: "lt-face", "aria-hidden": "true" },
        ...p.levels.map((mark, i) => h("span", { class: "lt-row", style: `--lvl:${i}` }, h("span", { class: "lt-mark" }, mark), h("i", { class: "lt-line" }))),
      );
      return tileFor(t(p.title), face, p.id === current, () => (this.host ? this.host.execute("format.listStyle", p.id) : this.ed.setListStyle(p.id)));
    });
    // Lists the document has that aren't one of these styles (ones a person defined, or the file came with).
    const inDocument = (this.ed.engine?.listDefinitions() ?? []).filter((d) => d.bullet === (kind === "bullet"));
    if (inDocument.length) {
      grid.append(h("div", { class: "lg-title" }, t("In this document")));
      for (const d of inDocument) {
        const face = h(
          "span",
          { class: "lt-face", "aria-hidden": "true" },
          ...d.levels.map((l, i) =>
            h(
              "span",
              { class: "lt-row", style: `--lvl:${i}` },
              h("span", { class: "lt-mark", style: l.font ? `font-family:${fontStack(l.font)}` : "" }, l.pic ? h("img", { class: "lt-pic", src: this.ed.engine.mediaUrl(l.pic.src), alt: "" }) : l.text),
              h("i", { class: "lt-line" }),
            ),
          ),
        );
        const title = d.name ?? d.levels.map((l) => l.text).join("  ");
        const on = this.state?.listNum !== undefined && d.nums.includes(this.state.listNum);
        tiles.push(tileFor(title, face, on, () => (this.host ? this.host.execute("format.applyList", d.numId) : undefined)));
      }
    }
    // Define your own.
    const defineIds = kind === "bullet" ? ["format.defineBullet"] : ["format.defineNumberFormat", "format.defineMultilevel"];
    grid.append(h("div", { class: "lg-sep" }));
    const actions = defineIds.map((id) => {
      const b = h("button", { class: "lg-action", type: "button", role: "menuitem", tabindex: "-1" }, this.host?.command(id)?.title ?? id);
      b.addEventListener("click", () => {
        applied = true;
        close();
        void this.host?.execute(id).catch(() => undefined);
      });
      grid.append(b);
      return b;
    });
    const start = tiles.find((x) => x.getAttribute("aria-checked") === "true") ?? tiles[0];
    start.tabIndex = 0;
    grid.addEventListener("keydown", (ev) => {
      const i = tiles.indexOf(document.activeElement as HTMLButtonElement);
      const a = actions.indexOf(document.activeElement as HTMLButtonElement);
      if (ev.key === "Tab") return ev.preventDefault();
      // The define lines below the tiles are a short list of their own: the arrows move between them and the tiles.
      if (a >= 0) {
        if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp") return;
        ev.preventDefault();
        const to = a + (ev.key === "ArrowDown" ? 1 : -1);
        if (to >= 0 && to < actions.length) actions[to].focus();
        else if (to < 0) (tiles.find((x) => x.tabIndex === 0) ?? tiles[tiles.length - 1]).focus();
        return;
      }
      const step: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: 3, ArrowUp: -3 };
      let to = -1;
      if (ev.key in step && i >= 0) to = i + step[ev.key];
      else if (ev.key === "Home") to = 0;
      else if (ev.key === "End") to = tiles.length - 1;
      else return;
      ev.preventDefault();
      if (to >= tiles.length && ev.key === "ArrowDown") {
        actions[0]?.focus();
        return;
      }
      if (to < 0 || to >= tiles.length) return;
      for (const x of tiles) x.tabIndex = -1;
      tiles[to].tabIndex = 0;
      tiles[to].focus();
    });
    start.focus();
  }

  private spacingPopover(anchor: HTMLElement) {
    const list = h("div", {});
    const close = popover(anchor, list);
    const cur = this.state?.para;
    const curMult = cur?.line && (cur.lineRule ?? "auto") === "auto" ? cur.line / 240 : null;
    for (const m of [1, 1.15, 1.5, 2]) {
      const it = h("button", { class: "menu-item", type: "button" }, m === 1 ? t("Single") : m === 2 ? t("Double") : String(m));
      if (curMult && Math.abs(curMult - m) < 0.02) it.classList.add("active");
      it.addEventListener("click", () => {
        close();
        this.ed.setPara({ lineMultiple: m });
      });
      list.append(it);
    }
    list.append(h("div", { class: "menu-sep" }));
    const before = (cur?.spaceBefore ?? 0) > 0;
    const after = (cur?.spaceAfter ?? 0) > 0;
    const b1 = h("button", { class: "menu-item", type: "button" }, before ? t("Remove space before") : t("Add space before"));
    b1.addEventListener("click", () => {
      close();
      this.ed.setPara({ spaceBefore: before ? 0 : 240 });
    });
    const b2 = h("button", { class: "menu-item", type: "button" }, after ? t("Remove space after") : t("Add space after"));
    b2.addEventListener("click", () => {
      close();
      this.ed.setPara({ spaceAfter: after ? 0 : 160 });
    });
    list.append(b1, b2);
  }
}
