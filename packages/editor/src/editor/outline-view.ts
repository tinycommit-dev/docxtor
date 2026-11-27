// What the Outline view does with headings: promote and demote, move a section up or down, fold and unfold, show
// some levels. Changes to the document are ops (`set_outline_level`, `move_blocks`), so they undo like any edit and
// the assistant can make the same changes; folding and levels are only a way of looking (see folds.ts).
import type { Op, OutlineEntry } from "@docxtor/core";
import type { Editor } from "./editor";

export class OutlineOps {
  constructor(private ed: Editor) {}

  /** Every block of the text with its level, as the engine has it now. */
  entries(): OutlineEntry[] {
    return this.ed.engine?.outlineBlocks() ?? [];
  }

  /** The indexes (into `entries`) of the blocks the selection touches, first to last. */
  private selected(entries: OutlineEntry[]): number[] {
    const r = this.ed.range();
    if (!r) return [];
    const a = this.ed.topBlockId(r[0].p);
    const b = this.ed.topBlockId(r[1].p);
    const i = entries.findIndex((e) => e.id === a);
    const j = entries.findIndex((e) => e.id === b);
    if (i < 0) return [];
    const from = Math.min(i, j < 0 ? i : j);
    const to = Math.max(i, j < 0 ? i : j);
    return Array.from({ length: to - from + 1 }, (_, k) => from + k);
  }

  /** The index just past a heading's section (it and the blocks under it). */
  private sectionEnd(entries: OutlineEntry[], i: number): number {
    const level = entries[i].level;
    if (!level) return i + 1;
    let k = i + 1;
    while (k < entries.length && !(entries[k].level > 0 && entries[k].level <= level)) k++;
    return k;
  }

  private visible(id: number): boolean {
    const el = this.ed.flow.querySelector<HTMLElement>(`[data-id="${id}"]`);
    return !!el && el.parentElement === this.ed.flow && !el.dataset.fold;
  }

  /** Run an op, keeping the selection where it was (the blocks change style or place, not the text). */
  private run(op: Op) {
    this.ed.exec(op, { select: this.ed.sel ?? undefined });
  }

  // -- levels ----------------------------------------------------------------------

  /** Promote (-1) or demote (+1) the selected paragraphs; a folded heading takes the headings folded under it along. */
  shift(delta: -1 | 1): boolean {
    const entries = this.entries();
    const picked = this.selected(entries).filter((i) => entries[i].kind === "p");
    if (!picked.length) return false;
    const ids: number[] = [];
    for (const i of picked) {
      const e = entries[i];
      // Level 1 can't go up: the heading stays, and so does what is folded under it.
      if (delta < 0 && e.level === 1) continue;
      if (!ids.includes(e.id)) ids.push(e.id);
      if (e.level > 0 && this.ed.folds.isFolded(e.id)) {
        for (let k = i + 1; k < this.sectionEnd(entries, i); k++) if (entries[k].level > 0 && !ids.includes(entries[k].id)) ids.push(entries[k].id);
      }
    }
    if (!ids.length) return false;
    this.run({ op: "set_outline_level", ids, delta });
    return true;
  }

  promote(): boolean {
    return this.shift(-1);
  }

  demote(): boolean {
    return this.shift(1);
  }

  /** Make the selected headings body text. */
  toBody(): boolean {
    const entries = this.entries();
    const ids = this.selected(entries).filter((i) => entries[i].kind === "p" && entries[i].level > 0).map((i) => entries[i].id);
    if (!ids.length) return false;
    this.run({ op: "set_outline_level", ids, level: 0 });
    return true;
  }

  /** Make the selected paragraphs level 1 headings. */
  toHeading1(): boolean {
    const entries = this.entries();
    const ids = this.selected(entries).filter((i) => entries[i].kind === "p").map((i) => entries[i].id);
    if (!ids.length) return false;
    this.run({ op: "set_outline_level", ids, level: 1 });
    return true;
  }

  // -- moving ------------------------------------------------------------------------

  /**
   * Where a move of the block run `[from, to)` goes, as the block to put it after or before, or null when it can't move.
   * A heading moves with everything under it: past a sibling's whole section, out of its parent upwards, or into the next section
   * below that section's own text. Body text moves past one block.
   */
  private target(entries: OutlineEntry[], from: number, to: number, dir: -1 | 1): { before?: number; after?: number } | null {
    const level = entries[from].level;
    if (dir < 0) {
      if (from === 0) return null;
      if (!level) {
        let j = from - 1;
        while (j > 0 && !this.visible(entries[j].id)) j--;
        return { before: entries[j].id };
      }
      // Past the previous heading of its level or higher: a sibling's whole section, or out of its parent.
      let s = from - 1;
      while (s >= 0 && !(entries[s].level > 0 && entries[s].level <= level)) s--;
      if (s >= 0) return { before: entries[s].id };
      return { before: entries[0].id };
    }
    if (to >= entries.length) return null;
    if (!level) {
      let j = to;
      // Over a folded heading, past everything it holds.
      while (j < entries.length - 1 && !this.visible(entries[j].id)) j++;
      const end = this.visible(entries[j].id) && entries[j].level > 0 && this.ed.folds.isFolded(entries[j].id) ? this.sectionEnd(entries, j) - 1 : j;
      return { after: entries[end].id };
    }
    const next = entries[to];
    // Into the next heading's section when it is of a higher level: below its own text, so it keeps that text.
    if (next.level > 0 && next.level < level) {
      let k = to + 1;
      while (k < entries.length && entries[k].level === 0) k++;
      return { after: entries[k - 1].id };
    }
    return { after: entries[this.sectionEnd(entries, to) - 1].id };
  }

