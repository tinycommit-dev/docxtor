// The sheets: line numbers, headers and footers, and the fields the layout fills.
import type { Paginator } from "./layout";
import { isFar } from "./far";
import { renderBlocks, tw } from "./render";
import { bordersShow, pageBorderEl, pageFillCss } from "./page-art";
import { watermarkElement } from "./watermark";
import type { StoryZone } from "./layout-types";
import type { RBlock } from "@docxtor/core";
import { layoutTabs } from "./tabs";
import type { RefPara } from "./field-text";
import { dateElementText, styleKey, styleRefText } from "./field-text";
import { DYNAMIC, pageLabel } from "./layout-units";

export const sheetMethods = {
  /**
   * Number the lines of the sections that ask for it. The numbers are drawn on the sheets, beside the
   * text and outside the editable flow, so they are not part of the text, the selection or its model.
   */
  drawLineNumbers(this: Paginator) {
    const p = this.setup;
    const secs = p?.sections ?? [];
    if (!p || !secs.some((s) => s.lineNumbers)) return;
    interface Line {
      top: number;
      bottom: number;
      page: number;
      section: number;
      font: string;
      /** Left edge of the column the line is in, relative to its sheet (unset: the section's left margin). */
      x?: number;
      /** Not a line but the lines of a block that is not drawn, counted (on `page`), which have no place to draw. */
      count?: number;
      /** The block (in `flat`) the line belongs to. */
      block: number;
    }
    const lines: Line[] = [];
    // Word draws them in its "Line Number" style, which is the document's normal text: one face and size
    // for every number (the first Normal paragraph's, or the flow's own), in the document's text colour.
    const flat = this.flatBlocks();
    const normal = flat.find((c): c is HTMLElement => c.classList.contains("fp") && (!(c as HTMLElement).dataset.style || (c as HTMLElement).dataset.style === "Normal") && !c.querySelector(".lbl"));
    const base = getComputedStyle(normal ?? this.flow);
    const font = `font-size:${base.fontSize};font-family:${base.fontFamily};color:${getComputedStyle(this.flow).color}`;
    const secOf = this.sectionsOf(flat);
    const colsOf = new Map<HTMLElement, (r: DOMRect) => number>();
    flat.forEach((el, i) => {
      const section = secOf[i];
      if (!secs[section]?.lineNumbers || !el.classList.contains("fp") || el.dataset.nln) return;
      const rec = this.recs.get(el);
      // A block that is not drawn has no place to number, but its lines count (counted on each page when it was last drawn).
      if (rec?.numbered && isFar(el)) {
        rec.numbered.forEach((n, j) => {
          if (n) lines.push({ top: 0, bottom: 0, page: rec.page + j, section, font, count: n, block: i });
        });
        return;
      }
      // In columns each column is numbered on its own, in reading order, beside the column.
      const wrap = el.parentElement?.classList.contains("cols") ? el.parentElement : null;
      let col: ((r: DOMRect) => number) | undefined;
      if (wrap) {
        col = colsOf.get(wrap);
        if (!col) {
          const sc = this.scale();
          const er = wrap.getBoundingClientRect();
          const n = this.secs[section].cols;
          const gap = this.secs[section].gap;
          const pitch = (er.width / sc + gap) / n;
          col = (r) => Math.max(0, Math.min(n - 1, Math.floor((((r.left + r.right) / 2 - er.left) / sc + gap / 2) / pitch)));
          colsOf.set(wrap, col);
        }
      }
      const sec = this.secs[section];
      const per: number[] = [];
      for (const l of this.textLines(el, col)) {
        const x = wrap ? sec.ml + l.col * ((sec.w - sec.ml - sec.mr + sec.gap) / sec.cols) : undefined;
        const page = this.pageAt((l.top + l.bottom) / 2);
        lines.push({ top: l.top, bottom: l.bottom, page, section, font, x, block: i });
        if (rec) {
          const j = Math.max(0, page - rec.page);
          per[j] = (per[j] ?? 0) + 1;
        }
      }
      if (rec) rec.numbered = Array.from(per, (n) => n ?? 0);
    });
    // The numbers each line gets, in order: one counter per section (or page, as the section says). The last one of each block, for the check below.
    const number = (list: Line[], out?: (l: Line, n: number) => void): Map<number, number> => {
      const ends = new Map<number, number>();
      let n = 0;
      let lastSection = -1;
      let lastPage = -1;
      for (const l of list) {
        const ln = secs[l.section].lineNumbers!;
        const start = Math.max(1, ln.start);
        if (l.section !== lastSection) {
          if (lastSection < 0 || ln.restart === "newSection") n = start - 1;
          lastSection = l.section;
        }
        if (ln.restart === "newPage" && l.page !== lastPage) n = start - 1;
        lastPage = l.page;
        if (l.count !== undefined) n += l.count;
        else {
          n++;
          out?.(l, n);
        }
        ends.set(l.block, n);
      }
      return ends;
    };
    // The numbers need nothing of a block's place: where the blocks are all drawn, counting them from the page map must give the same.
    if (this.verify) {
      const counted: Line[] = [];
      for (const l of lines) {
        const prev = counted[counted.length - 1];
        if (l.count === undefined && prev && prev.count !== undefined && prev.block === l.block && prev.page === l.page) prev.count++;
        else counted.push({ ...l, count: l.count ?? 1 });
      }
      const a = number(lines);
      const b = number(counted);
      for (const [blk, n] of a) if (b.get(blk) !== n) throw new Error(`Line numbers counted from the page map differ from the page's at block ${blk}: ${b.get(blk)} against ${n}`);
    }
    const holders = new Map<number, HTMLElement>();
    number(lines, (l, n) => {
      const ln = secs[l.section].lineNumbers!;
      if (n % Math.max(1, ln.countBy) !== 0) return;
      const sheet = this.sheets.children[l.page] as HTMLElement | undefined;
      if (!sheet) return;
      let holder = holders.get(l.page);
      if (!holder) {
        holder = document.createElement("div");
        holder.className = "lnum";
        holder.setAttribute("aria-hidden", "true");
        sheet.append(holder);
        holders.set(l.page, holder);
      }
      const num = document.createElement("span");
      num.textContent = String(n);
      // Right-aligned, `distance` (Word's automatic one is a quarter inch) to the left of the text, which is at
      // the left margin of its section on its page (a continuous section can change it part-way down).
      const box = this.page(l.page);
      const edge = (l.x ?? tw(secs[l.section].margin[3])) - tw(ln.distance ?? 360);
      num.style.cssText = `top:${l.top - box.top}px;right:${box.w - edge}px;height:${l.bottom - l.top}px;line-height:${l.bottom - l.top}px;${l.font}`;
      holder.append(num);
    });
  },
  /** What page k's sheet is made of: a string for what is a number or a flag, and objects compared by identity (the setup's headers and notes). */
  sheetKey(this: Paginator, k: number): { sig: string; refs: unknown[] } {
    const b = this.page(k);
    const sec = this.secs[b.section];
    const ch = this.chapterAt[k];
    let sig = `${b.top}|${b.left}|${b.w}|${b.h}|${b.mT}|${b.mB}|${b.ml}|${b.mr}|${b.section}|${b.first ? 1 : 0}|${b.num}|${this.pages}|${this.sectionPages[b.section] ?? 0}|${ch ? ch.label + "/" + ch.n : ""}|${this.cont ? 1 : 0}`;
    // Notes right under the text are placed from where the text ends.
    if (sec?.info.footnotePr?.pos === "beneathText") sig += `|${this.dirty.has(k) ? 1 : 0}|${this.pageEnd[k]}|${this.reserve[k] ?? 0}`;
    for (const r of this.rules) if (r.page === k) sig += `|r${r.x},${r.y1},${r.y2}`;
    const end = this.endByPage[k];
    const run = this.noteRuns[k];
    sig += `|${end ? end.top : ""}|${run?.from ? 1 : 0}${run?.to ? 1 : 0}`;
    const refs: unknown[] = [this.storyAt(k, "header")?.blocks, this.storyAt(k, "footer")?.blocks, end?.notes];
    const notes = this.notesByPage[k];
    if (notes) refs.push(...notes);
    return { sig, refs };
  },
  renderSheets(this: Paginator, incremental = false) {
    const p = this.setup;
    // Only a pass that started where the text changed leaves every sheet it did not touch as it was; a sheet being edited is made again.
    let reuse = incremental && !this.sheetsUseText && !this.story;
    if (!reuse) this.sheetsUseText = false;
    const state: { sig: string; refs: unknown[] }[] = [];
    const last = this.page(this.pages - 1);
    this.stage.style.height = `${last.top + last.h}px`;
    const have = this.sheets.children.length;
    for (let k = have; k < this.pages; k++) {
      const sheet = document.createElement("div");
      sheet.className = "sheet";
      this.sheets.append(sheet);
    }
    while (this.sheets.children.length > this.pages) this.sheets.lastElementChild!.remove();
    this.sectionPages = [];
    for (let k = 0; k < this.pages; k++) this.sectionPages[this.boxes[k].section] = (this.sectionPages[this.boxes[k].section] ?? 0) + 1;
    const story = this.story;
    // The page background is the document's own (a colour, gradient, pattern, texture or picture), not a token: it prints, too.
    const fill = pageFillCss(p?.pageFill, p?.pageColor, (part) => this.env.engine.mediaUrl(part));
    // (A fill that changed is every sheet's: it is compared once here, not written into every sheet's key.)
    if (fill !== this.sheetFill) reuse = false;
    this.sheetFill = fill;
    Array.from(this.sheets.children).forEach((s, k) => {
      const sheet = s as HTMLElement;
      const box = this.page(k);
      const key = this.sheetKey(k);
      state[k] = key;
      const had = this.sheetState[k];
      if (reuse && had && had.sig === key.sig && had.refs.length === key.refs.length && had.refs.every((r, i) => r === key.refs[i])) return;
      sheet.style.cssText = `top:${box.top}px;left:${box.left}px;width:${box.w}px;height:${box.h}px${fill ? `;${fill}` : ""}`;
      // The element being edited stays attached (detaching it would drop focus and selection).
      const editing = story?.page === k ? story : null;
      for (const c of Array.from(sheet.children)) if (c !== editing?.el) c.remove();
      if (!p) return;
      const si = box.section;
      const sec = this.secs[si];
      const side = `left:${box.ml}px;right:${box.mr}px`;
      // The watermark of the section (else the document's) goes first, under everything else on the sheet.
      const wm = this.cont ? undefined : (sec.info.watermark ?? p.watermark);
      if (wm) {
        const mark = watermarkElement(wm, box.w - box.ml - box.mr, box.h - sec.mt - sec.mb, (part) => this.env.engine.mediaUrl(part));
        mark.style.left = `${box.ml}px`;
        mark.style.top = `${sec.mt}px`;
        sheet.prepend(mark);
      }
      // The section's page borders, on the pages it asks for.
      if (!this.cont && bordersShow(sec.info.pageBorders, box.first)) {
        sheet.prepend(
          pageBorderEl(sec.info.pageBorders!, { w: box.w, h: box.h, mt: sec.mt, mr: box.mr, mb: sec.mb, ml: box.ml, hdr: sec.hdr, ftr: sec.ftr, hasHeader: !!this.storyAt(k, "header"), hasFooter: !!this.storyAt(k, "footer") }),
        );
      }
      const place = (zone: StoryZone, css: string, blocks: RBlock[] | undefined, cls: string): HTMLElement | undefined => {
        if (editing?.zone === zone) {
          editing.el.style.cssText = css;
          if (editing.el.parentElement !== sheet) sheet.append(editing.el);
          return;
        }
        if (!blocks?.length) return;
        const el = document.createElement("div");
        el.className = cls;
        el.style.cssText = css;
        for (const b of renderBlocks(blocks, { ...this.env, readonly: true })) el.append(b);
        this.fillFields(el, k);
        this.fieldHook?.(el);
        sheet.append(el);
        layoutTabs(el, this.defaultTab);
        return el;
      };
      if (!this.cont) {
        // What a header or footer holds is cut off at the page's edges (a picture wider than the page, as Word does).
        place("header", `top:${sec.hdr}px;${side};clip-path:inset(-${sec.hdr}px -${box.mr}px -${box.h}px -${box.ml}px)`, this.storyAt(k, "header")?.blocks, "hf hdr");
        place("footer", `bottom:${sec.ftr}px;${side};clip-path:inset(-${box.h}px -${box.mr}px -${sec.ftr}px -${box.ml}px)`, this.storyAt(k, "footer")?.blocks, "hf ftr");
      }
      const notes = this.notesByPage[k];
      if (notes?.length || editing?.zone === "notes") {
        // Footnotes go at the foot of the page, or right under the text when the section asks for that.
        const under = sec.info.footnotePr?.pos === "beneathText" && !this.dirty.has(k) && this.pageEnd[k] !== undefined;
        const at = under ? Math.min(this.pageEnd[k] - box.top + 6, box.h - box.mB - (this.reserve[k] ?? 0)) : undefined;
        const el = place("notes", `${at !== undefined ? `top:${at}px` : `bottom:${box.mB}px`};${side}`, notes?.flatMap((n) => n.blocks), "notes");
        if (el) this.decorateNotes(el, k);
      }
      const end = this.endByPage[k];
      if (end || editing?.zone === "endnotes") {
        const top = end ? end.top - box.top : box.mT;
        place("endnotes", `top:${top}px;${side}`, end?.notes.flatMap((n) => n.blocks), "notes endnotes");
      }
      for (const r of this.rules) {
        if (r.page !== k) continue;
        const line = document.createElement("div");
        line.className = "crule";
        line.setAttribute("aria-hidden", "true");
        line.style.cssText = `left:${r.x}px;top:${r.y1 - box.top}px;height:${r.y2 - r.y1}px`;
        sheet.append(line);
      }
      if (this.cont) return;
      const num = document.createElement("div");
      num.className = "page-num";
      num.textContent = String(k + 1);
      sheet.append(num);
    });
    this.sheetState = state;
  },
  fillFields(this: Paginator, root: HTMLElement, k: number) {
    this.fillGroups(root, false, () => k);
  },
  bodyParas(this: Paginator): RefPara[] {
    this.sheetsUseText = true;
    if (this.refParas) return this.refParas;
    const top = this.stage.getBoundingClientRect().top;
    const scale = this.scale();
    this.refParas = Array.from(this.flow.querySelectorAll<HTMLElement>(".fp[data-style]")).map((el) => {
      const copy = el.cloneNode(true) as HTMLElement;
      copy.querySelectorAll(".lbl, .pgsp, .ghost").forEach((n) => n.remove());
      return {
        page: this.recs.get(el)?.page ?? this.pageAt((el.getBoundingClientRect().top - top) / scale),
        style: el.dataset.style!,
        text: (copy.textContent ?? "").replace(/\s+/g, " ").trim(),
        number: el.querySelector(".lbl")?.textContent?.trim() ?? "",
      };
    });
    return this.refParas;
  },
  /** The ids a style name or id in a STYLEREF field stands for. */
  styleIds(this: Paginator, name: string): Set<string> {
    const want = styleKey(name);
    const ids = new Set<string>([want]);
    try {
      for (const s of this.env.engine.styles()) if (styleKey(s.name) === want || styleKey(s.id) === want) ids.add(styleKey(s.id));
    } catch {
      // No engine styles to look at: the name as written stands.
    }
    return ids;
  },
  /** What the layout puts in a field on page k, or null when the result the file has stands. */
  fieldValue(this: Paginator, f: string, instr: string | undefined, k: number): string | null {
    const box = this.page(k);
    // The field's own switches (`\* ROMAN`, `\# "000"`) apply to the number the layout found.
    const switched = (label: string) => (instr && /\\[*#@]/.test(instr) ? this.env.engine.applyFieldSwitches(instr, label) : label);
    switch (f) {
      case "PAGE":
      case "pgNum":
      {
        const info = this.secs[box.section]?.info;
        const chapter = info?.chapStyle ? this.chapterAt[k] : undefined;
        if (!chapter) return switched(pageLabel(box.num, info?.pageFormat));
        const sep = ({ period: ".", colon: ":", emDash: "\u2014", enDash: "\u2013" } as Record<string, string>)[info?.chapSep ?? ""] ?? "-";
        return switched(`${chapter.label}${sep}${pageLabel(chapter.n, info?.pageFormat)}`);
      }
      case "NUMPAGES":
        return switched(String(this.pages));
      case "SECTIONPAGES":
        return switched(String(this.sectionPages[box.section] ?? 1));
      case "SECTION":
        return switched(String(box.section + 1));
      case "STYLEREF": {
        // `\s` is the chapter-number form captions use (`STYLEREF 1 \s`): the engine keeps its result up to date.
        if (!instr || /\\s(?![a-z])/i.test(instr)) return null;
        const arg = /STYLEREF\s+(?:"([^"]*)"|(\S+))/i.exec(instr) ?? [];
        const style = arg[1] ?? arg[2];
        if (!style) return null;
        // A heading level (`STYLEREF 1`) stands for the built-in style of that level.
        const level = /^[1-9]$/.test(style) ? `Heading ${style}` : style;
        const text = styleRefText(instr, k, this.bodyParas(), this.styleIds(level));
        // No paragraph has the style: the result the file has stands.
        return text === "" ? null : text;
      }
      case "dayShort":
      case "dayLong":
      case "monthShort":
      case "monthLong":
      case "yearShort":
      case "yearLong":
        return dateElementText(f, this.setup?.lang || navigator.language);
    }
    return null;
  },
  /**
   * Fill the page-dependent fields under `root`. Each field is a run of pieces: the first carries the instruction
   * (`data-fi`), the rest follow it. In a header or footer the first piece gets the value as text and the rest are
   * emptied. In the text the pieces keep their text and the first shows the value through `data-val`.
   * Returns whether the text changed.
   */
  fillGroups(this: Paginator, root: HTMLElement, overlay: boolean, pageOf: (el: HTMLElement) => number): boolean {
    interface Group {
      first: HTMLElement;
      rest: HTMLElement[];
    }
    const groups: Group[] = [];
    let open: Group | null = null;
    for (const el of Array.from(root.querySelectorAll<HTMLElement>("[data-fld]"))) {
      const f = el.dataset.fld!;
      if (!DYNAMIC.has(f)) {
        open = null;
        continue;
      }
      // One piece per atom (a page number element); a field's result may be several pieces.
      const para = el.closest("[data-id]");
      if (open && !el.classList.contains("val") && open.first.dataset.fld === f && !el.dataset.fi && open.first.closest("[data-id]") === para) open.rest.push(el);
      else groups.push((open = { first: el, rest: [] }));
    }
    let changed = false;
    for (const g of groups) {
      const f = g.first.dataset.fld!;
      const val = this.fieldValue(f, g.first.dataset.fi, pageOf(g.first));
      if (val === null) continue;
      if (!overlay || g.first.classList.contains("val")) {
        const same = g.first.textContent === val;
        g.first.textContent = val;
        for (const r of g.rest) r.textContent = "";
        changed ||= !same;
        continue;
      }
      const stored = [g.first, ...g.rest].map((e) => e.textContent).join("");
      if (stored === val) {
        for (const e of [g.first, ...g.rest]) {
          e.classList.remove("fval");
          e.removeAttribute("data-val");
        }
        continue;
      }
      if (g.first.dataset.val !== val) {
        if (!g.first.classList.contains("fval")) g.first.style.setProperty("--fs", getComputedStyle(g.first).fontSize);
        g.first.dataset.val = val;
        changed = true;
      }
      for (const e of [g.first, ...g.rest]) e.classList.add("fval");
    }
    return changed;
  },
};

declare module "./layout" {
  interface Paginator {
    drawLineNumbers(): void;
    sheetKey(k: number): { sig: string; refs: unknown[]; };
    renderSheets(incremental?: boolean): void;
    fillFields(root: HTMLElement, k: number): void;
    bodyParas(): RefPara[];
    styleIds(name: string): Set<string>;
    fieldValue(f: string, instr: string | undefined, k: number): string | null;
    fillGroups(root: HTMLElement, overlay: boolean, pageOf: (el: HTMLElement) => number): boolean;
  }
}
