// The sample data a new chart starts with, as in Google Docs: something to look at and then edit.
import { t } from "@docxtor/extensions";
import type { ChartKind, InsertChartOp } from "@docxtor/core";

type Sample = Omit<InsertChartOp, "op" | "at">;

/** The sample charts that are a kind with a variant: stock and surface types, and combination charts. */
export type SampleVariant = "hlc" | "ohlc" | "vhlc" | "vohlc" | "surface" | "wireframe" | "contour" | "wireframe_contour" | "combo" | "combo_secondary" | "combo_area";

/** A sample chart for a kind: the title, categories and series are in the reader's language. */
export function sampleChart(kind: Exclude<ChartKind, "other">, variant?: SampleVariant): Sample {
  const q = [t("Q1"), t("Q2"), t("Q3"), t("Q4")];
  const sales = t("Sales");
  const costs = t("Costs");
  switch (kind) {
    case "pie":
    case "doughnut":
      return {
        kind,
        title: t("Chart title"),
        categories: [t("Rent"), t("Food"), t("Transport"), t("Other")],
        series: [{ name: t("Share"), values: [45, 30, 15, 10] }],
        data_labels: true,
        legend: "right",
      };
    case "scatter":
      return {
        kind,
        title: t("Chart title"),
        series: [
          { name: sales, x_values: [1, 2, 3, 4, 5, 6], values: [2.1, 3.4, 3.1, 5.2, 6.8, 7.4] },
          { name: costs, x_values: [1, 2, 3, 4, 5, 6], values: [1.4, 1.9, 2.6, 3.1, 3.3, 4.2] },
        ],
        legend: "bottom",
      };
    case "bubble":
      return {
        kind,
        title: t("Chart title"),
        series: [
          { name: t("North"), x_values: [1, 2, 3, 4], values: [3, 5, 4, 7], sizes: [10, 24, 16, 30] },
          { name: t("South"), x_values: [1, 2, 3, 4], values: [2, 3, 6, 4], sizes: [8, 12, 20, 10] },
        ],
        legend: "bottom",
      };
    case "stock": {
      const days = [t("Mon"), t("Tue"), t("Wed"), t("Thu"), t("Fri"), t("Sat")];
      const high = { name: t("High"), values: [21, 22, 24, 23, 25, 26] };
      const low = { name: t("Low"), values: [18, 19, 20, 20, 21, 22] };
      const close = { name: t("Close"), values: [20, 21, 21, 22, 24, 25] };
      const open = { name: t("Open"), values: [19, 20, 22, 21, 22, 24] };
      const volume = { name: t("Volume"), values: [1200, 900, 1500, 1100, 1700, 1400] };
      const type = variant === "ohlc" || variant === "vhlc" || variant === "vohlc" ? variant : "hlc";
      const series = type === "hlc" ? [high, low, close] : type === "ohlc" ? [open, high, low, close] : type === "vhlc" ? [volume, high, low, close] : [volume, open, high, low, close];
      return { kind, title: t("Chart title"), categories: days, series, legend: "bottom", options: { stock: type } };
    }
    case "surface": {
      const type = variant === "wireframe" || variant === "contour" || variant === "wireframe_contour" ? variant : "surface";
      const grid = [
        [2, 3, 5, 6, 5],
        [3, 6, 9, 10, 8],
        [4, 8, 12, 13, 9],
        [3, 6, 8, 9, 6],
      ];
      return { kind, title: t("Chart title"), categories: ["A", "B", "C", "D", "E"], series: grid.map((values, i) => ({ name: t("Series {n}", { n: i + 1 }), values })), legend: "right", options: { surface: type } };
    }
    case "line":
    case "area":
      return {
        kind,
        title: t("Chart title"),
        categories: q,
        series: [
          { name: sales, values: [24, 31, 29, 42] },
          { name: costs, values: [15, 18, 22, 24] },
        ],
        legend: "bottom",
      };
    case "radar":
      return {
        kind,
        title: t("Chart title"),
        categories: [t("Speed"), t("Power"), t("Range"), t("Comfort"), t("Price")],
        series: [
          { name: t("Model A"), values: [7, 5, 8, 6, 4] },
          { name: t("Model B"), values: [5, 8, 4, 7, 6] },
        ],
        legend: "bottom",
      };
    default: {
      if (variant === "combo" || variant === "combo_secondary" || variant === "combo_area") {
        const area = variant === "combo_area";
        return {
          kind: "column",
          title: t("Chart title"),
          categories: q,
          series: [
            { name: sales, values: [24, 31, 29, 42], kind: area ? "area" : "column" },
            area ? { name: costs, values: [15, 18, 22, 24], kind: "column" } : { name: t("Margin"), values: [0.12, 0.2, 0.15, 0.28], kind: "line", secondary: variant === "combo_secondary" },
          ],
          legend: "bottom",
          options: variant === "combo_secondary" ? { y2Axis: { numberFormat: "0%" } } : undefined,
        };
      }
      return {
        kind,
        title: t("Chart title"),
        categories: q,
        series: [
          { name: sales, values: [24, 31, 29, 42] },
          { name: costs, values: [15, 18, 22, 24] },
        ],
        legend: "bottom",
      };
    }
  }
}
