// Charts, as an extension: Insert, Chart puts a sample chart in the text (the data is then edited in a
// dialog), and Edit chart opens that dialog for the selected chart. The drawing itself is the editor's
// (chart/draw.ts); the assistant has its own tools for charts (insert_chart, set_chart).
import { DEFAULT_ICON_PACK, defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import type { ChartKind } from "@docxtor/core";
import { sampleChart, type SampleVariant } from "../chart/sample";
import { CHART_ICONS, type ChartIconName } from "../ui/chart-icons";
import type { Internals } from "./index";

type Kind = Exclude<ChartKind, "other">;

const INSERT: { id: string; kind: Kind; variant?: SampleVariant; title: () => string; icon: ChartIconName }[] = [
  { id: "column", kind: "column", title: () => t("Column chart"), icon: "chartColumn" },
  { id: "bar", kind: "bar", title: () => t("Bar chart"), icon: "chartBar" },
  { id: "line", kind: "line", title: () => t("Line chart"), icon: "chartLine" },
  { id: "pie", kind: "pie", title: () => t("Pie chart"), icon: "chartPie" },
  { id: "doughnut", kind: "doughnut", title: () => t("Doughnut chart"), icon: "chartDoughnut" },
  { id: "area", kind: "area", title: () => t("Area chart"), icon: "chartArea" },
  { id: "scatter", kind: "scatter", title: () => t("Scatter chart"), icon: "chartScatter" },
  { id: "radar", kind: "radar", title: () => t("Radar chart"), icon: "chartRadar" },
  { id: "bubble", kind: "bubble", title: () => t("Bubble chart"), icon: "chartBubble" },
  { id: "stock.hlc", kind: "stock", variant: "hlc", title: () => t("High-low-close stock chart"), icon: "chartStock" },
  { id: "stock.ohlc", kind: "stock", variant: "ohlc", title: () => t("Open-high-low-close stock chart"), icon: "chartStock" },
  { id: "stock.vhlc", kind: "stock", variant: "vhlc", title: () => t("Volume-high-low-close stock chart"), icon: "chartStock" },
  { id: "stock.vohlc", kind: "stock", variant: "vohlc", title: () => t("Volume-open-high-low-close stock chart"), icon: "chartStock" },
  { id: "surface.surface", kind: "surface", variant: "surface", title: () => t("3-D surface chart"), icon: "chartSurface" },
  { id: "surface.wireframe", kind: "surface", variant: "wireframe", title: () => t("Wireframe 3-D surface chart"), icon: "chartSurface" },
  { id: "surface.contour", kind: "surface", variant: "contour", title: () => t("Contour chart"), icon: "chartSurface" },
  { id: "surface.wireframe_contour", kind: "surface", variant: "wireframe_contour", title: () => t("Wireframe contour chart"), icon: "chartSurface" },
  { id: "combo.combo", kind: "column", variant: "combo", title: () => t("Clustered column and line chart"), icon: "chartCombo" },
  { id: "combo.combo_secondary", kind: "column", variant: "combo_secondary", title: () => t("Clustered column and line chart on a second axis"), icon: "chartCombo" },
  { id: "combo.combo_area", kind: "column", variant: "combo_area", title: () => t("Area and clustered column chart"), icon: "chartCombo" },
];

export function charts({ fe, editor: e, images }: Internals): Extension {
  return defineExtension({
    id: "docxtor.charts",
    name: msg("Charts"),
    description: msg("Insert, Chart puts a chart in the text; its data, type and title are edited in a dialog. Charts stay editable in Word."),
    activate(ctx) {
      ctx.iconPacks.register({ id: DEFAULT_ICON_PACK, label: msg("Docxtor"), icons: Object.entries(CHART_ICONS).map(([id, svg]) => ({ id, svg })) });
      const editable = () => !!fe.engine && !fe.readOnly;

      for (const { id, kind, variant, title, icon } of INSERT) {
        ctx.commands.register(
          `insert.chart.${id}`,
          () => {
            const r = e.range();
            if (!r) return;
            const at = r[0];
            const res = e.exec({ op: "insert_chart", at, ...sampleChart(kind, variant) });
            // Select the new chart, so its frame and handles show and the next step (Edit chart) is one click.
            if (res) images.selectAt(at.p, at.o);
          },
          { title: title(), category: t("Insert"), when: editable, icon },
        );
      }
      ctx.commands.register("chart.edit", () => images.editChart(), {
        title: t("Edit chart…"),
        category: t("Chart"),
        when: () => editable() && images.selected && images.isChart,
        icon: "chart",
      });
    },
  });
}
