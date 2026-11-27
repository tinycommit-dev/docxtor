// The panes of the Edit chart dialog that are about the chart as a whole: its type and data, its layout (title,
// legend, labels, gridlines) and its axes.
import type { Axis, ChartKind, DataLabels, Engine } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { drawChart } from "../chart/draw";
import { STOCK_TYPES, SURFACE_TYPES, isStackable, isXY, setStockType, switchKind, type Draft, type Kind } from "../chart/draft";
import { CHART_ICONS, type ChartIconName } from "./chart-icons";
import { check, field, group, numberInput, row, select, uniq } from "./chart-controls";
import type { Grid } from "./chart-grid";
import { h, svg } from "./dom";

/** What the panes share. */
export interface Ctx {
  d: Draft;
  engine: Engine;
  /** The draft changed: redraw the preview and check the numbers. */
  changed(): void;
  /** The number or kind of the series changed: panes that list them start over. */
  reshaped(): void;
  /** Draw the current pane again. */
  rerender(): void;
  /** Something the user can't fix by typing went wrong. */
  fail(message: string): void;
  grid: Grid;
  /** What the dialog remembers while it is open. */
  sel: { series: number; axis: "x" | "y" | "y2"; point: number; title: string };
  /** The chart as it was. */
  base: import("@docxtor/core").ChartModel;
  /** Palettes extensions offer. */
  extraPalettes: import("@docxtor/extensions").ChartPaletteContribution[];
}

export const KINDS: { kind: Kind; label: () => string; icon: ChartIconName }[] = [
  { kind: "column", label: () => t("Column"), icon: "chartColumn" },
  { kind: "bar", label: () => t("Bar"), icon: "chartBar" },
  { kind: "line", label: () => t("Line"), icon: "chartLine" },
  { kind: "area", label: () => t("Area"), icon: "chartArea" },
  { kind: "pie", label: () => t("Pie"), icon: "chartPie" },
  { kind: "doughnut", label: () => t("Doughnut"), icon: "chartDoughnut" },
  { kind: "scatter", label: () => t("Scatter"), icon: "chartScatter" },
  { kind: "bubble", label: () => t("Bubble"), icon: "chartBubble" },
  { kind: "radar", label: () => t("Radar"), icon: "chartRadar" },
  { kind: "stock", label: () => t("Stock"), icon: "chartStock" },
  { kind: "surface", label: () => t("Surface"), icon: "chartSurface" },
];

export const hasAxes = (k: ChartKind): boolean => k !== "pie" && k !== "doughnut" && k !== "other";

// -- the type and the data --------------------------------------------------------------------------------------

