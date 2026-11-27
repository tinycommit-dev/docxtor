// The vertical ruler: a strip beside the page that holds the caret, showing its top and bottom margins.
// It lives in the stage, so it scrolls and zooms with the pages. Drag a margin boundary to change the margin.
import { t } from "@docxtor/extensions";
import { METRIC, NS, UNIT_TW } from "./ruler";
import { tw } from "./render";

const WIDTH = 16;
/** Space between the strip and the page edge. */
const OFFSET = 8;

export interface VRulerHost {
  zoom(): number;
  /** The sheet of page k in the stage (CSS px): pages differ in size from section to section. */
  page(k: number): { top: number; left: number; w: number; h: number };
  /** New top and bottom margins in twips, for the section that owns the page the ruler stands beside. */
  onMargins(page: number, top: number, bottom: number): void;
}

export class VRuler {
  readonly el: HTMLElement;
  private track: HTMLElement;
  private mTop: HTMLElement;
  private mBottom: HTMLElement;
  private hTop: HTMLElement;
  private hBottom: HTMLElement;
  private page = 0;
  private height = 0;
  private margins: [number, number] = [0, 0];
  private dragging = false;
  private drawn = "";
  private placed = "";
  private shown = true;

  constructor(private host: VRulerHost) {
    this.mTop = this.div("vruler-margin");
    this.mBottom = this.div("vruler-margin");
    this.track = this.div("vruler-track");
    this.track.append(this.mTop, this.mBottom);
    this.hTop = this.handle(true);
    this.hBottom = this.handle(false);
    this.el = this.div("vruler");
    this.el.setAttribute("aria-hidden", "true");
    this.el.hidden = true;
    this.el.append(this.track, this.hTop, this.hBottom);
  }

  private div(cls: string) {
    const d = document.createElement("div");
    d.className = cls;
    return d;
  }

  private handle(top: boolean) {
    const m = this.div("vruler-handle");
    m.title = top ? t("Top margin") : t("Bottom margin");
    m.addEventListener("pointerdown", (e) => this.drag(e, top));
    return m;
  }

  /** Show or hide the strip (it is only worth drawing while the horizontal ruler is). */
  show(on: boolean) {
    this.shown = on;
    this.el.hidden = !on || !this.height;
  }

  /**
   * Stand beside page k, showing the top and bottom margins (twips) of the section that owns it.
   * Pages can differ in size and move when an earlier one changes, so the strip is placed every time.
   */
  update(k: number, top: number, bottom: number) {
    if (this.dragging) return;
    const box = this.host.page(k);
    this.el.hidden = !this.shown || !box.h;
    this.page = k;
    const key = `${box.h}:${top}:${bottom}`;
    if (key !== this.drawn) {
      this.drawn = key;
      this.height = box.h;
      this.margins = [top, bottom];
      this.draw();
    }
    const css = `top:${box.top}px;height:${box.h}px;left:${box.left - (WIDTH + OFFSET)}px;width:${WIDTH}px`;
    if (css !== this.placed) {
      this.placed = css;
      this.el.style.cssText = css;
    }
  }

  private draw() {
    const h = this.height;
    const top = tw(this.margins[0]);
    const bottom = tw(this.margins[1]);
    this.mTop.style.cssText = `top:0;height:${top}px`;
    this.mBottom.style.cssText = `bottom:0;height:${bottom}px`;
    this.hTop.style.top = `${top}px`;
    this.hBottom.style.top = `${h - bottom}px`;
    this.track.querySelector("svg")?.remove();
    const svg = document.createElementNS(NS, "svg");
    svg.setAttribute("viewBox", `0 0 ${WIDTH} ${h}`);
    svg.setAttribute("width", String(WIDTH));
    svg.setAttribute("height", String(h));
    const unitPx = tw(UNIT_TW);
    const sub = METRIC ? 4 : 8;
    const step = unitPx / sub;
    const ticks: string[] = [];
    const labels: SVGTextElement[] = [];
    for (let dir = -1; dir <= 1; dir += 2) {
      for (let i = dir === 1 ? 0 : 1; ; i++) {
        const y = top + dir * i * step;
        if (y < 0 || y > h) break;
        const major = i % sub === 0;
        const half = i % (sub / 2) === 0;
        if (major && i !== 0) {
          const label = document.createElementNS(NS, "text");
          label.setAttribute("x", String(WIDTH / 2));
          label.setAttribute("y", y.toFixed(2));
          label.setAttribute("dy", "0.35em");
          label.setAttribute("text-anchor", "middle");
          label.setAttribute("font-size", "8.5");
          label.setAttribute("font-family", "Inter, sans-serif");
          label.setAttribute("fill", "currentColor");
          label.textContent = String(i / sub);
          labels.push(label);
        } else if (!major) {
          const len = half ? 5 : 3;
          ticks.push(`M${WIDTH - len} ${y.toFixed(2)}H${WIDTH}`);
        }
      }
    }
    const path = document.createElementNS(NS, "path");
    path.setAttribute("d", ticks.join(""));
    path.setAttribute("stroke", "currentColor");
    path.setAttribute("stroke-width", "1");
    svg.append(path, ...labels);
    this.track.append(svg);
  }

  private drag(e: PointerEvent, isTop: boolean) {
    if (!this.height) return;
    e.preventDefault();
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    handle.classList.add("drag");
    this.dragging = true;
    const box = this.host.page(this.page);
    const page = this.page;
    const guide = this.div("vruler-guide");
    guide.style.cssText = `top:${parseFloat(this.el.style.top) + parseFloat(handle.style.top)}px;width:${box.w}px;left:${box.left}px`;
    this.el.parentElement?.append(guide);
    const z = this.host.zoom();
    const startY = e.clientY;
    const startTop = parseFloat(handle.style.top);
    const snap = tw(UNIT_TW) / (METRIC ? 4 : 8);
    // The text area keeps at least this much room.
    const room = 48;
    const topPx = tw(this.margins[0]);
    const bottomPx = tw(this.margins[1]);
    const lo = isTop ? 0 : topPx + room;
    const hi = isTop ? this.height - bottomPx - room : this.height;
    let y = startTop;
    const move = (ev: PointerEvent) => {
      y = Math.round((startTop + (ev.clientY - startY) / z) / snap) * snap;
      y = Math.max(lo, Math.min(hi, y));
      handle.style.top = `${y}px`;
      guide.style.top = `${parseFloat(this.el.style.top) + y}px`;
      (isTop ? this.mTop : this.mBottom).style.height = `${isTop ? y : this.height - y}px`;
    };
    const up = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      handle.removeEventListener("pointercancel", up);
      handle.classList.remove("drag");
      guide.remove();
      this.dragging = false;
      if (Math.abs(y - startTop) < 0.5) return;
      const toTw = (px: number) => Math.max(0, Math.round(px * 15));
      const top = isTop ? toTw(y) : this.margins[0];
      const bottom = isTop ? this.margins[1] : toTw(this.height - y);
      this.margins = [top, bottom];
      // Draw again from what the document says, whether or not the change went through.
      this.drawn = "";
      this.host.onMargins(page, top, bottom);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
    handle.addEventListener("pointercancel", up);
  }
}
