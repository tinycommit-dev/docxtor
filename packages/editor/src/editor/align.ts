// Paragraph alignment as people see it, and as Word's file says it. In a file `left` and `start` mean the start
// of the line and `right` and `end` the end of it, so in a right-to-left paragraph `left` is on the right side.
// The toolbar and the Paragraph dialog speak in what is on screen; the file gets the other word where the
// paragraph runs right to left.

export type VisualAlign = "left" | "center" | "right" | "both" | "distribute";

/** The side a file's alignment puts the text on (`both` and `distribute` fill the line). */
export function visualAlign(jc: string | undefined, rtl?: boolean): VisualAlign {
  const start = jc === undefined || jc === "left" || jc === "start";
  const end = jc === "right" || jc === "end";
  if (jc === "center") return "center";
  if (jc === "both" || jc === "justify") return "both";
  if (jc === "distribute") return "distribute";
  if (start) return rtl ? "right" : "left";
  if (end) return rtl ? "left" : "right";
  return "left";
}

/** The alignment to write for text that should sit on this side (or fill the line) in a paragraph of this direction. */
export function fileAlign(v: VisualAlign, rtl?: boolean): string {
  if (v === "left") return rtl ? "right" : "left";
  if (v === "right") return rtl ? "left" : "right";
  return v;
}
