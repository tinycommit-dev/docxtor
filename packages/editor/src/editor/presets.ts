// Word's preset shapes as SVG path data. The outlines follow the formulas of DrawingML's preset shape
// definitions (presetShapeDefinitions.xml) for the shapes people reach for; anything else draws as a
// rectangle. Everything is in points in a `w` by `h` box, with the preset's adjust values (`adj`,
// `adj1`, ... in the file's units, 100000 = 100%) read the way Word reads them.

import { connectorD } from "./connector-paths";

export interface PathSpec {
  d: string;
  /** Paint the inside. */
  fill: boolean;
  /** Draw the outline. */
  stroke: boolean;
}

type Adj = Record<string, number> | undefined;

const n = (v: number) => String(Math.round(v * 100) / 100);
const pin = (lo: number, v: number, hi: number) => Math.max(lo, Math.min(hi, v));
const pt = (x: number, y: number) => `${n(x)},${n(y)}`;
/** Polygon through the points. */
const poly = (...p: [number, number][]) => `M${p.map(([x, y]) => pt(x, y)).join(" L")} Z`;

const LINES = /^(line|straightConnector1|bentConnector\d|curvedConnector\d)$/;
/** Open shapes with no inside: lines and connectors. */
export const isLine = (geometry: string) => LINES.test(geometry);

/** The unrounded star and polygon vertices, scaled so they fill the box the way Word's hf/vf factors do. */
function regular(count: number, w: number, h: number, inner = 0): string {
  const pts: [number, number][] = [];
  const verts = inner ? count * 2 : count;
  for (let i = 0; i < verts; i++) {
    const r = inner && i % 2 ? inner : 1;
    const a = ((-90 + (360 / verts) * i) * Math.PI) / 180;
    pts.push([r * Math.cos(a), r * Math.sin(a)]);
  }
  const xs = pts.map((p) => p[0]);
  const ys = pts.map((p) => p[1]);
  const x0 = Math.min(...xs);
  const y0 = Math.min(...ys);
  const sx = w / (Math.max(...xs) - x0 || 1);
  const sy = h / (Math.max(...ys) - y0 || 1);
  return poly(...pts.map(([x, y]) => [(x - x0) * sx, (y - y0) * sy] as [number, number]));
}

const STAR_ADJ: Record<number, number> = { 4: 12500, 5: 19098, 6: 28868, 7: 34601, 8: 37500, 10: 42533, 12: 37500, 16: 37500, 24: 37500, 32: 37500 };

/** The outline of a callout's rectangle with a tail towards (xPos, yPos); `radius` rounds the corners. */
function wedgeRect(w: number, h: number, adj1: number, adj2: number, radius = 0): string {
  const dxPos = (w * adj1) / 100000;
  const dyPos = (h * adj2) / 100000;
  const xPos = w / 2 + dxPos;
  const yPos = h / 2 + dyPos;
  const dq = (dxPos * h) / w;
  const dz = Math.abs(dyPos) - Math.abs(dq);
  const x1 = (w * (dxPos > 0 ? 7 : 2)) / 12;
  const x2 = (w * (dxPos > 0 ? 10 : 5)) / 12;
  const y1 = (h * (dyPos > 0 ? 7 : 2)) / 12;
  const y2 = (h * (dyPos > 0 ? 10 : 5)) / 12;
  const xl = dz > 0 ? 0 : dxPos > 0 ? 0 : xPos;
  const xt = dz > 0 ? (dyPos > 0 ? x1 : xPos) : x1;
  const xr = dz > 0 ? w : dxPos > 0 ? xPos : w;
  const xb = dz > 0 ? (dyPos > 0 ? xPos : x1) : x1;
  const yl = dz > 0 ? y1 : dxPos > 0 ? y1 : yPos;
  const yt = dz > 0 ? (dyPos > 0 ? 0 : yPos) : 0;
  const yr = dz > 0 ? y1 : dxPos > 0 ? yPos : y1;
  const yb = dz > 0 ? (dyPos > 0 ? yPos : h) : h;
  if (!radius) {
    return poly([0, 0], [x1, 0], [xt, yt], [x2, 0], [w, 0], [w, y1], [xr, yr], [w, y2], [w, h], [x2, h], [xb, yb], [x1, h], [0, h], [0, y2], [xl, yl], [0, y1]);
  }
  const r = radius;
  const a = (x: number, y: number) => `A${n(r)},${n(r)} 0 0 1 ${pt(x, y)}`;
  return [
    `M0,${n(r)}`,
    a(r, 0),
    `L${pt(x1, 0)} L${pt(xt, yt)} L${pt(x2, 0)} L${pt(w - r, 0)}`,
    a(w, r),
    `L${pt(w, y1)} L${pt(xr, yr)} L${pt(w, y2)} L${pt(w, h - r)}`,
    a(w - r, h),
    `L${pt(x2, h)} L${pt(xb, yb)} L${pt(x1, h)} L${pt(r, h)}`,
    a(0, h - r),
    `L${pt(0, y2)} L${pt(xl, yl)} L${pt(0, y1)} Z`,
  ].join(" ");
}

