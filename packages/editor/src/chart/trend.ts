// The numbers behind trendlines and error bars: least-squares fits, moving averages, standard deviation and error.
import type { ErrorBars, Trendline } from "@docxtor/core";

export interface Fit {
  /** The fitted curve; absent for a moving average, which is only its points. */
  f?: (x: number) => number;
  /** Points of a moving average. */
  points?: { x: number; y: number }[];
  /** `y = 2.5x + 1.2`. */
  equation: string;
  r2: number;
}

const SUP = ["", "", "²", "³", "⁴", "⁵", "⁶"];

/** A coefficient as Excel shows it on a chart: about five significant digits. */
function coef(v: number): string {
  if (!Number.isFinite(v)) return "0";
  const r = Number(v.toPrecision(5));
  return Math.abs(r) >= 1e6 || (r !== 0 && Math.abs(r) < 1e-4) ? r.toExponential(3).replace("e+", "E+").replace("e-", "E-") : String(r);
}

/** Solve a small linear system (Gaussian elimination with pivoting); null when it has no single solution. */
function solve(a: number[][], b: number[]): number[] | null {
  const n = b.length;
  const m = a.map((row, i) => [...row, b[i]!]);
  for (let c = 0; c < n; c++) {
    let p = c;
    for (let r = c + 1; r < n; r++) if (Math.abs(m[r]![c]!) > Math.abs(m[p]![c]!)) p = r;
    if (Math.abs(m[p]![c]!) < 1e-12) return null;
    [m[c], m[p]] = [m[p]!, m[c]!];
    for (let r = c + 1; r < n; r++) {
      const f = m[r]![c]! / m[c]![c]!;
      for (let k = c; k <= n; k++) m[r]![k]! -= f * m[c]![k]!;
    }
  }
  const x = new Array<number>(n).fill(0);
  for (let r = n - 1; r >= 0; r--) {
    let s = m[r]![n]!;
    for (let k = r + 1; k < n; k++) s -= m[r]![k]! * x[k]!;
    x[r] = s / m[r]![r]!;
  }
  return x;
}

function polyFit(xs: number[], ys: number[], order: number): number[] | null {
  const n = order + 1;
  const a = Array.from({ length: n }, (_, i) => Array.from({ length: n }, (_, j) => xs.reduce((s, x) => s + x ** (i + j), 0)));
  const b = Array.from({ length: n }, (_, i) => xs.reduce((s, x, k) => s + ys[k]! * x ** i, 0));
  return solve(a, b);
}

function r2Of(xs: number[], ys: number[], f: (x: number) => number): number {
  const mean = ys.reduce((a, b) => a + b, 0) / (ys.length || 1);
  const tot = ys.reduce((s, y) => s + (y - mean) ** 2, 0);
  const res = ys.reduce((s, y, i) => s + (y - f(xs[i]!)) ** 2, 0);
  return tot === 0 ? 1 : Math.max(0, 1 - res / tot);
}

