// What goes into the element of a chart of the new kinds in the text: its drawing, its data as a table for screen
// readers, and the label the object-level tools read. The element itself (class, size, placement) is made by
// the renderer like any chart.
import { t } from "@docxtor/extensions";
import type { ChartExModel, RImage } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { toast } from "../ui/dom";
import { chartExLabel, chartExTable, drawChartEx } from "./chartex-draw";
import { registerChartEx } from "./registry";

export function fillChartEx(el: HTMLElement, model: ChartExModel, im: RImage) {
  el.append(drawChartEx(model, im.w, im.h), chartExTable(model));
  el.dataset.chart = model.kind;
  el.dataset.chartex = model.kind;
  el.dataset.alt = im.alt ?? chartExLabel(model);
  registerChartEx(el, model);
}

/**
 * Editing a chart of the new kinds: Docxtor can't, so say where it can be done. True when the chart at `at` is one
 * (the classic chart dialog is then not opened).
 */
export function editChartEx(ed: Editor, at: { p: number; o: number }): boolean {
  const atom = ed.root.querySelector<HTMLElement>(`[data-p="${at.p}"] .atom.img[data-o="${at.o}"]`);
  if (!atom?.dataset.chartex) return false;
  toast(t("Edit this chart type in Word"));
  return true;
}