/** An oval speech bubble with a tail. */
function wedgeEllipse(w: number, h: number, adj1: number, adj2: number): string {
  const rx = w / 2;
  const ry = h / 2;
  const dx = (w * adj1) / 100000;
  const dy = (h * adj2) / 100000;
  const t = Math.atan2(dy / ry, dx / rx);
  const d = 0.33;
  const on = (a: number) => pt(rx + rx * Math.cos(a), ry + ry * Math.sin(a));
  return `M${pt(rx + dx, ry + dy)} L${on(t - d)} A${n(rx)},${n(ry)} 0 1 0 ${on(t + d)} Z`;
}

const cloud = (w: number, h: number) => {
  // The scalloped outline in a 100 by 100 box, drawn with arcs, then fitted to the shape.
  const s = (x: number, y: number) => pt((x * w) / 100, (y * h) / 100);
  const r = (rx: number, ry: number) => `${n((rx * w) / 100)},${n((ry * h) / 100)}`;
  return [
    `M${s(18, 72)}`,
    `A${r(16, 18)} 0 0 1 ${s(17, 40)}`,
    `A${r(16, 18)} 0 0 1 ${s(40, 22)}`,
    `A${r(17, 17)} 0 0 1 ${s(70, 20)}`,
    `A${r(16, 18)} 0 0 1 ${s(88, 38)}`,
    `A${r(14, 17)} 0 0 1 ${s(84, 70)}`,
    `A${r(16, 16)} 0 0 1 ${s(60, 82)}`,
    `A${r(16, 14)} 0 0 1 ${s(36, 82)}`,
    `A${r(14, 14)} 0 0 1 ${s(18, 72)} Z`,
  ].join(" ");
};

