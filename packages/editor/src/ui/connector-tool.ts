// Connectors: lines that stay attached to shapes. The tool (Insert, Drawing, Shape, Lines) draws one by dragging from one
// shape's connection site to another's; dots show the sites of the shape under the pointer and the end snaps to the
// nearest. A selected connector shows a handle at each end instead of the usual frame: drag one to another site to glue
// it there, or off the shapes to let go. The engine keeps glued ends on their sites when a shape moves (see
// `crates/docxtor-core/src/shapes/connector.rs`), so moving a shape, nudging it, undoing and the assistant all keep it.
import { t } from "@docxtor/extensions";
import type { ConnectorRoute, MPos, Op, RShape } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { connectorD } from "../editor/connector-paths";
import { h, overlayLayer } from "./dom";
import { atomAt, boxOf, isConnector, offsetsIn, paragraphAt, posOf, PT_PX, sitesOnScreen, toScreen, type Box, type Pt, type ScreenSite } from "./geo-common";
import { claimGeometry, releaseGeometry } from "./freeform-tool";
import { track } from "./object-frame";
import type { ShapeTool } from "./shape-tool";

const NS = "http://www.w3.org/2000/svg";
/** How near the pointer has to be to a site to snap to it, in pixels. */
const SNAP_PX = 14;
const LINE_COLOR = "4472C4";

export type ConnectorKind = "straight" | "elbow" | "curved";

/** A site the pointer is at: the shape and which of its sites. */
interface Snap {
  pos: MPos;
  site: ScreenSite;
}

export class ConnectorTool {
  private layer: HTMLElement | null = null;
  private strip: HTMLElement | null = null;
  private kind: ConnectorKind = "straight";
  private arrow = false;
  /** The drawing under way: where the press began, and the site there. */
  private press: { at: Pt; snap: Snap | null; para: HTMLElement | null } | null = null;
  private overlay: SVGSVGElement | null = null;
  private handles: HTMLElement[] = [];
  private offSelection: (() => void) | null = null;
  private dragging = false;

  constructor(
    private ed: Editor,
    private shapes: ShapeTool,
  ) {
    this.offSelection = shapes.onSelection(() => this.syncHandles());
    ed.canvas.addEventListener("scroll", () => this.syncHandles(), { passive: true });
    window.addEventListener("resize", () => this.syncHandles());
  }

  /** A connector is being drawn. */
  get active(): boolean {
    return !!this.layer;
  }

  // -- the overlay: dots on the sites, and the connector being drawn ----------------------------

  private ensureOverlay(): SVGSVGElement {
    if (!this.overlay || !this.overlay.isConnected) {
      const svg = document.createElementNS(NS, "svg");
      svg.setAttribute("class", "geo-overlay");
      svg.setAttribute("aria-hidden", "true");
      overlayLayer().append(svg);
      this.overlay = svg;
    }
    return this.overlay;
  }

  private clearOverlay() {
    this.overlay?.remove();
    this.overlay = null;
  }

