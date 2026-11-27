// The lines Word's connector presets draw: elbow (`bentConnector2` to `5`) and curved (`curvedConnector2` to `5`), from
// the formulas in DrawingML's preset shape definitions. The path starts at the top-left of the box and ends at the
// bottom-right; the adjust values move the bends (`adj1` to `adj3`, 100000 is the box's width or height). Coordinates are
// in the box's own units (points).

type Adj = Record<string, number> | undefined;

const n = (v: number) => String(Math.round(v * 100) / 100);
const pt = (x: number, y: number) => `${n(x)},${n(y)}`;

/** SVG path data for a connector preset in a `w` by `h` box, or null for a name that is not an elbow or curved connector. */
export function connectorD(name: string, w: number, h: number, adj: Adj): string | null {
  const a = (key: string) => (adj?.[key] ?? 50000) / 100000;
  const [hc, vc] = [w / 2, h / 2];
  switch (name) {
    case "bentConnector2":
      return `M0,0 L${pt(w, 0)} L${pt(w, h)}`;
    case "bentConnector3": {
      const x1 = w * a("adj1");
      return `M0,0 L${pt(x1, 0)} L${pt(x1, h)} L${pt(w, h)}`;
    }
    case "bentConnector4": {
      const [x1, y2] = [w * a("adj1"), h * a("adj2")];
      return `M0,0 L${pt(x1, 0)} L${pt(x1, y2)} L${pt(w, y2)} L${pt(w, h)}`;
    }
    case "bentConnector5": {
      const [x1, y2, x3] = [w * a("adj1"), h * a("adj2"), w * a("adj3")];
      return `M0,0 L${pt(x1, 0)} L${pt(x1, y2)} L${pt(x3, y2)} L${pt(x3, h)} L${pt(w, h)}`;
    }
    case "curvedConnector2":
      return `M0,0 C${pt(hc, 0)} ${pt(w, vc)} ${pt(w, h)}`;
    case "curvedConnector3": {
      const x2 = w * a("adj1");
      const [x1, x3, y3] = [x2 / 2, (w + x2) / 2, (h * 3) / 4];
      return `M0,0 C${pt(x1, 0)} ${pt(x2, h / 4)} ${pt(x2, vc)} C${pt(x2, y3)} ${pt(x3, h)} ${pt(w, h)}`;
    }
    case "curvedConnector4": {
      const x2 = w * a("adj1");
      const y4 = h * a("adj2");
      const [x1, x3] = [x2 / 2, (w + x2) / 2];
      const [x4, x5] = [(x2 + x3) / 2, (x3 + w) / 2];
      const [y1, y5] = [y4 / 2, (h + y4) / 2];
      const [y2, y3] = [y1 / 2, (y1 + y4) / 2];
      return `M0,0 C${pt(x1, 0)} ${pt(x2, y2)} ${pt(x2, y1)} C${pt(x2, y3)} ${pt(x4, y4)} ${pt(x3, y4)} C${pt(x5, y4)} ${pt(w, y5)} ${pt(w, h)}`;
    }
    case "curvedConnector5": {
      const x3 = w * a("adj1");
      const y4 = h * a("adj2");
      const x6 = w * a("adj3");
      const [x1, x2] = [(x3 + x6) / 2, x3 / 2];
      const [x4, x5, x7] = [(x3 + x1) / 2, (x6 + x1) / 2, (x6 + w) / 2];
      const [y1, y5] = [y4 / 2, (h + y4) / 2];
      const [y2, y3] = [y1 / 2, (y1 + y4) / 2];
      const [y6, y7] = [(y5 + y4) / 2, (y5 + h) / 2];
      return `M0,0 C${pt(x2, 0)} ${pt(x3, y2)} ${pt(x3, y1)} C${pt(x3, y3)} ${pt(x4, y4)} ${pt(x1, y4)} C${pt(x5, y4)} ${pt(x6, y6)} ${pt(x6, y5)} C${pt(x6, y7)} ${pt(x7, h)} ${pt(w, h)}`;
    }
    default:
      return null;
  }
}

/** Whether a preset name is an elbow or curved connector. */
export const isBentOrCurved = (name: string) => /^(bent|curved)Connector[2-5]$/.test(name);
