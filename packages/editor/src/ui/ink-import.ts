// Taking a finished drawing back into the drawing tool: the strokes of a shape the freehand tool made, as points on the
// page, with the pen it was drawn with. A shape counts as a drawing when it has only open paths of its own with an
// outline and no fill, no text and no turn, and floats over the text.
import type { RShape } from "@docxtor/core";
import type { Pen, PenKind } from "./ink-tool";
import type { Pt } from "./ink-math";

const PT_PX = 96 / 72;
/** Points taken along each curve segment. */
const STEPS = 12;

export interface ImportedInk {
  strokes: Pt[][];
  pen: Pen;
}

/** The pen kind a line most looks like (a drawing doesn't say which pen made it). */
function kindOf(width: number, alpha: number, cap: string): PenKind {
  if (alpha < 1 && cap === "square") return "highlighter";
  if (alpha < 1) return "pencil";
  return width >= 3.5 ? "marker" : "pen";
}

/**
 * The strokes of a drawing shape, in pixels, with its top-left corner at (`left`, `top`); null when the shape isn't one
 * the drawing tool could have made.
 */
export function inkFromShape(s: RShape, left: number, top: number): ImportedInk | null {
  const paths = s.custom?.paths;
  if (s.kind !== "shape" || s.geometry !== "custom" || !paths?.length || s.wrap === "inline" || s.fill || !s.line || s.text || s.rot || s.flipH || s.flipV) return null;
  if (paths.some((p) => p.fill || !p.stroke || !p.cmds?.length || p.cmds[0].t !== "M")) return null;
  const strokes: Pt[][] = [];
  for (const p of paths) {
    const pw = p.w || 1;
    const ph = p.h || 1;
    const at = (x: number, y: number): Pt => ({ x: left + (x / pw) * s.w * PT_PX, y: top + (y / ph) * s.h * PT_PX });
    let run: Pt[] = [];
    let cur: [number, number] = [0, 0];
    const end = () => {
      if (run.length) strokes.push(run);
      run = [];
    };
    for (const c of p.cmds!) {
      if (c.t === "M") {
        end();
        cur = c.pts[0];
        run.push(at(cur[0], cur[1]));
      } else if (c.t === "L") {
        cur = c.pts[0];
        run.push(at(cur[0], cur[1]));
      } else if (c.t === "C" && c.pts.length === 3) {
        const [a, b, d] = c.pts;
        for (let i = 1; i <= STEPS; i++) {
          const t = i / STEPS;
          const m = 1 - t;
          const x = m * m * m * cur[0] + 3 * m * m * t * a[0] + 3 * m * t * t * b[0] + t * t * t * d[0];
          const y = m * m * m * cur[1] + 3 * m * m * t * a[1] + 3 * m * t * t * b[1] + t * t * t * d[1];
          run.push(at(x, y));
        }
        cur = d;
      }
    }
    end();
  }
  if (!strokes.length) return null;
  const alpha = s.line.alpha ?? 1;
  const cap = s.line.cap === "square" ? "square" : "round";
  return { strokes, pen: { kind: kindOf(s.line.widthPt, alpha, cap), color: s.line.color, width: s.line.widthPt, alpha, cap } };
}
