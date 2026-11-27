// The numbers on a chart: Excel number formats and tick values. Pure functions, no DOM.
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { autoRange, formatNumber, general, niceTicks } from "../src/chart/format.ts";

test("General drops floating-point noise", () => {
  assert.equal(general(0.1 + 0.2), "0.3");
  assert.equal(general(1234567), "1234567");
  assert.equal(general(-2.5), "-2.5");
  assert.equal(general(Number.NaN), "");
  assert.equal(formatNumber(12.5), "12.5");
  assert.equal(formatNumber(12.5, "General"), "12.5");
});

test("number format codes", () => {
  assert.equal(formatNumber(3.14159, "0"), "3");
  assert.equal(formatNumber(3.14159, "0.00"), "3.14");
  assert.equal(formatNumber(1234567.891, "#,##0"), "1,234,568");
  assert.equal(formatNumber(1234.5, "#,##0.00"), "1,234.50");
  assert.equal(formatNumber(0.256, "0%"), "26%");
  assert.equal(formatNumber(0.2564, "0.0%"), "25.6%");
  assert.equal(formatNumber(1234.5, "$#,##0.00"), "$1,234.50");
  assert.equal(formatNumber(3.5, '"EUR "0.0'), "EUR 3.5");
  assert.equal(formatNumber(7, '0" kg"'), "7 kg");
  assert.equal(formatNumber(5, "00"), "05");
  assert.equal(formatNumber(0.5, "#.00"), ".50");
});

test("negative numbers: a minus sign, or the format's own second section", () => {
  assert.equal(formatNumber(-1234, "#,##0"), "-1,234");
  assert.equal(formatNumber(-5, "0;(0)"), "(5)");
  assert.equal(formatNumber(5, "0;(0)"), "5");
  assert.equal(formatNumber(-0.4, "0"), "0");
});

test("a code Docxtor can't read falls back to General", () => {
  assert.equal(formatNumber(12.5, "@@@"), "12.5");
});

test("ticks run in round steps that cover the data", () => {
  const t = niceTicks(0, 42);
  assert.deepEqual([t.min, t.max, t.step], [0, 50, 10]);
  assert.deepEqual(t.values, [0, 10, 20, 30, 40, 50]);
  const small = niceTicks(0, 1);
  assert.deepEqual([small.min, small.max], [0, 1]);
  assert.ok(small.step <= 0.25);
  const neg = niceTicks(-3, 8);
  assert.equal(neg.min, -5);
  assert.equal(neg.max, 10);
  assert.ok(neg.values.includes(0));
});

test("fixed ends and steps are kept", () => {
  const t = niceTicks(0, 42, 5, { min: 10, max: 30, step: 5 });
  assert.deepEqual(t.values, [10, 15, 20, 25, 30]);
  const only = niceTicks(0, 42, 5, { max: 100 });
  assert.equal(only.max, 100);
  assert.equal(only.min, 0);
});

test("ticks for one value, and for nothing", () => {
  const same = niceTicks(5, 5);
  assert.ok(same.min < 5 && same.max > 5);
  const none = niceTicks(Number.NaN, Number.NaN);
  assert.deepEqual([none.min, none.max], [0, 1]);
});

test("Excel's rule for where an automatic axis starts", () => {
  assert.deepEqual(autoRange(24, 42, true), [0, 42]);
  assert.deepEqual(autoRange(10, 100, false), [0, 100]);
  // Data far from zero on a line chart: the axis doesn't start at zero.
  assert.deepEqual(autoRange(95, 100, false), [95, 100]);
  assert.deepEqual(autoRange(-20, -10, true), [-20, 0]);
});
