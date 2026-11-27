// Numbers on a chart: Excel number formats for the labels, and tick values that read well.

export interface Ticks {
  min: number;
  max: number;
  step: number;
  values: number[];
  /** The minor ticks between the major ones. */
  minor: number[];
  /** Logarithmic scale with this base. */
  log?: number;
}

/** A round step near `raw`: 1, 2, 5 times a power of ten. */
function niceStep(raw: number): number {
  const exp = Math.floor(Math.log10(raw));
  const base = 10 ** exp;
  const f = raw / base;
  const nice = f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10;
  return nice * base;
}

export interface TickOptions {
  min?: number;
  max?: number;
  step?: number;
  minorStep?: number;
  log?: number;
}

/** Tick values covering [min, max] with about `target` steps. A fixed `step` or fixed ends are kept as given. */
export function niceTicks(min: number, max: number, target = 5, fixed: TickOptions = {}): Ticks {
  if (fixed.log && fixed.log > 1) return logTicks(min, max, fixed);
  let lo = fixed.min ?? min;
  let hi = fixed.max ?? max;
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) {
    lo = 0;
    hi = 1;
  }
  if (hi < lo) [lo, hi] = [hi, lo];
  if (hi === lo) {
    const pad = Math.abs(lo) * 0.1 || 1;
    if (fixed.min === undefined) lo -= pad;
    if (fixed.max === undefined) hi += pad;
    if (hi === lo) hi = lo + 1;
  }
  const step = fixed.step && fixed.step > 0 ? fixed.step : niceStep((hi - lo) / Math.max(1, target));
  if (fixed.min === undefined) lo = Math.floor(lo / step + 1e-9) * step;
  if (fixed.max === undefined) hi = Math.ceil(hi / step - 1e-9) * step;
  const values: number[] = [];
  const n = Math.min(200, Math.round((hi - lo) / step));
  for (let i = 0; i <= n; i++) values.push(Number((lo + i * step).toPrecision(12)));
  // Minor ticks: Excel's automatic minor unit is a fifth of the major one.
  const ms = fixed.minorStep && fixed.minorStep > 0 ? fixed.minorStep : step / 5;
  const minor: number[] = [];
  const total = Math.min(1000, Math.round((hi - lo) / ms));
  for (let i = 0; i <= total; i++) {
    const v = Number((lo + i * ms).toPrecision(12));
    if (!values.some((m) => Math.abs(m - v) < ms / 1000)) minor.push(v);
  }
  return { min: lo, max: hi, step, values, minor };
}

function logTicks(min: number, max: number, fixed: TickOptions): Ticks {
  const base = fixed.log!;
  const pos = (v: number) => (v > 0 && Number.isFinite(v) ? v : 1);
  const lo = fixed.min && fixed.min > 0 ? fixed.min : base ** Math.floor(Math.log(pos(min)) / Math.log(base) + 1e-9);
  let hi = fixed.max && fixed.max > 0 ? fixed.max : base ** Math.ceil(Math.log(pos(max)) / Math.log(base) - 1e-9);
  if (hi <= lo) hi = lo * base;
  const values: number[] = [];
  const minor: number[] = [];
  const k0 = Math.ceil(Math.log(lo) / Math.log(base) - 1e-9);
  const k1 = Math.floor(Math.log(hi) / Math.log(base) + 1e-9);
  for (let k = k0; k <= k1 && values.length < 60; k++) values.push(Number((base ** k).toPrecision(12)));
  for (let k = k0 - 1; k <= k1; k++) {
    for (let m = 2; m < base && m < 10; m++) {
      const v = Number((m * base ** k).toPrecision(12));
      if (v > lo && v < hi) minor.push(v);
    }
  }
  return { min: lo, max: hi, step: base, values, minor, log: base };
}

/** Where a value falls on a scale, from 0 (the minimum) to 1 (the maximum). */
export function frac(t: Ticks, v: number): number {
  if (t.log) {
    const lg = (x: number) => Math.log(Math.max(x, 1e-300)) / Math.log(t.log!);
    return (lg(v) - lg(t.min)) / (lg(t.max) - lg(t.min) || 1);
  }
  return (v - t.min) / (t.max - t.min || 1);
}

