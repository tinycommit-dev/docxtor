// The logic behind the Edit chart dialog and the chart drawing that needs no DOM: trendline fits, error amounts,
// theme colours, logarithmic ticks, and the options a draft asks for.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import type { ChartModel } from "@docxtor/core";
import { resolveColor, themeSwatches } from "../src/chart/colors.ts";
import { addSeries, diffOptions, draftOf, jsonDiff, setStockType, stockRoles, switchKind } from "../src/chart/draft.ts";
import { frac, niceTicks, unitDivisor } from "../src/chart/format.ts";
import { errorAmounts, fitTrend, stdDev } from "../src/chart/trend.ts";

const near = (a: number, b: number, eps = 1e-6) => assert.ok(Math.abs(a - b) <= eps, `${a} is not ${b}`);

const model = (extra: Partial<ChartModel> = {}): ChartModel => ({
  kind: "column",
  grouping: "clustered",
  title: "Sales",
  categories: ["Q1", "Q2", "Q3", "Q4"],
  series: [
    { name: "Sales", values: [24, 31, 29, 42], kind: "column", color: "4472C4" },
    { name: "Costs", values: [15, 18, 22, 24], kind: "column", color: "ED7D31" },
  ],
  legend: { position: "b" },
  axes: { grid: true },
  dataLabels: false,
  xAxis: {},
  yAxis: { majorGrid: true },
  theme: { dk1: "000000", lt1: "FFFFFF", dk2: "44546A", lt2: "E7E6E6", accent1: "4472C4", accent2: "ED7D31" },
  ...extra,
} as ChartModel);

test("a linear fit finds the line, its equation and R squared", () => {
  const f = fitTrend({ type: "linear" }, [1, 2, 3, 4], [3, 5, 7, 9])!;
  near(f.f!(5), 11);
  near(f.r2, 1);
  assert.equal(f.equation, "y = 2x + 1");
  // A fixed intercept.
  const g = fitTrend({ type: "linear", intercept: 0 }, [1, 2, 3], [2, 4, 6])!;
  near(g.f!(10), 20);
});

test("exponential, logarithmic, power and polynomial fits", () => {
  const e = fitTrend({ type: "exp" }, [1, 2, 3, 4], [2, 4, 8, 16])!;
  near(e.f!(5), 32, 1e-6);
  const l = fitTrend({ type: "log" }, [1, 2, 4, 8], [1, 3, 5, 7])!;
  near(l.f!(16), 9, 1e-6);
  const p = fitTrend({ type: "power" }, [1, 2, 3, 4], [3, 12, 27, 48])!;
  near(p.f!(5), 75, 1e-6);
  const q = fitTrend({ type: "poly", order: 2 }, [0, 1, 2, 3], [1, 2, 5, 10])!;
  near(q.f!(4), 17, 1e-6);
  assert.match(q.equation, /^y = 1x² \+ 0x \+ 1$/);
  // What can't be fitted gives nothing.
  assert.equal(fitTrend({ type: "exp" }, [1, 2], [1, -1]), null);
  assert.equal(fitTrend({ type: "log" }, [0, 1], [1, 2]), null);
  assert.equal(fitTrend({ type: "linear" }, [1], [1]), null);
});

test("a moving average starts at its period", () => {
  const m = fitTrend({ type: "movingAvg", period: 3 }, [1, 2, 3, 4, 5], [1, 2, 3, 6, 9])!;
  assert.deepEqual(m.points, [{ x: 3, y: 2 }, { x: 4, y: 11 / 3 }, { x: 5, y: 6 }]);
});

test("error amounts: fixed, percentage, standard deviation, standard error and custom", () => {
  const all = [2, 4, 4, 4, 5, 5, 7, 9];
  near(stdDev(all), Math.sqrt(32 / 7));
  assert.deepEqual(errorAmounts({ dir: "y", barType: "both", valueType: "fixed", value: 2 }, 0, 10, all), { plus: 2, minus: 2 });
  assert.deepEqual(errorAmounts({ dir: "y", barType: "plus", valueType: "percentage", value: 10 }, 0, 50, all), { plus: 5, minus: 0 });
  near(errorAmounts({ dir: "y", barType: "both", valueType: "stdErr" }, 0, 1, all).plus, stdDev(all) / Math.sqrt(8));
  assert.deepEqual(errorAmounts({ dir: "y", barType: "both", valueType: "custom", plus: [1, 2], minus: [3, 4] }, 1, 0, all), { plus: 2, minus: 4 });
});