  /** Move the selected heading (with everything under it) or the selected body text up (-1) or down (1). */
  move(dir: -1 | 1): boolean {
    const entries = this.entries();
    const picked = this.selected(entries);
    if (!picked.length) return false;
    return this.moveRun(entries, picked[0], Math.max(picked[picked.length - 1] + 1, entries[picked[0]].level ? this.sectionEnd(entries, picked[0]) : 0), dir);
  }

  /** Move a heading (by id) with everything under it up or down, as the selection's move does. */
  moveHeading(id: number, dir: -1 | 1): boolean {
    const entries = this.entries();
    const from = entries.findIndex((e) => e.id === id);
    return from >= 0 && this.moveRun(entries, from, this.sectionEnd(entries, from), dir);
  }

  private moveRun(entries: OutlineEntry[], from: number, to: number, dir: -1 | 1): boolean {
    const target = this.target(entries, from, to, dir);
    if (!target) return false;
    const ids = entries.slice(from, to).map((e) => e.id);
    if (target.before !== undefined && ids.includes(target.before)) return false;
    if (target.after !== undefined && ids.includes(target.after)) return false;
    this.run({ op: "move_blocks", ids, ...target });
    this.ed.revealBlock(ids[0]);
    return true;
  }

  moveUp(): boolean {
    return this.move(-1);
  }

  moveDown(): boolean {
    return this.move(1);
  }

  /** Move a section (a heading and what is under it) to just before or after another block: the Navigation pane's drag. */
  moveSection(id: number, place: { before?: number; after?: number }): boolean {
    const entries = this.entries();
    const i = entries.findIndex((e) => e.id === id);
    if (i < 0) return false;
    const ids = entries.slice(i, this.sectionEnd(entries, i)).map((e) => e.id);
    if ((place.before !== undefined && ids.includes(place.before)) || (place.after !== undefined && ids.includes(place.after))) return false;
    this.run({ op: "move_blocks", ids, ...place });
    return true;
  }

  // -- folding and levels ------------------------------------------------------------

  /** The headings the selection is in or on (body text counts as the heading above it). */
  private owners(entries: OutlineEntry[]): OutlineEntry[] {
    const out: OutlineEntry[] = [];
    for (const i of this.selected(entries)) {
      let k = i;
      while (k >= 0 && !entries[k].level) k--;
      if (k >= 0 && !out.includes(entries[k])) out.push(entries[k]);
    }
    return out;
  }

  /** Fold the selected headings (the heading the caret is under, for body text). */
  collapse(): boolean {
    const ed = this.ed;
    const heads = this.owners(this.entries()).filter((e) => {
      const el = ed.flow.querySelector<HTMLElement>(`[data-id="${e.id}"]`);
      return el && ed.folds.hasContent(el);
    });
    if (!heads.length) return false;
    for (const e of heads) ed.folds.set(e.id, true);
    ed.foldsChanged();
    return true;
  }

  /** Unfold the selected headings. */
  expand(): boolean {
    const heads = this.owners(this.entries()).filter((e) => this.ed.folds.isFolded(e.id));
    if (!heads.length) return false;
    for (const e of heads) this.ed.folds.set(e.id, false);
    this.ed.foldsChanged();
    return true;
  }

  expandAll() {
    this.ed.folds.unfoldAll();
    this.ed.folds.levels = 0;
    this.ed.foldsChanged();
  }

  collapseAll() {
    this.ed.folds.foldAll();
    this.ed.foldsChanged();
  }

  /** Show headings down to a level (1 to 9) and no body text; 0 shows everything. */
  showLevel(n: number) {
    const folds = this.ed.folds;
    folds.unfoldAll();
    folds.levels = Math.max(0, Math.min(9, Math.round(n)));
    this.ed.foldsChanged();
  }

  get level(): number {
    return this.ed.folds.levels;
  }

  get firstLineOnly(): boolean {
    return this.ed.folds.firstLine;
  }

  setFirstLineOnly(on: boolean) {
    this.ed.folds.firstLine = on;
    this.ed.foldsChanged();
  }

  /** Tab and Shift+Tab in the Outline view: demote and promote. Tables keep Tab for their cells. */
  key(e: KeyboardEvent): boolean {
    if (e.key !== "Tab" || e.ctrlKey || e.metaKey || e.altKey || this.ed.readOnly || this.ed.story) return false;
    if (this.ed.format()?.inTable) return false;
    e.preventDefault();
    this.shift(e.shiftKey ? -1 : 1);
    return true;
  }
}
