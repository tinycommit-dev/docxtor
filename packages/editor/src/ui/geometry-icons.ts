// Icons for curves, freeforms, edit points and connectors (24px grid, 1.75 stroke, like the rest of Docxtor's), and the
// small drawings the shape picker shows for them.
const wrap = (d: string) =>
  `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const GEOMETRY_ICONS = {
  curve: wrap('<path d="M3.5 17c3.5 0 4-10 8.5-10s4.5 10 8.5 10"/><circle cx="3.5" cy="17" r="1.1"/><circle cx="12" cy="7" r="1.1"/><circle cx="20.5" cy="17" r="1.1"/>'),
  freeform: wrap('<path d="M4 18 8 6l5 8c1.5-2.5 3-3 4.5-2.5L20.5 18z"/>'),
  scribble: wrap('<path d="M3.5 15c2-6 3.5-8 4.5-6s-1 7 1 7 2-8 4-8 1 8 3 8 2-4 4.5-4"/>'),
  editPoints: wrap('<path d="M5 17c1-8 5-11 14-10"/><rect x="3" y="15" width="4" height="4" rx=".8"/><rect x="17" y="5" width="4" height="4" rx=".8"/><circle cx="11.5" cy="8" r="1.4"/>'),
  elbow: wrap('<path d="M4 18h8V6h8"/><circle cx="4" cy="18" r="1.1"/><circle cx="20" cy="6" r="1.1"/>'),
  curvedConnector: wrap('<path d="M4 18c6 0 4-12 10-12h6"/><circle cx="4" cy="18" r="1.1"/><circle cx="20" cy="6" r="1.1"/>'),
  connector: wrap('<path d="M4.5 19.5 19.5 4.5"/><circle cx="4.5" cy="19.5" r="1.1"/><circle cx="19.5" cy="4.5" r="1.1"/>'),
  smoothPoint: wrap('<path d="M3 15c4 0 5-6 9-6s5 6 9 6"/><circle cx="12" cy="9" r="1.6"/>'),
  cornerPoint: wrap('<path d="M3 17 12 7l9 10"/><circle cx="12" cy="7" r="1.6"/>'),
  straightPoint: wrap('<path d="M3 12h18"/><circle cx="12" cy="12" r="1.6"/>'),
  deletePoint: wrap('<path d="M3 17c4 0 5-9 9-9s5 9 9 9"/><path d="m9.5 4.5 5 5M14.5 4.5l-5 5"/>'),
  addPoint: wrap('<path d="M3 17c4 0 5-9 9-9s5 9 9 9"/><path d="M12 3.5v4M10 5.5h4"/>'),
};

export type GeometryIconName = keyof typeof GEOMETRY_ICONS;

/** The tools the shape picker starts rather than inserting a shape. */
export type GeometryTool = "curve" | "freeform" | "scribble" | "straight" | "elbow" | "curved";

const NS = "http://www.w3.org/2000/svg";

/** A small drawing of a tool's result for the picker, 30 by 22, like the previews of shapes. */
export function toolPreview(tool: GeometryTool, arrow = false): SVGElement {
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", "-2 -2 34 26");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("class", "shape-preview");
  const path = (d: string, extra: Record<string, string> = {}) => {
    const p = document.createElementNS(NS, "path");
    p.setAttribute("d", d);
    p.setAttribute("fill", "none");
    p.setAttribute("stroke", "currentColor");
    p.setAttribute("stroke-width", "1.5");
    p.setAttribute("stroke-linejoin", "round");
    p.setAttribute("stroke-linecap", "round");
    for (const [k, v] of Object.entries(extra)) p.setAttribute(k, v);
    svg.append(p);
  };
  const dot = (x: number, y: number) => {
    const c = document.createElementNS(NS, "circle");
    c.setAttribute("cx", String(x));
    c.setAttribute("cy", String(y));
    c.setAttribute("r", "1.8");
    c.setAttribute("fill", "currentColor");
    svg.append(c);
  };
  const head = (x: number, y: number, dx: number, dy: number) => {
    // A filled arrow head at (x, y) pointing along (dx, dy).
    const [px, py] = [-dy, dx];
    const h = document.createElementNS(NS, "path");
    h.setAttribute("d", `M${x},${y} L${x - dx * 7 + px * 4},${y - dy * 7 + py * 4} L${x - dx * 7 - px * 4},${y - dy * 7 - py * 4} Z`);
    h.setAttribute("fill", "currentColor");
    svg.append(h);
  };
  switch (tool) {
    case "curve":
      path("M0,19 C6,19 8,3 15,3 C22,3 24,19 30,19");
      dot(0, 19);
      dot(15, 3);
      dot(30, 19);
      break;
    case "freeform":
      path("M1,19 L7,3 L16,13 C19,8 24,8 29,19 Z");
      break;
    case "scribble":
      path("M1,14 C4,2 8,2 9,10 C10,19 14,19 16,9 C17,2 21,3 22,12 C23,18 27,15 30,6");
      break;
    case "straight":
      path(arrow ? "M1,20 L22,6" : "M1,20 L29,2");
      if (arrow) head(29, 2, 0.84, -0.54);
      break;
    case "elbow":
      path(arrow ? "M1,20 H15 V4 H22" : "M1,20 H15 V4 H29");
      if (arrow) head(29, 4, 1, 0);
      break;
    case "curved":
      path(arrow ? "M1,20 C13,20 13,4 22,4" : "M1,20 C13,20 17,4 29,4");
      if (arrow) head(29, 4, 1, 0);
      break;
  }
  return svg;
}
