// Charts: what the engine reports about a Word chart, and the ops that insert and change one.
import type { MPos } from "./types.ts";

/** `bar` is horizontal bars, `column` vertical ones. `other` is a kind Docxtor doesn't draw. */
export type ChartKind = "column" | "bar" | "line" | "pie" | "doughnut" | "area" | "scatter" | "radar" | "bubble" | "stock" | "surface" | "other";

/** Stock chart types: high-low-close, open-high-low-close, and the same with volume. */
export type StockType = "hlc" | "ohlc" | "vhlc" | "vohlc";

/** Surface chart types. */
export type SurfaceType = "surface" | "wireframe" | "contour" | "wireframe_contour";

/** What a data label shows and where. */
export interface DataLabels {
  value?: boolean;
  category?: boolean;
  series?: boolean;
  percent?: boolean;
  legendKey?: boolean;
  bubbleSize?: boolean;
  /** `ctr`, `inEnd`, `inBase`, `outEnd`, `bestFit`, `t`, `b`, `l` or `r`; absent is the chart's default place. */
  position?: string;
  separator?: string;
  /** Excel number format; absent follows the data. */
  numberFormat?: string;
}

export type TrendlineType = "linear" | "exp" | "log" | "poly" | "power" | "movingAvg";

export interface Trendline {
  type: TrendlineType;
  /** Polynomial order, 2 to 6. */
  order?: number;
  /** Moving average period. */
  period?: number;
  forward?: number;
  backward?: number;
  intercept?: number;
  equation?: boolean;
  rSquared?: boolean;
  name?: string;
}

export interface ErrorBars {
  dir: "x" | "y";
  barType: "both" | "plus" | "minus";
  valueType: "fixed" | "percentage" | "stdDev" | "stdErr" | "custom";
  value?: number;
  plus?: number[];
  minus?: number[];
  noCap?: boolean;
}

/** One axis as the dialog shows it. Absent numbers are automatic. */
export interface Axis {
  title?: string;
  hidden?: boolean;
  reverse?: boolean;
  min?: number;
  max?: number;
  majorUnit?: number;
  minorUnit?: number;
  logBase?: number;
  numberFormat?: string;
  majorGrid?: boolean;
  minorGrid?: boolean;
  /** `none`, `in`, `out` or `cross`. */
  majorTick?: string;
  minorTick?: string;
  /** `nextTo`, `low`, `high` or `none`. */
  labelPos?: string;
  displayUnits?: string;
}

export interface View3d {
  rotX: number;
  rotY: number;
  perspective: number;
}

/** How series of a bar, line or area group combine. */
export type ChartGrouping = "clustered" | "stacked" | "percent";

export interface ChartSeries {
  name: string;
  /** One value per category (the Y values, for a scatter chart); null is a gap. */
  values: (number | null)[];
  /** Hex without `#`, when the file sets one (theme colours are resolved). Absent: Word's automatic colour for the series' position. */
  color?: string;
  /** Scatter charts: the X value of each point. */
  xValues?: (number | null)[];
  /** The series' own kind; differs from the chart's in a combination chart. */
  kind: ChartKind;
  /** Drawn against the second value axis. */
  secondary?: boolean;
  marker?: boolean;
  smooth?: boolean;
  /** Scatter and line series drawn as markers only. */
  noLine?: boolean;
  /** Slice colours (pie, doughnut), by point. */
  pointColors?: (string | null)[];
  /** The spreadsheet ranges the data comes from, e.g. `Sheet1!$B$2:$B$5`. */
  nameRef?: string;
  catRef?: string;
  valRef?: string;
  /** Bubble charts: the size of each bubble. */
  sizes?: (number | null)[];
  sizeRef?: string;
  labels?: DataLabels;
  trendlines?: Trendline[];
  errorBars?: ErrorBars[];
  /** `none` (outline only), `soft` (see-through) or `gradient`. */
  fill?: "none" | "soft" | "gradient";
  /** Line or outline width in points. */
  lineWidth?: number;
  lineColor?: string;
}

export interface ChartAxes {
  /** The category axis title (X for scatter). */
  xTitle?: string;
  /** The value axis title. */
  yTitle?: string;
  yMin?: number;
  yMax?: number;
  yMajor?: number;
  /** Excel number format of the value axis, e.g. `0%` or `#,##0.00`. */
  numberFormat?: string;
  xHidden?: boolean;
  yHidden?: boolean;
  grid: boolean;
  /** The category axis runs backwards. */
  xReverse?: boolean;
  xMin?: number;
  xMax?: number;
  /** The number format the data carries, used while the axis follows the data. */
  dataFormat?: string;
}

