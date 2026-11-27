// The new chart types Word 2016 added (waterfall, histogram, Pareto, box and whisker, funnel, treemap, sunburst,
// region map), as the engine reports them. The numbers a chart needs are already worked out (the bins of a
// histogram, the quartiles of a box, the running totals of a waterfall, the tree of a treemap), so the editor
// only lays them out. Docxtor doesn't edit these charts; Word does.

export type ChartExKind = "waterfall" | "histogram" | "pareto" | "boxWhisker" | "funnel" | "treemap" | "sunburst" | "regionMap" | "other";

export interface ChartExAxes {
  catTitle?: string;
  valTitle?: string;
  valMin?: number;
  valMax?: number;
  valMajor?: number;
  /** The value axis has major gridlines. */
  grid: boolean;
  catHidden?: boolean;
  valHidden?: boolean;
  /** Space between bars as a fraction of a bar's width. */
  gapWidth?: number;
}

export interface WaterfallPoint {
  label: string;
  value: number;
  /** Where the bar starts and ends on the value axis. */
  start: number;
  end: number;
  kind: "increase" | "decrease" | "total";
}

/** One bar of a histogram: a range of numbers, or a category. */
export interface HistogramBin {
  lo?: number;
  hi?: number;
  label?: string;
  count: number;
  /** Whether the range includes its ends: `[lo, hi]` or `(lo, hi]`. */
  closedLo: boolean;
  closedHi: boolean;
  kind: "bin" | "underflow" | "overflow" | "category";
}

export interface ChartExHistogram {
  bins: HistogramBin[];
  byCategory: boolean;
  /** Pareto: the running share of the total after each bar (0 to 1); the bars are sorted largest first. */
  cumulative?: number[];
  lineName?: string;
  lineColor?: string;
}

export interface BoxStat {
  n: number;
  q1: number;
  median: number;
  q3: number;
  whiskerLo: number;
  whiskerHi: number;
  mean: number;
  outliers: number[];
  /** The values inside the whiskers. */
  points: number[];
}

export interface BoxSeries {
  name: string;
  color?: string;
  /** One per category; null where the series has no values in it. */
  boxes: (BoxStat | null)[];
}

export interface ChartExBoxes {
  categories: string[];
  series: BoxSeries[];
  meanMarker: boolean;
  meanLine: boolean;
  outliers: boolean;
  /** Every value inside the whiskers is drawn as a point. */
  points: boolean;
  quartile: "exclusive" | "inclusive";
}

export interface ChartExPoint {
  label: string;
  value: number;
}

/** A node of a treemap's or sunburst's hierarchy; the root has no name. A parent's value is the sum of its children's. */
export interface TreeNode {
  name: string;
  value: number;
  children?: TreeNode[];
}

export interface ChartExModel {
  kind: ChartExKind;
  title?: string;
  legend?: { position: "r" | "l" | "t" | "b" };
  /** The theme's accent colours, in order, for points without a colour of their own. */
  accents?: string[];
  dataLabels: boolean;
  /** What a data label shows: the category's name, the value. */
  labelNames: boolean;
  labelValues: boolean;
  /** Excel number format of the labels and the value axis. */
  numberFormat?: string;
  axes: ChartExAxes;
  /** The first series' name and colour (hex without `#`). */
  name?: string;
  color?: string;
  /** Colours of single points, by index. */
  pointColors?: (string | null)[];
  waterfall?: { points: WaterfallPoint[]; connectors: boolean };
  histogram?: ChartExHistogram;
  boxes?: ChartExBoxes;
  funnel?: ChartExPoint[];
  tree?: TreeNode;
  /** Treemap: where the names of parents go. */
  parentLabels?: "banner" | "overlapping" | "none";
}
