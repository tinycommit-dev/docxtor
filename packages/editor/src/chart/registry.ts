// The chart each drawn chart element shows, so a chart can be drawn again at another size (while it is
// being resized with the handles) without asking the engine.
import type { ChartExModel, ChartModel } from "@docxtor/core";
import { drawChart } from "./draw";
import { drawChartEx } from "./chartex-draw";

const models = new WeakMap<HTMLElement, ChartModel>();
const exModels = new WeakMap<HTMLElement, ChartExModel>();

export function registerChart(el: HTMLElement, model: ChartModel) {
  models.set(el, model);
}

/** A chart of the new kinds (waterfall, treemap...), drawn by chartex-draw. */
export function registerChartEx(el: HTMLElement, model: ChartExModel) {
  exModels.set(el, model);
}

export function chartOf(el: HTMLElement | null): ChartModel | ChartExModel | undefined {
  return el ? models.get(el) ?? exModels.get(el) : undefined;
}

/** Draw the chart in `el` again for a new size in points. */
export function redrawChart(el: HTMLElement, wPt: number, hPt: number) {
  const model = models.get(el);
  const ex = model ? undefined : exModels.get(el);
  if (!model && !ex) return;
  const next = model ? drawChart(model, wPt, hPt) : drawChartEx(ex!, wPt, hPt);
  const old = el.querySelector(".chart-svg");
  if (old) old.replaceWith(next);
  else el.prepend(next);
}
