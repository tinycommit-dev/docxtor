// Aligning and distributing objects, the way Word does it. Aligning to the margins or the page writes the
// alignment itself (`relativeFrom="margin"` and `<wp:align>`), so the object stays put when the page changes.
// Aligning to each other, and distributing, work out where the objects are on the pages and write new offsets
// counted from what each one's position already counts from. The whole command is one batch, so one undo.
import { t } from "@docxtor/extensions";
import type { MPos, ObjectInfo, Op } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { frameOf, offsetsAt } from "../editor/float-place";
import { alignMoves, axisOf, bounds, distributeMoves, type Edge, type Rect } from "./align-math";

const PT_PX = 96 / 72;

/** What the objects are lined up against. */
export type AlignTo = "objects" | "margin" | "page";
export type AlignAction = Edge | "distribute-h" | "distribute-v";

export const ALIGN_EDGES: Edge[] = ["left", "center", "right", "top", "middle", "bottom"];

export interface AlignPlan {
  ops: Op[];
  /** Objects left out because they are in line with the text. */
  skipped: number;
  /** Objects the plan moves. */
  moved: number;
  /** The reference actually used (a single object has nothing to line up with, so it goes to the page). */
  to: AlignTo;
}

interface Item {
  pos: MPos;
  info: ObjectInfo;
  atom: HTMLElement;
  para: HTMLElement;
  /** The box on screen, in viewport pixels (the box the turned object takes). */
  rect: Rect;
  /** The object's own width and height in points, before turning. */
  w: number;
  h: number;
}

const WORDS: Record<Edge, { rel: "rel_x" | "rel_y"; align: "align" | "align_y"; word: string }> = {
  left: { rel: "rel_x", align: "align", word: "left" },
  center: { rel: "rel_x", align: "align", word: "center" },
  right: { rel: "rel_x", align: "align", word: "right" },
  top: { rel: "rel_y", align: "align_y", word: "top" },
  middle: { rel: "rel_y", align: "align_y", word: "center" },
  bottom: { rel: "rel_y", align: "align_y", word: "bottom" },
};

/** The atom of the object at a position. */
function atomAt(ed: Editor, pos: MPos): HTMLElement | null {
  return ed.paraElement(pos.p)?.querySelector<HTMLElement>(`:scope > .atom[data-o="${pos.o}"]`) ?? null;
}

function itemFor(ed: Editor, pos: MPos, infos: Map<string, ObjectInfo>): Item | null {
  const info = infos.get(`${pos.p}:${pos.o}`);
  const atom = atomAt(ed, pos);
  const para = atom?.closest<HTMLElement>("[data-p]");
  if (!info || !atom || !para || !info.floating) return null;
  const r = atom.getBoundingClientRect();
  return { pos, info, atom, para, rect: { left: r.left, top: r.top, right: r.right, bottom: r.bottom }, w: Number(atom.dataset.w) || r.width / PT_PX / ed.zoom, h: Number(atom.dataset.h) || r.height / PT_PX / ed.zoom };
}

/** The margins (or the page) of the page an object is on, in viewport pixels. */
function pageRect(ed: Editor, item: Item, to: "margin" | "page"): Rect {
  const host = ed.floatHost;
  const f = frameOf(host, item.para, "page", to === "margin" ? "margin" : "page");
  const box = host.pageBox(f.page);
  const h: [number, number] = to === "margin" ? [box.left + box.ml, box.left + box.w - box.mr] : [box.left, box.left + box.w];
  const s = host.stage.getBoundingClientRect();
  const z = host.zoom || 1;
  return { left: s.left + h[0] * z, right: s.left + h[1] * z, top: s.top + f.v[0] * z, bottom: s.top + f.v[1] * z };
}

/** Offsets (points) to write so the object's unturned box sits at `left`, `top` (viewport pixels). */
function offsetsFor(ed: Editor, item: Item, left: number, top: number): { x: number; y: number } {
  const info = item.info;
  const frame = frameOf(ed.floatHost, item.para, info.relX, info.relY);
  const o = offsetsAt(ed.floatHost, frame, { left, top });
  return { x: Math.round(o.x * 10) / 10, y: Math.round(o.y * 10) / 10 };
}