export function chartPane(ctx: Ctx, kindTouched: { value: boolean }): HTMLElement {
  const { d } = ctx;
  const stackLabel = { clustered: () => t("Side by side"), stacked: () => t("Stacked"), percent: () => t("Stacked to 100%") };
  const types = h("div", { class: "chart-types", role: "radiogroup", "aria-label": t("Chart type") });
  const buttons = KINDS.map(({ kind, label, icon }) => {
    const on = kind === d.kind && (!ctx.base.combo || kindTouched.value) && (ctx.base.kind !== "other" || kindTouched.value);
    const b = h("button", { type: "button", class: `chart-type${on ? " on" : ""}`, role: "radio", "aria-checked": String(on), tabindex: kind === d.kind ? "0" : "-1", "data-kind": kind }, svg(CHART_ICONS[icon]), h("span", {}, label()));
    b.addEventListener("click", () => pick(kind));
    types.append(b);
    return [kind, b] as const;
  });
  const pick = (kind: Kind) => {
    kindTouched.value = true;
    switchKind(d, kind);
    ctx.grid.render();
    ctx.reshaped();
    ctx.changed();
    ctx.rerender();
    (document.querySelector(`.chart-types [data-kind="${kind}"]`) as HTMLElement | null)?.focus();
  };
  types.addEventListener("keydown", (e) => {
    const i = KINDS.findIndex((k) => k.kind === d.kind);
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    pick(KINDS[(i + step + KINDS.length) % KINDS.length]!.kind);
  });
  void buttons;

  const notes: HTMLElement[] = [];
  if (ctx.base.combo && !kindTouched.value) notes.push(h("p", { class: "help" }, t("This chart combines several types. Choosing a type makes them all the same. To mix types, use the Series tab.")));
  if (ctx.base.kind === "other" && !kindTouched.value) notes.push(h("p", { class: "help" }, t("Docxtor can't edit this kind of chart yet. Pick a type to replace it.")));

  // The kind's own choice: stacking, the stock type, the surface type, the doughnut hole.
  let variant: HTMLElement | null = null;
  if (isStackable(d.kind)) {
    const s = select((["clustered", "stacked", "percent"] as const).map((g) => [g, stackLabel[g]()] as [string, string]), d.grouping, (v) => {
      d.grouping = v as Draft["grouping"];
      ctx.changed();
    });
    variant = field(t("Style"), s);
  } else if (d.kind === "stock") {
    const s = select(STOCK_TYPES.map((x) => [x.type, x.label()] as [string, string]), d.f.stock ?? "hlc", (v) => {
      setStockType(d, v as never);
      ctx.grid.render();
      ctx.reshaped();
      ctx.changed();
    });
    variant = field(t("Stock chart type"), s);
  } else if (d.kind === "surface") {
    const s = select(SURFACE_TYPES.map((x) => [x.type, x.label()] as [string, string]), d.f.surface ?? "surface", (v) => {
      d.f.surface = v as never;
      ctx.changed();
    });
    variant = field(t("Surface chart type"), s);
  } else if (d.kind === "doughnut") {
    variant = field(t("Hole size, percent"), numberInput(d.f.hole ?? 50, (v) => {
      d.f.hole = v === undefined ? undefined : Math.max(10, Math.min(90, Math.round(v)));
      ctx.changed();
    }, { min: 10, max: 90 }));
  }

  const title = h("input", { type: "text", id: "chart-title", value: d.title, autocomplete: "off", placeholder: t("Chart title") });
  title.addEventListener("input", () => {
    d.title = title.value;
    ctx.changed();
  });
  return h("div", { class: "chart-pane-body" }, h("div", { class: "field" }, h("label", { id: "chart-type-label" }, t("Chart type")), types, ...notes), variant, field(t("Title"), title), h("div", { class: "field" }, h("label", {}, t("Data")), ctx.grid.el));
}

// -- data labels ------------------------------------------------------------------------------------------------

const POSITION_LABEL: Record<string, () => string> = {
  ctr: () => t("Center"),
  inEnd: () => t("Inside end"),
  inBase: () => t("Inside base"),
  outEnd: () => t("Outside end"),
  bestFit: () => t("Best fit"),
  t: () => t("Above"),
  b: () => t("Below"),
  l: () => t("Left"),
  r: () => t("Right"),
};

/** The label places the file format allows for a kind of series (anything else makes Word refuse the file). */
export function labelPositions(kind: ChartKind, grouping: string): string[] {
  switch (kind) {
    case "column":
    case "bar":
      return grouping === "clustered" ? ["ctr", "inEnd", "inBase", "outEnd"] : ["ctr", "inEnd", "inBase"];
    case "line":
    case "scatter":
    case "radar":
    case "bubble":
      return ["t", "b", "l", "r", "ctr"];
    case "pie":
      return ["bestFit", "ctr", "inEnd", "outEnd"];
    default:
      return [];
  }
}

const SEPARATORS: [string, () => string][] = [
  [", ", () => t("Comma")],
  ["; ", () => t("Semicolon")],
  [". ", () => t("Period")],
  ["\n", () => t("New line")],
  [" ", () => t("Space")],
];

export const defaultLabels = (kind: ChartKind): DataLabels => (kind === "pie" || kind === "doughnut" ? { percent: true } : { value: true });

/**
 * Editor of data labels. `get` is the labels it shows, `edit` changes them (for every series it stands for),
 * `master` turns them on or off.
 */
