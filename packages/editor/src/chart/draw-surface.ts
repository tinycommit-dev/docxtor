// Surface charts: values over a grid of categories (across) and series (into the picture). A 3-D surface and a
// wireframe are drawn as projected polygons sorted back to front; a contour chart is a flat map in colour bands,
// and a wireframe contour is the lines between the bands. The bands are the value axis' steps, coloured like the
// first series of a column chart.
import type { ChartModel } from "@docxtor/core";
import type { Drawer } from "./draw";
import { formatNumber, niceTicks } from "./format";
import { FONT, color, el, finite, textWidth, tip } from "./svg";

export interface Band {
  lo: number;
  hi: number;
  color: string;
  label: string;
}

const grid = (m: ChartModel): (number | null)[][] => m.series.map((s) => m.categories.map((_, i) => (finite(s.values[i]) ? s.values[i]! : null)));

/** The value bands of a surface chart, lowest first. */
export function surfaceBands(m: ChartModel): Band[] {
  const vals = m.series.flatMap((s) => s.values.filter(finite));
  const lo = vals.length ? Math.min(...vals) : 0;
  const hi = vals.length ? Math.max(...vals) : 1;
  const ya = m.yAxis ?? {};
  const tk = niceTicks(Math.min(lo, 0) === 0 && lo >= 0 ? 0 : lo, hi, 5, { min: ya.min, max: ya.max, step: ya.majorUnit });
  const fmt = ya.numberFormat ?? m.axes.dataFormat;
  const bands: Band[] = [];
  for (let i = 0; i + 1 < tk.values.length; i++) {
    const a = tk.values[i]!;
    const b = tk.values[i + 1]!;
    bands.push({ lo: a, hi: b, color: color(m, i), label: `${formatNumber(a, fmt)}-${formatNumber(b, fmt)}` });
  }
  return bands.length ? bands : [{ lo: 0, hi: 1, color: color(m, 0), label: "0-1" }];
}

const bandOf = (bands: Band[], v: number): Band => bands.find((b) => v < b.hi) ?? bands[bands.length - 1]!;

export function drawSurface(d: Drawer) {
  const m = d.m;
  const z = grid(m);
  const rows = z.length;
  const cols = m.categories.length;
  if (rows < 2 || cols < 2) {
    d.add(el("text", { class: "chart-t", x: d.W / 2, y: d.H / 2, "text-anchor": "middle" }, "Not enough data"));
    return;
  }
  const bands = surfaceBands(m);
  const flat = m.surface === "contour" || m.surface === "wireframe_contour";
  const wire = m.surface === "wireframe" || m.surface === "wireframe_contour";
  const box = { x: d.left + 8, y: d.top + 4, w: d.right - d.left - 16, h: d.bottom - d.top - 8 };
  if (flat) contour(d, z, bands, wire, box);
  else surface3d(d, z, bands, wire, box);
}

/** Value at fractional grid position (bilinear). */
function sample(z: (number | null)[][], r: number, c: number): number {
  const r0 = Math.min(z.length - 2, Math.max(0, Math.floor(r)));
  const c0 = Math.min(z[0]!.length - 2, Math.max(0, Math.floor(c)));
  const fr = r - r0;
  const fc = c - c0;
  const v = (rr: number, cc: number) => z[rr]![cc] ?? 0;
  return v(r0, c0) * (1 - fr) * (1 - fc) + v(r0, c0 + 1) * (1 - fr) * fc + v(r0 + 1, c0) * fr * (1 - fc) + v(r0 + 1, c0 + 1) * fr * fc;
}