/** Fit a trendline to the points; null when the data can't be fitted (too few points, logs of zero or less). */
export function fitTrend(t: Trendline, xs: number[], ys: number[]): Fit | null {
  const n = Math.min(xs.length, ys.length);
  if (n < 2) return null;
  switch (t.type) {
    case "linear": {
      const sx = xs.reduce((a, b) => a + b, 0);
      const sy = ys.reduce((a, b) => a + b, 0);
      let slope: number;
      let icpt: number;
      if (t.intercept !== undefined) {
        icpt = t.intercept;
        const num = xs.reduce((s, x, i) => s + x * (ys[i]! - icpt), 0);
        const den = xs.reduce((s, x) => s + x * x, 0);
        slope = den ? num / den : 0;
      } else {
        const mx = sx / n;
        const my = sy / n;
        const den = xs.reduce((s, x) => s + (x - mx) ** 2, 0);
        if (den === 0) return null;
        slope = xs.reduce((s, x, i) => s + (x - mx) * (ys[i]! - my), 0) / den;
        icpt = my - slope * mx;
      }
      const f = (x: number) => slope * x + icpt;
      return { f, equation: `y = ${coef(slope)}x ${icpt < 0 ? "−" : "+"} ${coef(Math.abs(icpt))}`, r2: r2Of(xs, ys, f) };
    }
    case "poly": {
      const order = Math.min(6, Math.max(2, t.order ?? 2));
      if (n <= order) return null;
      const c = polyFit(xs, ys, order);
      if (!c) return null;
      const f = (x: number) => c.reduce((s, k, i) => s + k * x ** i, 0);
      const terms: string[] = [];
      for (let i = order; i >= 0; i--) {
        const v = c[i]!;
        const body = i === 0 ? coef(Math.abs(v)) : `${coef(Math.abs(v))}x${SUP[i] ?? ""}`;
        terms.push(terms.length === 0 ? `${v < 0 ? "−" : ""}${body}` : `${v < 0 ? "−" : "+"} ${body}`);
      }
      return { f, equation: `y = ${terms.join(" ")}`, r2: r2Of(xs, ys, f) };
    }
    case "exp": {
      if (ys.some((y) => y <= 0)) return null;
      const c = polyFit(xs, ys.map(Math.log), 1);
      if (!c) return null;
      const a = Math.exp(c[0]!);
      const b = c[1]!;
      const f = (x: number) => a * Math.exp(b * x);
      return { f, equation: `y = ${coef(a)}e^${coef(b)}x`, r2: r2Of(xs, ys, f) };
    }
    case "log": {
      if (xs.some((x) => x <= 0)) return null;
      const c = polyFit(xs.map(Math.log), ys, 1);
      if (!c) return null;
      const f = (x: number) => c[1]! * Math.log(x) + c[0]!;
      return { f, equation: `y = ${coef(c[1]!)}ln(x) ${c[0]! < 0 ? "−" : "+"} ${coef(Math.abs(c[0]!))}`, r2: r2Of(xs, ys, f) };
    }
    case "power": {
      if (xs.some((x) => x <= 0) || ys.some((y) => y <= 0)) return null;
      const c = polyFit(xs.map(Math.log), ys.map(Math.log), 1);
      if (!c) return null;
      const a = Math.exp(c[0]!);
      const b = c[1]!;
      const f = (x: number) => a * x ** b;
      return { f, equation: `y = ${coef(a)}x^${coef(b)}`, r2: r2Of(xs, ys, f) };
    }
    case "movingAvg": {
      const p = Math.max(2, t.period ?? 2);
      if (n < p) return null;
      const points: { x: number; y: number }[] = [];
      for (let i = p - 1; i < n; i++) points.push({ x: xs[i]!, y: ys.slice(i - p + 1, i + 1).reduce((a, b) => a + b, 0) / p });
      return { points, equation: "", r2: 0 };
    }
  }
}

export function stdDev(values: number[]): number {
  const n = values.length;
  if (n < 2) return 0;
  const mean = values.reduce((a, b) => a + b, 0) / n;
  return Math.sqrt(values.reduce((s, v) => s + (v - mean) ** 2, 0) / (n - 1));
}

/** How far below and above a value one error bar reaches, in data units. */
export function errorAmounts(eb: ErrorBars, i: number, v: number, all: number[]): { minus: number; plus: number } {
  let both = 0;
  switch (eb.valueType) {
    case "fixed":
      both = eb.value ?? 1;
      break;
    case "percentage":
      both = (Math.abs(v) * (eb.value ?? 5)) / 100;
      break;
    case "stdDev":
      both = stdDev(all) * (eb.value ?? 1);
      break;
    case "stdErr":
      both = stdDev(all) / Math.sqrt(Math.max(1, all.length));
      break;
    case "custom":
      return { plus: eb.barType === "minus" ? 0 : (eb.plus?.[i] ?? 0), minus: eb.barType === "plus" ? 0 : (eb.minus?.[i] ?? 0) };
  }
  return { plus: eb.barType === "minus" ? 0 : both, minus: eb.barType === "plus" ? 0 : both };
}