export function labelEditor(kind: ChartKind, grouping: string, get: () => DataLabels | undefined, edit: (f: (l: DataLabels) => void) => void, master: (on: boolean) => void, rerender: () => void): HTMLElement {
  const l = get();
  const root = h("div", { class: "chart-labels-editor" });
  root.append(check(t("Show data labels"), !!l, (on) => {
    master(on);
    rerender();
  }));
  if (!l) return root;
  const flag = (label: string, key: keyof DataLabels) => check(label, !!l[key], (on) => edit((x) => ((x as Record<string, unknown>)[key] = on || undefined)));
  const xy = isXY(kind);
  root.append(
    h(
      "div",
      { class: "chart-checks" },
      flag(t("Value"), "value"),
      flag(xy ? t("X value") : t("Category name"), "category"),
      flag(t("Series name"), "series"),
      kind === "pie" || kind === "doughnut" ? flag(t("Percentage"), "percent") : null,
      kind === "bubble" ? flag(t("Bubble size"), "bubbleSize") : null,
      flag(t("Legend key"), "legendKey"),
    ),
  );
  const positions = labelPositions(kind, grouping);
  const pos = positions.length ? field(t("Label position"), select([["", t("Default")] as [string, string], ...positions.map((p) => [p, POSITION_LABEL[p]!()] as [string, string])], l.position ?? "", (v) => edit((x) => (x.position = v || undefined)))) : null;
  const sep = field(t("Separator"), select(SEPARATORS.map(([v, f]) => [v, f()] as [string, string]), l.separator ?? ", ", (v) => edit((x) => (x.separator = v === ", " ? undefined : v))));
  const fmt = field(t("Number format"), (() => {
    const i = h("input", { type: "text", autocomplete: "off", spellcheck: "false", placeholder: t("Linked to source"), value: l.numberFormat ?? "" });
    i.addEventListener("input", () => edit((x) => (x.numberFormat = i.value.trim() || undefined)));
    return i;
  })());
  root.append(row(pos, sep), fmt);
  return root;
}

// -- the layout -------------------------------------------------------------------------------------------------

export function layoutPane(ctx: Ctx): HTMLElement {
  const { d } = ctx;
  const f = d.f;
  const axes = hasAxes(d.kind);
  // Title.
  const mode = !d.title ? "none" : f.titleOverlay ? "overlay" : "above";
  const titleSel = select([["none", t("None")], ["above", t("Above chart")], ["overlay", t("Overlay")]] as [string, string][], mode, (v) => {
    if (v === "none") {
      if (d.title) ctx.sel.title = d.title;
      d.title = "";
      f.titleOverlay = undefined;
    } else {
      if (!d.title) d.title = ctx.sel.title || t("Chart title");
      f.titleOverlay = v === "overlay" || undefined;
    }
    ctx.changed();
    ctx.rerender();
  });
  const titleText = h("input", { type: "text", value: d.title, autocomplete: "off", placeholder: t("Chart title") });
  titleText.addEventListener("input", () => {
    const was = !!d.title;
    d.title = titleText.value;
    ctx.changed();
    if (was !== !!d.title) ctx.rerender();
  });
  // Legend.
  const legendSel = select(
    [["none", t("None")], ["r", t("Right")], ["b", t("Bottom")], ["t", t("Top")], ["l", t("Left")], ["tr", t("Top right")]] as [string, string][],
    f.legend ? f.legend.position : "none",
    (v) => {
      f.legend = v === "none" ? undefined : { position: v as "r" };
      if (v === "none") f.legendOverlay = undefined;
      ctx.changed();
      ctx.rerender();
    },
  );
  // Data labels of all series.
  const first = f.series.find((s) => s.labels)?.labels;
  const labels = labelEditor(
    d.kind,
    d.grouping,
    () => first,
    (fn) => {
      for (const s of f.series) if (s.labels) fn(s.labels);
      ctx.changed();
    },
    (on) => {
      for (const s of f.series) s.labels = on ? (s.labels ?? { ...defaultLabels(d.kind) }) : undefined;
      ctx.changed();
    },
    ctx.rerender,
  );
  const body = h(
    "div",
    { class: "chart-pane-body" },
    group(t("Chart title"), row(field(t("Title"), titleText), field(t("Title position"), titleSel))),
    group(t("Legend"), row(field(t("Legend position"), legendSel), check(t("Show over the chart"), !!f.legendOverlay, (on) => {
      f.legendOverlay = on || undefined;
      ctx.changed();
    }, !f.legend))),
    group(t("Data labels"), labels, h("p", { class: "help" }, t("These apply to every series. The Series tab has labels for one series."))),
  );
  if (axes) {
    const xyKind = isXY(d.kind);
    const titleField = (label: string, get: () => string | undefined, set: (v: string | undefined) => void) => {
      const i = h("input", { type: "text", value: get() ?? "", autocomplete: "off" });
      i.addEventListener("input", () => {
        set(i.value || undefined);
        ctx.changed();
      });
      return field(label, i);
    };
    const hasSecond = f.series.some((s) => s.secondary);
    body.append(
      group(
        t("Axis titles"),
        titleField(xyKind ? t("Horizontal axis (X)") : t("Category axis"), () => f.xAxis.title, (v) => (f.xAxis.title = v)),
        titleField(xyKind ? t("Vertical axis (Y)") : t("Value axis"), () => f.yAxis.title, (v) => (f.yAxis.title = v)),
        hasSecond ? titleField(t("Second value axis"), () => f.y2Axis?.title, (v) => (f.y2Axis = { ...(f.y2Axis ?? {}), title: v })) : null,
      ),
      group(
        t("Gridlines"),
        h(
          "div",
          { class: "chart-checks" },
          d.kind === "radar" || d.kind === "surface" ? null : check(xyKind ? t("Horizontal axis, major") : t("Category axis, major"), !!f.xAxis.majorGrid, (on) => ((f.xAxis.majorGrid = on || undefined), ctx.changed())),
          d.kind === "radar" || d.kind === "surface" ? null : check(xyKind ? t("Horizontal axis, minor") : t("Category axis, minor"), !!f.xAxis.minorGrid, (on) => ((f.xAxis.minorGrid = on || undefined), ctx.changed())),
          check(xyKind ? t("Vertical axis, major") : t("Value axis, major"), !!f.yAxis.majorGrid, (on) => ((f.yAxis.majorGrid = on || undefined), ctx.changed())),
          check(xyKind ? t("Vertical axis, minor") : t("Value axis, minor"), !!f.yAxis.minorGrid, (on) => ((f.yAxis.minorGrid = on || undefined), ctx.changed())),
          hasSecond ? check(t("Second value axis, major"), !!f.y2Axis?.majorGrid, (on) => ((f.y2Axis = { ...(f.y2Axis ?? {}), majorGrid: on || undefined }), ctx.changed())) : null,
        ),
      ),
    );
  }
  return body;
}