function contour(d: Drawer, z: (number | null)[][], bands: Band[], wire: boolean, box: { x: number; y: number; w: number; h: number }) {
  const m = d.m;
  const rows = z.length;
  const cols = z[0]!.length;
  const labelW = Math.max(...m.series.map((s) => textWidth(s.name))) + 10;
  const plot = { x: box.x + labelW, y: box.y, w: Math.max(20, box.w - labelW), h: Math.max(20, box.h - FONT - 12) };
  const SUB = 8;
  const nr = (rows - 1) * SUB;
  const nc = (cols - 1) * SUB;
  const cw = plot.w / nc;
  const ch = plot.h / nr;
  const g = el("g", { class: "chart-contour" });
  // The first series is at the bottom, as in Excel.
  const yAt = (rr: number) => plot.y + plot.h - rr * ch;
  const vals: number[][] = [];
  for (let r = 0; r <= nr; r++) {
    vals.push([]);
    for (let c = 0; c <= nc; c++) vals[r]!.push(sample(z, r / SUB, c / SUB));
  }
  if (!wire) {
    for (let r = 0; r < nr; r++) {
      for (let c = 0; c < nc; c++) {
        const avg = (vals[r]![c]! + vals[r]![c + 1]! + vals[r + 1]![c]! + vals[r + 1]![c + 1]!) / 4;
        const b = bandOf(bands, avg);
        g.append(el("rect", { class: "chart-patch", x: plot.x + c * cw, y: yAt(r + 1), width: cw + 0.5, height: ch + 0.5, fill: b.color }));
      }
    }
  } else {
    // Lines where the surface crosses a band edge (marching squares).
    for (const edge of bands.slice(1)) {
      const t = edge.lo;
      const col = bandOf(bands, t).color;
      for (let r = 0; r < nr; r++) {
        for (let c = 0; c < nc; c++) {
          const corners = [vals[r]![c]!, vals[r]![c + 1]!, vals[r + 1]![c + 1]!, vals[r + 1]![c]!];
          const pts: { x: number; y: number }[] = [];
          const at = (a: number, b: number, pa: [number, number], pb: [number, number]) => {
            const f = (t - a) / (b - a);
            return { x: plot.x + (pa[0] + (pb[0] - pa[0]) * f) * cw, y: yAt(pa[1] + (pb[1] - pa[1]) * f) };
          };
          const pos: [number, number][] = [[c, r], [c + 1, r], [c + 1, r + 1], [c, r + 1]];
          for (let k = 0; k < 4; k++) {
            const a = corners[k]!;
            const b = corners[(k + 1) % 4]!;
            if (a < t !== b < t) pts.push(at(a, b, pos[k]!, pos[(k + 1) % 4]!));
          }
          for (let k = 0; k + 1 < pts.length; k += 2) g.append(el("line", { class: "chart-contour-line", x1: pts[k]!.x, y1: pts[k]!.y, x2: pts[k + 1]!.x, y2: pts[k + 1]!.y, stroke: col }));
        }
      }
    }
    g.append(el("rect", { class: "chart-contour-frame", x: plot.x, y: plot.y, width: plot.w, height: plot.h, fill: "none" }));
  }
  d.add(g);
  const labels = el("g", { class: "chart-axes" });
  m.categories.forEach((c, i) => labels.append(el("text", { class: "chart-t", x: plot.x + (i * plot.w) / (cols - 1), y: plot.y + plot.h + FONT + 4, "text-anchor": i === 0 ? "start" : i === cols - 1 ? "end" : "middle" }, c)));
  m.series.forEach((s, i) => labels.append(el("text", { class: "chart-t", x: plot.x - 6, y: yAt((i * nr) / (rows - 1)) + FONT * 0.35, "text-anchor": "end" }, s.name)));
  d.add(labels);
}