/** The paths of a preset in a `w` by `h` box (a rectangle for a name Docxtor doesn't draw). */
export function presetPaths(name: string, w: number, h: number, adj: Adj): PathSpec[] {
  const a = (key: string, def: number) => adj?.[key] ?? def;
  const ss = Math.min(w, h);
  const hc = w / 2;
  const vc = h / 2;
  const solid = (d: string): PathSpec[] => [{ d, fill: true, stroke: true }];
  const rect = () => solid(poly([0, 0], [w, 0], [w, h], [0, h]));
  if (isLine(name)) return [{ d: connectorD(name, w, h, adj) ?? `M0,0 L${pt(w, h)}`, fill: false, stroke: true }];
  switch (name) {
    case "rect":
    case "flowChartProcess":
    case "textBox":
      return rect();
    case "roundRect":
    case "flowChartAlternateProcess": {
      const r = (ss * pin(0, name === "roundRect" ? a("adj", 16667) : 16667, 50000)) / 100000;
      return solid(`M0,${n(r)} A${n(r)},${n(r)} 0 0 1 ${pt(r, 0)} L${pt(w - r, 0)} A${n(r)},${n(r)} 0 0 1 ${pt(w, r)} L${pt(w, h - r)} A${n(r)},${n(r)} 0 0 1 ${pt(w - r, h)} L${pt(r, h)} A${n(r)},${n(r)} 0 0 1 ${pt(0, h - r)} Z`);
    }
    case "round1Rect": {
      const r = (ss * pin(0, a("adj", 16667), 50000)) / 100000;
      return solid(`M0,0 L${pt(w - r, 0)} A${n(r)},${n(r)} 0 0 1 ${pt(w, r)} L${pt(w, h)} L${pt(0, h)} Z`);
    }
    case "round2SameRect": {
      const r = (ss * pin(0, a("adj1", 16667), 50000)) / 100000;
      return solid(`M0,${n(r)} A${n(r)},${n(r)} 0 0 1 ${pt(r, 0)} L${pt(w - r, 0)} A${n(r)},${n(r)} 0 0 1 ${pt(w, r)} L${pt(w, h)} L${pt(0, h)} Z`);
    }
    case "snip1Rect": {
      const c = (ss * pin(0, a("adj", 16667), 50000)) / 100000;
      return solid(poly([0, 0], [w - c, 0], [w, c], [w, h], [0, h]));
    }
    case "ellipse":
    case "flowChartConnector":
      return solid(`M0,${n(vc)} A${n(hc)},${n(vc)} 0 1 1 ${pt(w, vc)} A${n(hc)},${n(vc)} 0 1 1 ${pt(0, vc)} Z`);
    case "triangle": {
      const x = (w * pin(0, a("adj", 50000), 100000)) / 100000;
      return solid(poly([0, h], [x, 0], [w, h]));
    }
    case "rtTriangle":
      return solid(poly([0, 0], [0, h], [w, h]));
    case "diamond":
    case "flowChartDecision":
      return solid(poly([0, vc], [hc, 0], [w, vc], [hc, h]));
    case "parallelogram": {
      const x = (ss * pin(0, a("adj", 25000), (100000 * w) / ss)) / 100000;
      return solid(poly([0, h], [x, 0], [w, 0], [w - x, h]));
    }
    case "trapezoid": {
      const x = (ss * pin(0, a("adj", 25000), (50000 * w) / ss)) / 100000;
      return solid(poly([0, h], [x, 0], [w - x, 0], [w, h]));
    }
    case "pentagon":
      return solid(regular(5, w, h));
    case "heptagon":
      return solid(regular(7, w, h));
    case "decagon":
      return solid(regular(10, w, h));
    case "dodecagon":
      return solid(regular(12, w, h));
    case "hexagon": {
      const x = (ss * pin(0, a("adj", 25000), (50000 * w) / ss)) / 100000;
      return solid(poly([0, vc], [x, 0], [w - x, 0], [w, vc], [w - x, h], [x, h]));
    }
    case "octagon": {
      const x = (ss * pin(0, a("adj", 29289), 50000)) / 100000;
      return solid(poly([0, x], [x, 0], [w - x, 0], [w, x], [w, h - x], [w - x, h], [x, h], [0, h - x]));
    }
    case "star4":
    case "star5":
    case "star6":
    case "star7":
    case "star8":
    case "star10":
    case "star12":
    case "star16":
    case "star24":
    case "star32": {
      const count = Number(name.slice(4));
      return solid(regular(count, w, h, pin(0, a("adj", STAR_ADJ[count]), 50000) / 50000));
    }
    case "rightArrow": {
      const a1 = pin(0, a("adj1", 50000), 100000);
      const dx = (ss * pin(0, a("adj2", 50000), (100000 * w) / ss)) / 100000;
      const dy = (h * a1) / 200000;
      return solid(poly([0, vc - dy], [w - dx, vc - dy], [w - dx, 0], [w, vc], [w - dx, h], [w - dx, vc + dy], [0, vc + dy]));
    }
    case "leftArrow": {
      const a1 = pin(0, a("adj1", 50000), 100000);
      const dx = (ss * pin(0, a("adj2", 50000), (100000 * w) / ss)) / 100000;
      const dy = (h * a1) / 200000;
      return solid(poly([w, vc - dy], [dx, vc - dy], [dx, 0], [0, vc], [dx, h], [dx, vc + dy], [w, vc + dy]));
    }
    case "upArrow": {
      const a1 = pin(0, a("adj1", 50000), 100000);
      const dy = (ss * pin(0, a("adj2", 50000), (100000 * h) / ss)) / 100000;
      const dx = (w * a1) / 200000;
      return solid(poly([0, dy], [hc, 0], [w, dy], [hc + dx, dy], [hc + dx, h], [hc - dx, h], [hc - dx, dy]));
    }
    case "downArrow": {
      const a1 = pin(0, a("adj1", 50000), 100000);
      const dy = (ss * pin(0, a("adj2", 50000), (100000 * h) / ss)) / 100000;
      const dx = (w * a1) / 200000;
      return solid(poly([hc - dx, 0], [hc + dx, 0], [hc + dx, h - dy], [w, h - dy], [hc, h], [0, h - dy], [hc - dx, h - dy]));
    }
    case "leftRightArrow": {
      const a1 = pin(0, a("adj1", 50000), 100000);
      const dx = (ss * pin(0, a("adj2", 50000), (50000 * w) / ss)) / 100000;
      const dy = (h * a1) / 200000;
      return solid(poly([0, vc], [dx, 0], [dx, vc - dy], [w - dx, vc - dy], [w - dx, 0], [w, vc], [w - dx, h], [w - dx, vc + dy], [dx, vc + dy], [dx, h]));
    }
    case "chevron": {
      const x = (ss * pin(0, a("adj", 50000), (100000 * w) / ss)) / 100000;
      return solid(poly([0, 0], [w - x, 0], [w, vc], [w - x, h], [0, h], [x, vc]));
    }
    case "homePlate": {
      const x = (ss * pin(0, a("adj", 50000), (100000 * w) / ss)) / 100000;
      return solid(poly([0, 0], [w - x, 0], [w, vc], [w - x, h], [0, h]));
    }
    case "plus": {
      const x = (ss * pin(0, a("adj", 25000), 50000)) / 100000;
      return solid(poly([0, x], [x, x], [x, 0], [w - x, 0], [w - x, x], [w, x], [w, h - x], [w - x, h - x], [w - x, h], [x, h], [x, h - x], [0, h - x]));
    }
    case "heart": {
      const dx1 = (w * 49) / 48;
      const dx2 = (w * 10) / 48;
      const y1 = -h / 3;
      return solid(`M${pt(hc, h / 4)} C${pt(hc + dx2, y1)} ${pt(hc + dx1, h / 4)} ${pt(hc, h)} C${pt(hc - dx1, h / 4)} ${pt(hc - dx2, y1)} ${pt(hc, h / 4)} Z`);
    }
    case "cloud":
      return solid(cloud(w, h));
    case "donut": {
      const t = (ss * pin(0, a("adj", 25000), 50000)) / 100000;
      const ring = (rx: number, ry: number, x0: number, y0: number) => `M${pt(x0, y0 + ry)} A${n(rx)},${n(ry)} 0 1 1 ${pt(x0 + 2 * rx, y0 + ry)} A${n(rx)},${n(ry)} 0 1 1 ${pt(x0, y0 + ry)} Z`;
      return [{ d: `${ring(hc, vc, 0, 0)} ${ring(hc - t, vc - t, t, t)}`, fill: true, stroke: true }];
    }
    case "can": {
      const e = (ss * pin(0, a("adj", 25000), (50000 * h) / ss)) / 100000 / 2;
      const rx = hc;
      return [
        { d: `M0,${n(e)} A${n(rx)},${n(e)} 0 0 1 ${pt(w, e)} L${pt(w, h - e)} A${n(rx)},${n(e)} 0 0 1 ${pt(0, h - e)} Z`, fill: true, stroke: true },
        { d: `M0,${n(e)} A${n(rx)},${n(e)} 0 0 0 ${pt(w, e)}`, fill: false, stroke: true },
      ];
    }
    case "cube": {
      const c = (ss * pin(0, a("adj", 25000), 100000)) / 100000;
      return [
        { d: poly([0, c], [c, 0], [w, 0], [w, h - c], [w - c, h], [0, h]), fill: true, stroke: true },
        { d: `M0,${n(c)} L${pt(w - c, c)} L${pt(w, 0)} M${pt(w - c, c)} L${pt(w - c, h)}`, fill: false, stroke: true },
      ];
    }
    case "foldedCorner": {
      const c = (ss * pin(0, a("adj", 16667), 50000)) / 100000;
      return [
        { d: poly([0, 0], [w, 0], [w, h - c], [w - c, h], [0, h]), fill: true, stroke: true },
        { d: poly([w - c, h], [w - c * 0.15, h - c * 0.85], [w, h - c]), fill: true, stroke: true },
      ];
    }
    case "frame": {
      const t = (ss * pin(0, a("adj1", 12500), 50000)) / 100000;
      return solid(`${poly([0, 0], [w, 0], [w, h], [0, h])} ${poly([t, t], [t, h - t], [w - t, h - t], [w - t, t])}`);
    }
    case "wedgeRectCallout":
      return solid(wedgeRect(w, h, a("adj1", -20833), a("adj2", 62500)));
    case "wedgeRoundRectCallout":
      return solid(wedgeRect(w, h, a("adj1", -20833), a("adj2", 62500), (ss * pin(0, a("adj3", 16667), 50000)) / 100000));
    case "wedgeEllipseCallout":
      return solid(wedgeEllipse(w, h, a("adj1", -20833), a("adj2", 62500)));
    case "flowChartTerminator": {
      const rx = (w * 3475) / 21600;
      return solid(`M${pt(rx, 0)} L${pt(w - rx, 0)} A${n(rx)},${n(vc)} 0 0 1 ${pt(w - rx, h)} L${pt(rx, h)} A${n(rx)},${n(vc)} 0 0 1 ${pt(rx, 0)} Z`);
    }
    case "flowChartInputOutput":
      return solid(poly([(w * 4230) / 21600, 0], [w, 0], [(w * 17370) / 21600, h], [0, h]));
    case "flowChartPredefinedProcess": {
      const x = w / 8;
      return [
        { d: poly([0, 0], [w, 0], [w, h], [0, h]), fill: true, stroke: true },
        { d: `M${pt(x, 0)} L${pt(x, h)} M${pt(w - x, 0)} L${pt(w - x, h)}`, fill: false, stroke: true },
      ];
    }
    case "flowChartDocument":
      return solid(`M0,0 L${pt(w, 0)} L${pt(w, h * 0.83)} C${pt(w * 0.75, h * 0.83)} ${pt(w * 0.75, h * 1.05)} ${pt(hc, h * 0.92)} C${pt(w * 0.25, h * 0.8)} ${pt(w * 0.25, h * 0.98)} ${pt(0, h * 0.9)} Z`);
    default:
      return rect();
  }
}