export interface ChartModel {
  kind: ChartKind;
  grouping: ChartGrouping;
  /** More than one kind of series (see `series[].kind`). */
  combo?: boolean;
  title?: string;
  categories: string[];
  series: ChartSeries[];
  /** Absent: no legend. */
  legend?: { position: "r" | "l" | "t" | "b" | "tr" };
  axes: ChartAxes;
  dataLabels: boolean;
  /** The labels show percentages (pie, doughnut) instead of values. */
  labelsPercent?: boolean;
  varyColors?: boolean;
  /** Doughnut hole size in percent. */
  hole?: number;
  filled?: boolean;
  style?: number;
  /** The theme's accent colours, in order, for series without a colour of their own. */
  accents?: string[];
  titleOverlay?: boolean;
  legendOverlay?: boolean;
  /** The category axis (X for scatter and bubble charts), the value axis and the second value axis. */
  xAxis: Axis;
  yAxis: Axis;
  y2Axis?: Axis;
  gapWidth?: number;
  overlap?: number;
  bubbleScale?: number;
  sizeRepresents?: "area" | "width";
  showNegativeBubbles?: boolean;
  stock?: StockType;
  surface?: SurfaceType;
  view3d?: View3d;
  /** `colorful1` to `colorful4`, `mono1` to `mono13`, when the chart has a `colorsN.xml`. */
  palette?: string;
  /** The theme's colours by name: dk1, lt1, dk2, lt2, accent1 to accent6, as `RRGGBB`. */
  theme?: Record<string, string>;
}

/** A series as given to an op. */
export interface ChartSeriesInput {
  name: string;
  values: (number | null)[];
  /** Scatter and bubble charts. */
  x_values?: (number | null)[];
  /** Bubble charts. */
  sizes?: (number | null)[];
  /** The series' own type (a combination chart) and axis. */
  kind?: "column" | "bar" | "line" | "area" | "scatter";
  secondary?: boolean;
}

/**
 * Formatting, merged into the chart's current formatting (`null` removes a setting). The same names as the
 * chart model; `series` entries name the series by `index`.
 */
export interface ChartOptions {
  legend?: { position: "r" | "l" | "t" | "b" | "tr" } | null;
  legendOverlay?: boolean;
  titleOverlay?: boolean;
  xAxis?: Partial<Record<keyof Axis, unknown>>;
  yAxis?: Partial<Record<keyof Axis, unknown>>;
  y2Axis?: Partial<Record<keyof Axis, unknown>>;
  gapWidth?: number | null;
  overlap?: number | null;
  bubbleScale?: number | null;
  sizeRepresents?: "area" | "width" | null;
  showNegativeBubbles?: boolean;
  stock?: StockType;
  surface?: SurfaceType;
  hole?: number | null;
  varyColors?: boolean;
  series?: ({ index: number } & Partial<Record<keyof ChartSeries, unknown>>)[];
  /** Apply a look from the Style gallery (1 to 8). */
  look?: number;
  /** Apply a palette from the Change colors gallery. */
  palette?: string;
}

/** Insert a chart at `at`. Size in points (default 432 x 252). `legend` is `none`, `right`, `left`, `top`, `bottom` or `top_right`. */
export interface InsertChartOp {
  op: "insert_chart";
  at: MPos;
  kind: Exclude<ChartKind, "other">;
  title?: string;
  categories?: string[];
  series: ChartSeriesInput[];
  grouping?: ChartGrouping;
  legend?: "none" | "right" | "left" | "top" | "bottom" | "top_right";
  data_labels?: boolean;
  width?: number;
  height?: number;
  alt?: string;
  options?: ChartOptions;
}

/**
 * Change the chart at `at`. Pass only what changes; `categories` and `series` replace all the data.
 * `title: ""` removes the title and `legend: "none"` the legend. The chart's workbook is rewritten to match.
 */
export interface SetChartOp {
  op: "set_chart";
  at: MPos;
  kind?: Exclude<ChartKind, "other">;
  grouping?: ChartGrouping;
  title?: string;
  categories?: string[];
  series?: ChartSeriesInput[];
  legend?: "none" | "right" | "left" | "top" | "bottom" | "top_right";
  data_labels?: boolean;
  x_title?: string;
  y_title?: string;
  options?: ChartOptions;
}