function surface3d(d: Drawer, z: (number | null)[][], bands: Band[], wire: boolean, box: { x: number; y: number; w: number; h: number }) {
  const m = d.m;
  const rows = z.length;
  const cols = z[0]!.length;
  const view = m.view3d ?? { rotX: 15, rotY: 20, perspective: 30 };
  const lo = bands[0]!.lo;
  const hi = bands[bands.length - 1]!.hi;
  const rx = (view.rotX * Math.PI) / 180;
  const ry = (view.rotY * Math.PI) / 180;
  const persp = (view.perspective / 100) * 0.35;
  const H = 0.65;
  // A point of the box (x across, y up, z into the picture, each -1 to 1) on the page, with its depth.
  const proj = (x: number, y: number, zz: number) => {
    const x1 = x * Math.cos(ry) + zz * Math.sin(ry);
    const z1 = -x * Math.sin(ry) + zz * Math.cos(ry);
    const y2 = y * Math.cos(rx) - z1 * Math.sin(rx);
    const z2 = y * Math.sin(rx) + z1 * Math.cos(rx);
    const s = 1 / (1 - z2 * persp);
    return { x: x1 * s, y: -y2 * s, depth: z2 };
  };
  const X = (c: number) => (c / (cols - 1)) * 2 - 1;
  const Z = (r: number) => (r / (rows - 1)) * 2 - 1;
  const Y = (v: number) => ((Math.min(hi, Math.max(lo, v)) - lo) / (hi - lo || 1)) * 2 * H - H;
  // The page area these points fill, to scale into the plot.
  const corners = [[-1, -H, -1], [1, -H, -1], [1, -H, 1], [-1, -H, 1], [-1, H, -1], [1, H, -1], [1, H, 1], [-1, H, 1]].map(([x, y, zz]) => proj(x!, y!, zz!));
  const labelW = Math.max(...m.categories.map((c) => textWidth(c)), 0);
  const room = { l: 44, r: Math.max(30, labelW / 2 + 6), t: 6, b: FONT * 2 + 16 };
  const minX = Math.min(...corners.map((p) => p.x));
  const maxX = Math.max(...corners.map((p) => p.x));
  const minY = Math.min(...corners.map((p) => p.y));
  const maxY = Math.max(...corners.map((p) => p.y));
  const aw = box.w - room.l - room.r;
  const ah = box.h - room.t - room.b;
  const sc = Math.max(1, Math.min(aw / (maxX - minX || 1), ah / (maxY - minY || 1)));
  const ox = box.x + room.l + (aw - (maxX - minX) * sc) / 2 - minX * sc;
  const oy = box.y + room.t + (ah - (maxY - minY) * sc) / 2 - minY * sc;
  const P = (x: number, y: number, zz: number) => {
    const p = proj(x, y, zz);
    return { x: ox + p.x * sc, y: oy + p.y * sc, depth: p.depth };
  };

  // The floor, the two walls at the back and the value axis along the left back edge.
  const floor = [P(-1, -H, -1), P(1, -H, -1), P(1, -H, 1), P(-1, -H, 1)];
  const g = el("g", { class: "chart-surface" });
  g.append(el("path", { class: "chart-floor", d: `M${floor.map((p) => `${p.x} ${p.y}`).join("L")}Z` }));
  const backZ = P(0, 0, -1).depth < P(0, 0, 1).depth ? -1 : 1;
  const wallX = P(-1, 0, 0).depth < P(1, 0, 0).depth ? -1 : 1;
  const ticks0 = bands.map((b) => b.lo).concat([bands[bands.length - 1]!.hi]);
  for (const [corners, lines] of [
    [[P(-1, -H, backZ), P(1, -H, backZ), P(1, H, backZ), P(-1, H, backZ)], ticks0.map((v) => [P(-1, Y(v), backZ), P(1, Y(v), backZ)])],
    [[P(wallX, -H, -1), P(wallX, -H, 1), P(wallX, H, 1), P(wallX, H, -1)], ticks0.map((v) => [P(wallX, Y(v), -1), P(wallX, Y(v), 1)])],
  ] as const) {
    g.append(el("path", { class: "chart-floor", d: `M${corners.map((p) => `${p.x} ${p.y}`).join("L")}Z` }));
    for (const [a, b] of lines) g.append(el("line", { class: "chart-grid", x1: a.x, y1: a.y, x2: b.x, y2: b.y }));
  }
  // The vertical edge on the left of the picture.
  const left = [[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([x, zz]) => ({ x: x!, z: zz!, p: P(x!, -H, zz!) })).sort((a, b) => a.p.x - b.p.x)[0]!;
  const ticks = bands.map((b) => b.lo).concat([bands[bands.length - 1]!.hi]);
  const fmt = m.yAxis?.numberFormat ?? m.axes.dataFormat;
  g.append(el("line", { class: "chart-axis", x1: left.p.x, y1: left.p.y, x2: P(left.x, H, left.z).x, y2: P(left.x, H, left.z).y }));
  for (const v of ticks) {
    const p = P(left.x, Y(v), left.z);
    g.append(el("text", { class: "chart-t", x: p.x - 6, y: p.y + FONT * 0.35, "text-anchor": "end" }, formatNumber(v, fmt)));
  }

  // The cells, far ones first.
  interface Cell {
    pts: { x: number; y: number }[];
    depth: number;
    band: Band;
    r: number;
    c: number;
  }
  const cells: Cell[] = [];
  for (let r = 0; r < rows - 1; r++) {
    for (let c = 0; c < cols - 1; c++) {
      const vs = [z[r]![c], z[r]![c + 1], z[r + 1]![c + 1], z[r + 1]![c]].map((v) => v ?? lo);
      const ps = [P(X(c), Y(vs[0]!), Z(r)), P(X(c + 1), Y(vs[1]!), Z(r)), P(X(c + 1), Y(vs[2]!), Z(r + 1)), P(X(c), Y(vs[3]!), Z(r + 1))];
      cells.push({ pts: ps, depth: ps.reduce((a, p) => a + p.depth, 0) / 4, band: bandOf(bands, vs.reduce((a, b) => a + b, 0) / 4), r, c });
    }
  }
  cells.sort((a, b) => a.depth - b.depth);
  for (const cell of cells) {
    const d2 = `M${cell.pts.map((p) => `${p.x} ${p.y}`).join("L")}Z`;
    g.append(el("path", { class: wire ? "chart-wire" : "chart-facet", d: d2, fill: wire ? "none" : cell.band.color, stroke: wire ? cell.band.color : undefined }, tip(`${m.series[cell.r]?.name ?? ""}, ${m.categories[cell.c] ?? ""}: ${cell.band.label}`)));
  }
  // Category labels along the lower front edge, series names along the right one.
  const front = [-1, 1].map((zz) => P(0, -H, zz)).sort((a, b) => b.y - a.y)[0]!;
  const frontZ = P(0, -H, 1).y === front.y ? 1 : -1;
  m.categories.forEach((cat, i) => {
    const p = P(X(i), -H, frontZ);
    g.append(el("text", { class: "chart-t", x: p.x, y: p.y + FONT + 6, "text-anchor": "middle" }, cat));
  });
  const side = [-1, 1].map((x) => P(x, -H, 0)).sort((a, b) => b.x - a.x)[0]!;
  const sideX = P(1, -H, 0).x === side.x ? 1 : -1;
  m.series.forEach((s, i) => {
    const p = P(sideX, -H, Z(i));
    g.append(el("text", { class: "chart-t", x: p.x + 8, y: p.y + FONT * 0.8, "text-anchor": "start" }, s.name));
  });
  d.add(g);
}
