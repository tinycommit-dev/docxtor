// The small pictures behind a page's texture and pattern fills. Docxtor draws them itself (no image files to ship): a
// texture is noise and weave in a 64 pixel tile, a pattern an 8 by 8 bitmap in two colours. Both go into the document as
// PNG images, the way Word keeps them.
import { msg } from "@docxtor/extensions";

/** A small seeded random generator, so a texture looks the same every time it is drawn. */
function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

type Rgb = [number, number, number];

function canvas(size: number): { c: HTMLCanvasElement; g: CanvasRenderingContext2D } {
  const c = document.createElement("canvas");
  c.width = c.height = size;
  return { c, g: c.getContext("2d")! };
}

/** Fill with a base colour and scatter lighter and darker pixels over it. */
function speckle(size: number, base: Rgb, amount: number, seed: number, draw?: (g: CanvasRenderingContext2D, rand: () => number, size: number) => void) {
  const { c, g } = canvas(size);
  g.fillStyle = `rgb(${base.join(",")})`;
  g.fillRect(0, 0, size, size);
  const rand = rng(seed);
  draw?.(g, rand, size);
  const img = g.getImageData(0, 0, size, size);
  for (let i = 0; i < img.data.length; i += 4) {
    const d = (rand() - 0.5) * amount;
    for (let k = 0; k < 3; k++) img.data[i + k] = Math.max(0, Math.min(255, img.data[i + k] + d));
  }
  g.putImageData(img, 0, 0);
  return c;
}

/** The textures offered (Fill effects, Texture): a name and a way to draw the tile. */
export const TEXTURES: { id: string; name: string; draw: () => HTMLCanvasElement }[] = [
  { id: "paper", name: msg("Paper"), draw: () => speckle(64, [244, 239, 227], 10, 1) },
  { id: "parchment", name: msg("Parchment"), draw: () => speckle(64, [237, 222, 187], 22, 2) },
  { id: "newsprint", name: msg("Newsprint"), draw: () => speckle(64, [226, 225, 222], 26, 3) },
  { id: "granite", name: msg("Granite"), draw: () => speckle(64, [150, 150, 154], 90, 4) },
  { id: "sand", name: msg("Sand"), draw: () => speckle(64, [221, 201, 160], 40, 5) },
  {
    id: "canvas",
    name: msg("Canvas"),
    draw: () =>
      speckle(64, [233, 226, 211], 14, 6, (g, _r, s) => {
        g.strokeStyle = "rgba(120,105,80,0.22)";
        g.lineWidth = 1;
        for (let i = 0; i < s; i += 4) {
          g.beginPath();
          g.moveTo(i + 0.5, 0);
          g.lineTo(i + 0.5, s);
          g.moveTo(0, i + 0.5);
          g.lineTo(s, i + 0.5);
          g.stroke();
        }
      }),
  },
  {
    id: "burlap",
    name: msg("Burlap"),
    draw: () =>
      speckle(64, [190, 156, 102], 36, 7, (g, _r, s) => {
        g.strokeStyle = "rgba(90,62,28,0.35)";
        g.lineWidth = 2;
        for (let i = 0; i < s; i += 8) {
          g.beginPath();
          g.moveTo(i + 1, 0);
          g.lineTo(i + 1, s);
          g.moveTo(0, i + 5);
          g.lineTo(s, i + 5);
          g.stroke();
        }
      }),
  },
  {
    id: "denim",
    name: msg("Denim"),
    draw: () =>
      speckle(64, [92, 120, 160], 24, 8, (g, _r, s) => {
        g.strokeStyle = "rgba(30,50,90,0.3)";
        g.lineWidth = 2;
        for (let i = -s; i < s * 2; i += 6) {
          g.beginPath();
          g.moveTo(i, 0);
          g.lineTo(i + s, s);
          g.stroke();
        }
      }),
  },
  {
    id: "wood",
    name: msg("Wood"),
    draw: () =>
      speckle(64, [176, 128, 82], 20, 9, (g, rand, s) => {
        for (let y = 0; y < s; y += 3) {
          g.strokeStyle = `rgba(90,54,24,${0.12 + rand() * 0.2})`;
          g.lineWidth = 1 + rand() * 1.5;
          g.beginPath();
          g.moveTo(0, y);
          g.bezierCurveTo(s / 3, y + rand() * 4 - 2, (s * 2) / 3, y + rand() * 4 - 2, s, y);
          g.stroke();
        }
      }),
  },
];