test("a theme colour is resolved against the theme, with Word's lighter and darker shades", () => {
  const theme = { accent2: "ED7D31", dk1: "000000", lt1: "FFFFFF" };
  assert.equal(resolveColor("scheme:accent2", theme), "ED7D31");
  assert.equal(resolveColor("scheme:bg1", theme), "FFFFFF");
  assert.equal(resolveColor("336699", theme), "336699");
  assert.equal(resolveColor("nonsense", theme), undefined);
  const lighter = resolveColor("scheme:accent2:lumMod=40000:lumOff=60000", theme)!;
  const base = parseInt("ED7D31", 16);
  assert.ok(parseInt(lighter.slice(0, 2), 16) >= (base >> 16), lighter);
  assert.ok(lighter !== "ED7D31");
  const rows = themeSwatches(theme, (n, s) => `${n} ${s}`);
  assert.equal(rows.length, 6);
  assert.equal(rows[0]!.length, 10);
  assert.equal(rows[0]![5]!.spec, "scheme:accent2");
});

test("logarithmic ticks run in powers of the base, and positions follow the logarithm", () => {
  const t = niceTicks(3, 4500, 5, { log: 10 });
  assert.deepEqual(t.values, [1, 10, 100, 1000, 10000]);
  assert.equal(t.log, 10);
  near(frac(t, 100), 0.5);
  assert.ok(t.minor.includes(20) && t.minor.includes(300));
  const fixed = niceTicks(0, 1, 5, { min: 10, max: 1000, log: 10 });
  assert.deepEqual(fixed.values, [10, 100, 1000]);
  // Linear ticks keep a fixed minor unit.
  const lin = niceTicks(0, 10, 5, { min: 0, max: 10, step: 5, minorStep: 1 });
  assert.deepEqual(lin.values, [0, 5, 10]);
  assert.deepEqual(lin.minor, [1, 2, 3, 4, 6, 7, 8, 9]);
  assert.equal(unitDivisor("millions"), 1e6);
  assert.equal(unitDivisor(undefined), 1);
});

test("a draft that changed nothing asks for nothing", () => {
  const m = model();
  assert.equal(diffOptions(m, draftOf(m)), undefined);
});

test("what changed in the draft becomes options: axes, a series' colour and labels, the legend", () => {
  const m = model();
  const d = draftOf(m);
  d.f.yAxis.title = "Money";
  d.f.yAxis.max = 100;
  d.f.yAxis.majorGrid = false;
  d.f.series[1]!.color = "scheme:accent4";
  d.f.series[0]!.labels = { value: true, position: "outEnd" };
  d.f.series[0]!.trendlines = [{ type: "linear", equation: true }];
  d.f.legend = { position: "tr" };
  d.f.legendOverlay = true;
  const o = diffOptions(m, d)!;
  assert.deepEqual(o.yAxis, { title: "Money", max: 100, majorGrid: null });
  assert.equal(o.xAxis, undefined);
  assert.deepEqual(o.legend, { position: "tr" });
  assert.equal(o.legendOverlay, true);
  assert.deepEqual(o.series, [
    { index: 0, labels: { value: true, position: "outEnd" }, trendlines: [{ type: "linear", equation: true }] },
    { index: 1, color: "scheme:accent4" },
  ]);
  // Removing the legend, and a setting back to automatic.
  const d2 = draftOf(m);
  d2.f.legend = undefined;
  delete d2.f.yAxis.majorGrid;
  const o2 = diffOptions(m, d2)!;
  assert.equal(o2.legend, null);
  assert.deepEqual(o2.yAxis, { majorGrid: null });
});

test("a series added in the dialog starts like the last one and only differences are sent", () => {
  const m = model();
  m.series[1]!.labels = { value: true };
  const d = draftOf(m);
  addSeries(d, "Profit");
  assert.equal(d.series.length, 3);
  assert.deepEqual(d.f.series[2]!.labels, { value: true });
  assert.equal(d.f.series[2]!.color, undefined);
  assert.equal(diffOptions(m, d), undefined, "it copies the labels the engine copies too");
  d.f.series[2]!.kind = "line";
  d.f.series[2]!.secondary = true;
  d.f.series[2]!.color = "FF0000";
  assert.deepEqual(diffOptions(m, d)!.series, [{ index: 2, color: "FF0000", kind: "line", secondary: true }]);
});

test("switching to a stock chart makes the series its type has, and switching type renames them", () => {
  const m = model({ series: [1, 2, 3, 4].map((n) => ({ name: `S${n}`, values: [n, n, n, n], kind: "column" as const })) });
  const d = draftOf(m);
  switchKind(d, "stock");
  assert.equal(d.kind, "stock");
  assert.equal(d.f.stock, "ohlc");
  assert.deepEqual(d.series.map((s) => s.name), stockRoles("ohlc"));
  assert.ok(d.f.series.every((s) => s.kind === "stock"));
  setStockType(d, "vohlc");
  assert.equal(d.series.length, 5);
  assert.equal(d.f.series[0]!.kind, "column");
  assert.equal(d.f.series[1]!.secondary, true);
  assert.equal(d.series[0]!.name, stockRoles("vohlc")[0]);
  // Back to a column chart: one kind again.
  switchKind(d, "column");
  assert.ok(d.f.series.every((s) => s.kind === "column" && !s.secondary));
  assert.equal(d.f.stock, undefined);
});