/**
 * Work out the ops for an alignment or a distribution of the objects at `positions`. Objects in line with the text
 * have no place to move to and are left out (`skipped`).
 */
export function planAlign(ed: Editor, positions: MPos[], action: AlignAction, to: AlignTo): AlignPlan {
  const infos = new Map(ed.engine.objects().map((o) => [`${o.p}:${o.o}`, o]));
  // A hidden object isn't on the page: it is left where it is, without a word.
  const shown = positions.filter((p) => !infos.get(`${p.p}:${p.o}`)?.hidden);
  const items = shown.map((p) => itemFor(ed, p, infos)).filter((i): i is Item => !!i);
  const skipped = shown.length - items.length;
  // One object has nothing to line up with: it goes to the page.
  const ref: AlignTo = to === "objects" && items.length < 2 ? "page" : to;
  const plan: AlignPlan = { ops: [], skipped, moved: 0, to: ref };
  if (!items.length) return plan;
  const z = ed.zoom || 1;
  const px = (pt: number) => pt * PT_PX * z;
  const dist = action === "distribute-h" || action === "distribute-v";

  // Aligning to the margins or the page writes the alignment, as Word does.
  if (!dist && ref !== "objects") {
    const w = WORDS[action as Edge];
    for (const it of items) {
      plan.ops.push({ op: "set_object_position", at: it.pos, [w.rel]: ref, [w.align]: w.word } as Op);
    }
    plan.moved = items.length;
    return plan;
  }

  const rects = items.map((i) => i.rect);
  let moves: number[];
  let axis: "h" | "v";
  if (dist) {
    axis = action === "distribute-h" ? "h" : "v";
    moves = distributeMoves(rects, axis, ref === "objects" ? undefined : pageRect(ed, items[0], ref));
  } else {
    axis = axisOf(action as Edge);
    moves = alignMoves(rects, action as Edge, bounds(rects));
  }
  const write = dist && ref !== "objects" ? { rel: ref } : null;
  items.forEach((it, i) => {
    const d = moves[i];
    if (Math.abs(d) < 0.05) return;
    // The unturned box: the middle of what is on screen, less half the object's own size.
    const cx = (it.rect.left + it.rect.right) / 2;
    const cy = (it.rect.top + it.rect.bottom) / 2;
    const left = cx - px(it.w) / 2 + (axis === "h" ? d : 0);
    const top = cy - px(it.h) / 2 + (axis === "v" ? d : 0);
    const info = it.info;
    // Counted from the margin or the page when the objects were spread across it.
    let o: { x: number; y: number };
    if (write) {
      const frame = frameOf(ed.floatHost, it.para, "page", write.rel);
      const box = ed.floatHost.pageBox(frame.page);
      const h0 = write.rel === "margin" ? box.left + box.ml : box.left;
      const s = ed.floatHost.stage.getBoundingClientRect();
      o = { x: Math.round(((left - s.left) / z - h0) / PT_PX * 10) / 10, y: Math.round(((top - s.top) / z - frame.v[0]) / PT_PX * 10) / 10 };
    } else o = offsetsFor(ed, it, left, top);
    const op: Record<string, unknown> = { op: "set_object_position", at: it.pos };
    if (axis === "h") {
      op.x = o.x;
      if (write) op.rel_x = write.rel;
      else if (!info.relX) op.rel_x = "column";
    } else {
      op.y = o.y;
      if (write) op.rel_y = write.rel;
      else if (!info.relY) op.rel_y = "paragraph";
    }
    plan.ops.push(op as Op);
  });
  plan.moved = plan.ops.length;
  return plan;
}

/** The message for a command that had nothing to move. */
export function skippedMessage(skipped: number): string {
  return skipped === 1 ? t("An object in line with the text can't be aligned. Give it a wrapping other than in line first.") : t("Objects in line with the text can't be aligned. Give them a wrapping other than in line first.");
}