/** What a display unit divides by (the axis shows thousands, millions ...). */
export function unitDivisor(unit: string | undefined): number {
  switch (unit) {
    case "hundreds":
      return 1e2;
    case "thousands":
      return 1e3;
    case "tenThousands":
      return 1e4;
    case "hundredThousands":
      return 1e5;
    case "millions":
      return 1e6;
    case "tenMillions":
      return 1e7;
    case "hundredMillions":
      return 1e8;
    case "billions":
      return 1e9;
    case "trillions":
      return 1e12;
    default:
      return 1;
  }
}

/** Where Excel starts an automatic value axis: at zero unless the data sits far from it. */
export function autoRange(min: number, max: number, includeZero: boolean): [number, number] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [0, 1];
  if (includeZero) return [Math.min(0, min), Math.max(0, max)];
  if (min >= 0 && max > 0 && (max - min) / max > 1 / 6) return [0, max];
  if (max <= 0 && min < 0 && (min - max) / min > 1 / 6) return [min, 0];
  return [min, max];
}

function group(int: string): string {
  return int.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/** Plain numbers: no floating-point noise, no more than a dozen digits. */
export function general(v: number): string {
  if (!Number.isFinite(v)) return "";
  const r = Number(v.toPrecision(12));
  if (Math.abs(r) >= 1e15 || (r !== 0 && Math.abs(r) < 1e-6)) return r.toExponential().replace("e+", "E+").replace("e-", "E-");
  return String(r);
}

/**
 * Format a number with an Excel format code: `General`, `0`, `0.00`, `#,##0`, `0%`, `0.0%`, `$#,##0.00`,
 * `"EUR "0.0`, and a second section for negatives. Anything the codes can't say falls back to General.
 */
export function formatNumber(v: number, code?: string): string {
  if (!code || code === "General" || code === "@") return general(v);
  const sections = code.split(";");
  let section = sections[0] ?? code;
  let value = v;
  if (v < 0 && sections.length > 1) {
    section = sections[1]!;
    value = -v;
  }
  // Literal text: "quoted", \escaped, _x padding and [Colour] tags.
  let clean = "";
  for (let i = 0; i < section.length; i++) {
    const c = section[i]!;
    if (c === '"') {
      const end = section.indexOf('"', i + 1);
      const lit = section.slice(i + 1, end < 0 ? undefined : end);
      clean += lit.replace(/[0#?.,%]/g, (m) => `\u0001${m}`);
      i = end < 0 ? section.length : end;
    } else if (c === "\\") {
      clean += `\u0001${section[++i] ?? ""}`;
    } else if (c === "[") {
      const end = section.indexOf("]", i);
      if (end < 0) break;
      const tag = section.slice(i + 1, end);
      const cur = /^\$([^-\]]*)/.exec(tag);
      if (cur) clean += `\u0001${cur[1]}`;
      i = end;
    } else if (c === "_" || c === "*") {
      i++;
    } else clean += c;
  }
  const m = /[0#?][0#?,]*(?:\.[0#?]*)?/.exec(clean.replace(/\u0001./g, "\u0002\u0002"));
  if (!m) return general(v);
  const idx = m.index;
  const pattern = clean.replace(/\u0001./g, "\u0002\u0002").slice(idx, idx + m[0].length);
  // Map back: the pattern came from the same positions in `clean` (escaped chars were width 2 in both).
  const prefix = clean.slice(0, idx).replace(/\u0001/g, "");
  const suffix = clean.slice(idx + m[0].length).replace(/\u0001/g, "");
  const percent = /%/.test(prefix + suffix) && !/\u0001%/.test(clean);
  if (percent) value *= 100;
  const dec = pattern.includes(".") ? pattern.split(".")[1]!.length : 0;
  const thousands = pattern.split(".")[0]!.includes(",");
  const minInt = (pattern.split(".")[0]!.match(/0/g) ?? []).length;
  const fixed = Math.abs(value).toFixed(dec);
  let [int, frac = ""] = fixed.split(".") as [string, string?];
  if (int.length < minInt) int = int.padStart(minInt, "0");
  if (minInt === 0 && int === "0" && dec > 0) int = "";
  if (thousands) int = group(int);
  const sign = v < 0 && sections.length === 1 && Number(fixed) !== 0 ? "-" : "";
  return `${sign}${prefix}${int}${frac ? `.${frac}` : ""}${suffix}`;
}
