// Small statistics: medians, spreads and slopes.
export const median = (a: number[]): number => {
  if (!a.length) return NaN;
  const s = [...a].sort((x, y) => x - y);
  const m = s.length >> 1;
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
export const percentile = (a: number[], p: number): number => {
  if (!a.length) return NaN;
  const s = [...a].sort((x, y) => x - y);
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))];
};
export const min = (a: number[]) => Math.min(...a);
export const max = (a: number[]) => Math.max(...a);
export const mean = (a: number[]) => a.reduce((x, y) => x + y, 0) / (a.length || 1);
/** Largest minus smallest, as a share of the median. */
export const spreadOf = (a: number[]): number => {
  const m = median(a);
  return m === 0 ? (max(a) - min(a) === 0 ? 0 : Infinity) : (max(a) - min(a)) / Math.abs(m);
};
/** Least-squares slope of y over x (cost per unit of size). */
export function slope(xs: number[], ys: number[]): number {
  const mx = mean(xs);
  const my = mean(ys);
  let n = 0;
  let d = 0;
  xs.forEach((x, i) => {
    n += (x - mx) * (ys[i] - my);
    d += (x - mx) ** 2;
  });
  return d === 0 ? 0 : n / d;
}
export const round = (n: number, d = 2) => (Number.isFinite(n) ? +n.toFixed(d) : n);