  private el<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>): SVGElementTagNameMap[K] {
    const e = document.createElementNS(NS, tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
    return e;
  }

  /** The shapes a connector can be glued to: floating ones with a place of their own, not lines or groups. */
  private candidates(skip?: HTMLElement | null): { atom: HTMLElement; pos: MPos; shape: RShape }[] {
    const out: { atom: HTMLElement; pos: MPos; shape: RShape }[] = [];
    for (const atom of Array.from(this.ed.flow.querySelectorAll<HTMLElement>(".atom.shape"))) {
      if (atom === skip || atom.classList.contains("line") || atom.dataset.kind !== "shape" || atom.dataset.wrap === "inline") continue;
      const pos = posOf(atom);
      if (!pos) continue;
      let shape: RShape | null = null;
      try {
        shape = this.ed.engine.shapeAt(pos);
      } catch {
        shape = null;
      }
      if (!shape || shape.kind !== "shape" || isConnector(shape) || shape.x === undefined || shape.y === undefined) continue;
      out.push({ atom, pos, shape });
    }
    return out;
  }

  /**
   * Show the sites of the shape the pointer is over (or near) and return the site it is at, if it is near one. The shape
   * under the pointer, or a shape the pointer is within reach of, shows its dots.
   */
  private hint(x: number, y: number, skip?: HTMLElement | null): Snap | null {
    const svg = this.ensureOverlay();
    const out: SVGElement[] = [];
    let best: { snap: Snap; d: number } | null = null;
    for (const c of this.candidates(skip)) {
      const r = c.atom.getBoundingClientRect();
      const m = SNAP_PX;
      if (x < r.left - m || x > r.right + m || y < r.top - m || y > r.bottom + m) continue;
      const sites = sitesOnScreen(this.ed, c.atom, c.pos, c.shape);
      for (const site of sites) {
        const d = Math.hypot(site.x - x, site.y - y);
        if (d <= SNAP_PX && (!best || d < best.d)) best = { snap: { pos: c.pos, site }, d };
      }
      for (const site of sites) out.push(this.el("circle", { class: "geo-site", cx: site.x, cy: site.y, r: 4, "data-idx": site.idx }));
    }
    if (best) out.push(this.el("circle", { class: "geo-site hot", cx: best.snap.site.x, cy: best.snap.site.y, r: 6 }));
    svg.replaceChildren(...out);
    return best?.snap ?? null;
  }

  /** The connector between two points on screen, drawn with the preset the engine would pick. */
  private route(kind: ConnectorKind, s: Pt, ds: Pt | null, e: Pt, de: Pt | null): { route: ConnectorRoute; d: string; transform: string } | null {
    const z = this.ed.zoom || 1;
    const k = PT_PX * z;
    const to = (p: Pt): [number, number] => [(p[0] - s[0]) / k, (p[1] - s[1]) / k];
    let route: ConnectorRoute;
    try {
      route = this.ed.engine.connectorRoute(kind, [0, 0], ds ?? [0, 0], to(e), de ?? [0, 0]);
    } catch {
      return null;
    }
    const [w, hh] = [route.w * k, route.h * k];
    const d = connectorD(route.prst, w, hh, route.adj) ?? `M0,0 L${w},${hh}`;
    const [cx, cy] = [s[0] + (route.x + route.w / 2) * k, s[1] + (route.y + route.h / 2) * k];
    const transform = `translate(${cx} ${cy}) rotate(${route.rot}) scale(${route.flipH ? -1 : 1} ${route.flipV ? -1 : 1}) translate(${-w / 2} ${-hh / 2})`;
    return { route, d, transform };
  }

  private drawRoute(r: { d: string; transform: string } | null) {
    const svg = this.ensureOverlay();
    svg.querySelector(".geo-route")?.remove();
    if (!r) return;
    svg.append(this.el("path", { class: "geo-route", d: r.d, transform: r.transform }));
  }

  // -- drawing a connector -----------------------------------------------------------------------

  start(kind: ConnectorKind, arrow = false) {
    if (this.layer || this.ed.readOnly || this.ed.story) return;
    claimGeometry(this);
    const r = this.ed.range();
    this.shapes.hide();
    if (r) this.ed.select(r[0]);
    this.kind = kind;
    this.arrow = arrow;
    const layer = h("div", { class: "ink-layer geo-layer", "aria-label": t("Connector"), role: "application", "data-geo": "connector" });
    this.ed.stageElement.append(layer);
    this.layer = layer;
    const cancel = h("button", { type: "button", class: "tb", title: t("Cancel") }, t("Cancel"));
    cancel.addEventListener("click", () => this.cancel());
    this.strip = h("div", { class: "obj-strip geo-strip", role: "toolbar", "aria-label": t("Connector") }, h("span", { class: "readout obj-size", "aria-live": "polite" }, t("Drag from one shape to another. The dots show where it can attach.")), cancel);
    overlayLayer().append(this.strip);
    const c = this.ed.canvas.getBoundingClientRect();
    const w = this.strip.offsetWidth || 420;
    this.strip.style.left = `${Math.max(8, c.left + c.width / 2 - w / 2)}px`;
    this.strip.style.top = `${Math.max(8, c.top + 12)}px`;
    layer.addEventListener("pointerdown", (e) => this.down(e));
    layer.addEventListener("pointermove", (e) => this.move(e));
    layer.addEventListener("pointerup", (e) => this.up(e));
    layer.addEventListener("pointercancel", () => this.abortPress());
    layer.addEventListener("pointerleave", () => {
      if (!this.press) this.clearOverlay();
    });
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        this.cancel();
      }
    };
    document.addEventListener("keydown", key, true);
    this.offKey = () => document.removeEventListener("keydown", key, true);
    layer.tabIndex = -1;
    layer.focus({ preventScroll: true });
  }

  private offKey: (() => void) | null = null;

  private down(e: PointerEvent) {
    if (e.button !== 0 || !this.layer || this.press) return;
    e.preventDefault();
    this.layer.setPointerCapture(e.pointerId);
    const snap = this.hint(e.clientX, e.clientY);
    const at: Pt = snap ? [snap.site.x, snap.site.y] : [e.clientX, e.clientY];
    this.press = { at, snap, para: paragraphAt(this.ed, at[0], at[1]) };
  }

  private move(e: PointerEvent) {
    const press = this.press;
    const snap = this.hint(e.clientX, e.clientY, press?.snap ? atomAt(this.ed, press.snap.pos) : null);
    if (!press) return;
    const end: Pt = snap ? [snap.site.x, snap.site.y] : [e.clientX, e.clientY];
    this.drawRoute(this.route(this.kind, press.at, press.snap?.site.dir ?? null, end, snap?.site.dir ?? null));
  }

  private up(e: PointerEvent) {
    const press = this.press;
    if (!press || !this.layer) return;
    this.layer.releasePointerCapture?.(e.pointerId);
    this.press = null;
    const snap = this.hint(e.clientX, e.clientY, press.snap ? atomAt(this.ed, press.snap.pos) : null);
    const end: Pt = snap ? [snap.site.x, snap.site.y] : [e.clientX, e.clientY];
    this.clearOverlay();
    if (Math.hypot(end[0] - press.at[0], end[1] - press.at[1]) < 6 || !press.para) return;
    this.commit(press.at, press.snap, end, snap, press.para);
  }

  private abortPress() {
    this.press = null;
    this.clearOverlay();
  }

  private commit(from: Pt, start: Snap | null, to: Pt, end: Snap | null, para: HTMLElement) {
    const kind = this.kind;
    const arrow = this.arrow;
    this.close();
    const at = { p: Number(para.dataset.p), o: 0 };
    const op: Record<string, unknown> = {
      op: "insert_connector",
      at,
      kind,
      from: offsetsIn(this.ed, para, from[0], from[1]),
      to: offsetsIn(this.ed, para, to[0], to[1]),
      line: arrow ? { color: LINE_COLOR, width_pt: 1.5, tail_arrow: "triangle" } : { color: LINE_COLOR, width_pt: 1.5 },
    };
    // The shapes' positions are counted before the connector goes in at the start of the paragraph.
    if (start) op.start = { shape: start.pos, idx: start.site.idx };
    if (end) op.end = { shape: end.pos, idx: end.site.idx };
    const res = this.ed.exec(op as Op, { select: { anchor: at, focus: { p: at.p, o: 1 } } });
    const caret = res?.[0]?.caret;
    if (caret) this.shapes.selectAt(at.p, caret.o - 1);
  }

  cancel() {
    const was = this.layer;
    this.close();
    if (was) this.ed.focus();
  }

  private close() {
    this.offKey?.();
    this.offKey = null;
    this.layer?.remove();
    this.strip?.remove();
    this.layer = this.strip = null;
    this.press = null;
    this.clearOverlay();
    releaseGeometry(this);
  }

  // -- the handles of a selected connector -------------------------------------------------------

  /** The connector that is selected, with the box it is drawn in; null when the selection is not one with ends to drag. */
  private subject(): { s: RShape; pos: MPos; atom: HTMLElement; box: Box } | null {
    const s = this.shapes.current;
    const pos = this.shapes.targetPos;
    if (!s || !pos || this.shapes.isMember || this.shapes.multiple || this.ed.readOnly || !isConnector(s) || s.x === undefined || s.y === undefined) return null;
    const atom = atomAt(this.ed, pos);
    return atom ? { s, pos, atom, box: boxOf(this.ed, atom, s) } : null;
  }

  private syncHandles() {
    if (this.dragging) return;
    const sub = this.subject();
    for (const hd of this.handles) hd.remove();
    this.handles = [];
    this.shapes.suspendFrame(!!sub, "ends");
    if (!sub || this.layer) return;
    const z = this.ed.zoom || 1;
    const { s, box } = sub;
    const ends: Pt[] = [toScreen(box, 0, 0), toScreen(box, s.w * PT_PX * z, s.h * PT_PX * z)];
    const glued = [!!s.connect?.start, !!s.connect?.end];
    ends.forEach((p, i) => {
      const hd = h("span", { class: glued[i] ? "obj-handle cxn-end glued" : "obj-handle cxn-end", "data-end": i === 0 ? "start" : "end", title: i === 0 ? t("Start of the connector") : t("End of the connector"), style: `position:fixed;left:${p[0]}px;top:${p[1]}px` });
      hd.addEventListener("pointerdown", (e) => this.dragEnd(e, i === 0 ? "start" : "end"));
      overlayLayer().append(hd);
      this.handles.push(hd);
    });
  }

  /** Drag one end of the selected connector. */
  private dragEnd(e: PointerEvent, which: "start" | "end") {
    const sub = this.subject();
    if (!sub || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const { s, pos, atom, box } = sub;
    const z = this.ed.zoom || 1;
    const k = PT_PX * z;
    const ends: Pt[] = [toScreen(box, 0, 0), toScreen(box, s.w * k, s.h * k)];
    const [mine, other] = which === "start" ? [ends[0], ends[1]] : [ends[1], ends[0]];
    const kindOf = (): ConnectorKind => (s.geometry.startsWith("bent") ? "elbow" : s.geometry.startsWith("curved") ? "curved" : "straight");
    // The way the other end faces, from the shape it is glued to (else the connector's own).
    const otherGlued = which === "start" ? s.connect?.end : s.connect?.start;
    let otherDir: Pt | null = null;
    if (otherGlued) otherDir = this.dirOfGlue(otherGlued.id, otherGlued.idx);
    this.dragging = true;
    let snap: Snap | null = null;
    let to: Pt = mine;
    track(
      e,
      (_dx, _dy, ev) => {
        snap = this.hint(ev.clientX, ev.clientY, atom);
        to = snap ? [snap.site.x, snap.site.y] : [ev.clientX, ev.clientY];
        const r = which === "start" ? this.route(kindOf(), to, snap?.site.dir ?? null, other, otherDir) : this.route(kindOf(), other, otherDir, to, snap?.site.dir ?? null);
        this.drawRoute(r);
        const hd = this.handles[which === "start" ? 0 : 1];
        if (hd) {
          hd.style.left = `${to[0]}px`;
          hd.style.top = `${to[1]}px`;
        }
      },
      (moved) => {
        this.dragging = false;
        this.clearOverlay();
        if (!moved) return this.syncHandles();
        // The end's place in the connector's own frame: where it was, plus how far it was dragged.
        const frame = this.endInFrame(s, which);
        const dest: [number, number] = [Math.round((frame[0] + (to[0] - mine[0]) / k) * 100) / 100, Math.round((frame[1] + (to[1] - mine[1]) / k) * 100) / 100];
        const op: Record<string, unknown> = { op: "set_connector", at: pos, [which === "start" ? "from" : "to"]: dest };
        const sn = snap as Snap | null;
        op[which] = sn ? { shape: sn.pos, idx: sn.site.idx } : "free";
        this.ed.exec(op as Op, { select: { anchor: pos, focus: { p: pos.p, o: pos.o + 1 } } });
        this.syncHandles();
      },
    );
  }

  /** The place of a connector's end in points, in the frame its position counts from. */
  private endInFrame(s: RShape, which: "start" | "end"): Pt {
    const b: Box = { cx: (s.x ?? 0) + s.w / 2, cy: (s.y ?? 0) + s.h / 2, w: s.w, h: s.h, rot: s.rot ?? 0, flipH: !!s.flipH, flipV: !!s.flipV };
    return which === "start" ? toScreen(b, 0, 0) : toScreen(b, s.w, s.h);
  }

  /** The way a glued shape's site faces on screen, found by the drawing id of the shape. */
  private dirOfGlue(id: number, idx: number): Pt | null {
    for (const c of this.candidates()) {
      if (c.shape.id !== id) continue;
      return sitesOnScreen(this.ed, c.atom, c.pos, c.shape).find((s) => s.idx === idx)?.dir ?? null;
    }
    return null;
  }

  dispose() {
    this.offSelection?.();
    this.cancel();
    for (const hd of this.handles) hd.remove();
  }
}