/** An 8 by 8 pattern: the rows, `#` for the foreground and `.` for the background. */
export interface PatternDef {
  id: string;
  name: string;
  rows: string[];
}

/** Bayer's ordered dither matrix, which spreads a percentage of dots evenly over 8 by 8 pixels. */
const BAYER = [
  [0, 32, 8, 40, 2, 34, 10, 42],
  [48, 16, 56, 24, 50, 18, 58, 26],
  [12, 44, 4, 36, 14, 46, 6, 38],
  [60, 28, 52, 20, 62, 30, 54, 22],
  [3, 35, 11, 43, 1, 33, 9, 41],
  [51, 19, 59, 27, 49, 17, 57, 25],
  [15, 47, 7, 39, 13, 45, 5, 37],
  [63, 31, 55, 23, 61, 29, 53, 21],
];

const percent = (p: number): string[] => BAYER.map((row) => row.map((v) => (v < (p / 100) * 64 ? "#" : ".")).join(""));

/** The patterns offered (Fill effects, Pattern), named as Word names them. */
export const PATTERNS: PatternDef[] = [
  { id: "5", name: msg("5%"), rows: percent(5) },
  { id: "10", name: msg("10%"), rows: percent(10) },
  { id: "25", name: msg("25%"), rows: percent(25) },
  { id: "50", name: msg("50%"), rows: percent(50) },
  { id: "75", name: msg("75%"), rows: percent(75) },
  { id: "lightDown", name: msg("Light downward diagonal"), rows: ["#...#...", ".#...#..", "..#...#.", "...#...#", "#...#...", ".#...#..", "..#...#.", "...#...#"] },
  { id: "darkUp", name: msg("Dark upward diagonal"), rows: ["###..###", "##..###.", "#..###..", "..###..#", ".###..##", "###..###", "##..###.", "#..###.."] },
  { id: "lightHorizontal", name: msg("Light horizontal"), rows: ["########", "........", "........", "........", "########", "........", "........", "........"] },
  { id: "darkHorizontal", name: msg("Dark horizontal"), rows: ["########", "########", "........", "........", "########", "########", "........", "........"] },
  { id: "lightVertical", name: msg("Light vertical"), rows: ["#...#...", "#...#...", "#...#...", "#...#...", "#...#...", "#...#...", "#...#...", "#...#..."] },
  { id: "darkVertical", name: msg("Dark vertical"), rows: ["##..##..", "##..##..", "##..##..", "##..##..", "##..##..", "##..##..", "##..##..", "##..##.."] },
  { id: "smallGrid", name: msg("Small grid"), rows: ["########", "#.......", "#.......", "#.......", "########", "#.......", "#.......", "#......."] },
  { id: "largeGrid", name: msg("Large grid"), rows: ["########", "#......#", "#......#", "#......#", "#......#", "#......#", "#......#", "########"] },
  { id: "checker", name: msg("Small checker board"), rows: ["##..##..", "##..##..", "..##..##", "..##..##", "##..##..", "##..##..", "..##..##", "..##..##"] },
  { id: "dottedGrid", name: msg("Dotted grid"), rows: ["#.#.#.#.", "........", "#.#.#.#.", "........", "#.#.#.#.", "........", "#.#.#.#.", "........"] },
  { id: "solidDiamond", name: msg("Solid diamond"), rows: ["...##...", "..####..", ".######.", "########", "########", ".######.", "..####..", "...##..."] },
];

const hexToRgb = (hex: string): Rgb => [parseInt(hex.slice(0, 2), 16), parseInt(hex.slice(2, 4), 16), parseInt(hex.slice(4, 6), 16)];

/** A pattern drawn in two colours (hex without `#`). */
export function patternCanvas(p: PatternDef, fore: string, back: string): HTMLCanvasElement {
  const { c, g } = canvas(8);
  const img = g.createImageData(8, 8);
  const [fr, fg, fb] = hexToRgb(fore);
  const [br, bg, bb] = hexToRgb(back);
  p.rows.forEach((row, y) =>
    [...row].forEach((ch, x) => {
      const i = (y * 8 + x) * 4;
      const [r, gg, b] = ch === "#" ? [fr, fg, fb] : [br, bg, bb];
      img.data[i] = r;
      img.data[i + 1] = gg;
      img.data[i + 2] = b;
      img.data[i + 3] = 255;
    }),
  );
  g.putImageData(img, 0, 0);
  return c;
}

/** The PNG bytes of a canvas. */
export function pngBytes(c: HTMLCanvasElement): Uint8Array {
  const b64 = c.toDataURL("image/png").split(",")[1];
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