// -- the axes ---------------------------------------------------------------------------------------------------

const NUMBER_PRESETS: [string, string, () => string][] = [
  ["general", "", () => t("General")],
  ["number", "#,##0.00", () => t("Number")],
  ["currency", "$#,##0.00", () => t("Currency")],
  ["percentage", "0%", () => t("Percentage")],
  ["date", "m/d/yyyy", () => t("Date")],
];

const UNITS: [string, () => string][] = [
  ["", () => t("None")],
  ["hundreds", () => t("Hundreds")],
  ["thousands", () => t("Thousands")],
  ["tenThousands", () => t("Ten thousands")],
  ["hundredThousands", () => t("Hundred thousands")],
  ["millions", () => t("Millions")],
  ["tenMillions", () => t("Ten millions")],
  ["hundredMillions", () => t("Hundred millions")],
  ["billions", () => t("Billions")],
  ["trillions", () => t("Trillions")],
];

const TICKS = (): [string, string][] => [["none", t("None")], ["in", t("Inside")], ["out", t("Outside")], ["cross", t("Cross")]];
const LABEL_POS = (): [string, string][] => [["nextTo", t("Next to axis")], ["low", t("Low")], ["high", t("High")], ["none", t("None")]];

export function axesPane(ctx: Ctx): HTMLElement {
  const { d } = ctx;
  const f = d.f;
  if (!hasAxes(d.kind)) return h("div", { class: "chart-pane-body" }, h("p", { class: "help" }, t("This kind of chart has no axes.")));
  const xyKind = isXY(d.kind);
  const hasSecond = f.series.some((s) => s.secondary);
  if (ctx.sel.axis === "y2" && !hasSecond) ctx.sel.axis = "y";
  const which = ctx.sel.axis;
  const which3: [string, string][] = [["x", xyKind ? t("Horizontal (X)") : t("Category")], ["y", xyKind ? t("Vertical (Y)") : t("Value")], ...(hasSecond ? [["y2", t("Second value")] as [string, string]] : [])];
  const seg = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Axis") });
  for (const [v, l] of which3) {
    const b = h("button", { type: "button", role: "radio", class: v === which ? "on" : "", "aria-checked": String(v === which) }, l);
    b.addEventListener("click", () => {
      ctx.sel.axis = v as "x" | "y" | "y2";
      ctx.rerender();
    });
    seg.append(b);
  }
  const ax: Axis = which === "x" ? f.xAxis : which === "y" ? f.yAxis : (f.y2Axis ??= {});
  const isValue = which !== "x" || xyKind;
  const set = <K extends keyof Axis>(k: K, v: Axis[K] | undefined | false) => {
    if (v === undefined || v === false) delete ax[k];
    else ax[k] = v as Axis[K];
    ctx.changed();
  };
  const show = check(t("Show axis"), !ax.hidden, (on) => set("hidden", !on || undefined));
  // Scale.
  const num = (label: string, key: "min" | "max" | "majorUnit" | "minorUnit") => field(label, numberInput(ax[key], (v) => set(key, v), { placeholder: t("Automatic"), label }));
  const scale = isValue
    ? group(
        t("Scale"),
        row(num(t("Minimum"), "min"), num(t("Maximum"), "max")),
        row(num(t("Major unit"), "majorUnit"), num(t("Minor unit"), "minorUnit")),
        row(
          check(t("Logarithmic scale"), ax.logBase !== undefined, (on) => {
            set("logBase", on ? 10 : undefined);
            ctx.rerender();
          }),
          ax.logBase !== undefined ? field(t("Base"), numberInput(ax.logBase, (v) => set("logBase", v), { min: 2, max: 1000, label: t("Logarithmic base") })) : null,
        ),
        check(t("Values in reverse order"), !!ax.reverse, (on) => set("reverse", on || undefined)),
      )
    : group(t("Scale"), check(t("Categories in reverse order"), !!ax.reverse, (on) => set("reverse", on || undefined)));
  // Numbers.
  const linked = ax.numberFormat === undefined;
  const preset = NUMBER_PRESETS.find((p) => p[1] === (ax.numberFormat ?? ""))?.[0] ?? "custom";
  const code = h("input", { type: "text", autocomplete: "off", spellcheck: "false", "aria-label": t("Format code"), value: ax.numberFormat ?? "" });
  code.addEventListener("input", () => set("numberFormat", code.value || undefined));
  const numbers = isValue
    ? group(
        t("Numbers"),
        check(t("Linked to source"), linked, (on) => {
          set("numberFormat", on ? undefined : "#,##0.00");
          ctx.rerender();
        }),
        linked
          ? null
          : row(
              field(t("Category"), select([...NUMBER_PRESETS.map(([k, , l]) => [k, l()] as [string, string]), ["custom", t("Custom")] as [string, string]], preset, (v) => {
                const p = NUMBER_PRESETS.find((x) => x[0] === v);
                if (p) set("numberFormat", p[1] || "General");
                ctx.rerender();
              })),
              field(t("Format code"), code),
            ),
        field(t("Display units"), select(UNITS.map(([v, l]) => [v, l()] as [string, string]), ax.displayUnits ?? "", (v) => set("displayUnits", v || undefined))),
      )
    : null;
  const ticks = group(
    t("Tick marks and labels"),
    row(field(t("Major tick marks"), select(TICKS(), ax.majorTick ?? "none", (v) => set("majorTick", v))), field(t("Minor tick marks"), select(TICKS(), ax.minorTick ?? "none", (v) => set("minorTick", v)))),
    field(t("Labels"), select(LABEL_POS(), ax.labelPos ?? "nextTo", (v) => set("labelPos", v))),
  );
  const grid = group(
    t("Gridlines"),
    h("div", { class: "chart-checks" }, check(t("Major gridlines"), !!ax.majorGrid, (on) => set("majorGrid", on || undefined)), check(t("Minor gridlines"), !!ax.minorGrid, (on) => set("minorGrid", on || undefined))),
  );
  const title = h("input", { type: "text", value: ax.title ?? "", autocomplete: "off" });
  title.addEventListener("input", () => set("title", title.value || undefined));
  return h("div", { class: "chart-pane-body" }, seg, field(t("Axis title"), title), show, scale, numbers, ticks, grid);
}

/** A small drawing of the chart for a gallery tile. */
export function thumb(model: import("@docxtor/core").ChartModel): SVGElement {
  // Drawn at the size of the preview and shrunk by the tile, so the text shrinks with it.
  const s = drawChart(model, 300, 210);
  s.removeAttribute("role");
  s.setAttribute("aria-hidden", "true");
  s.querySelectorAll("title").forEach((x) => x.remove());
  return s;
}

void uniq;
