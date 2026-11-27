// The Text box section of the Format shape panel: the space between the text and the edge, where the text sits, what happens
// when it doesn't fit, its direction, whether it wraps, and columns. Changes are `set_shape` ops with a `body` patch.
import type { BodyIn, RShape } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { type SectionCtx, type ShapeSection } from "./format-shape-ctx";
import { checkField, lengthField, numberField, radioField, section, selectField } from "./panel-controls";

export function textBoxSection(ctx: SectionCtx): ShapeSection {
  const sec = section("textbox", t("Text box"));
  const body = (b: BodyIn) => ctx.patch({ body: b });

  const anchor = selectField<string>({
    label: t("Vertical alignment"),
    options: [
      ["top", t("Top")],
      ["middle", t("Middle")],
      ["bottom", t("Bottom")],
    ],
    commit: (v) => body({ v_anchor: v as "top" | "middle" | "bottom" }),
  });
  const direction = selectField<string>({
    label: t("Text direction"),
    options: [
      ["horz", t("Horizontal")],
      ["vert", t("Rotate all text 90°")],
      ["vert270", t("Rotate all text 270°")],
      ["wordArtVert", t("Stacked")],
    ],
    commit: (v) => body({ vert: v as BodyIn["vert"] }),
  });
  const fit = radioField<"none" | "text" | "shape">({
    label: t("Autofit"),
    options: [
      ["none", t("Do not autofit")],
      ["text", t("Shrink text on overflow")],
      ["shape", t("Resize shape to fit text")],
    ],
    commit: (v) => body({ auto_fit: v }),
  });
  const wrap = checkField({ label: t("Wrap text in shape"), commit: (on) => body({ wrap_text: on }) });
  const margins = {
    l: lengthField({ label: t("Left margin"), min: 0, commit: (v) => body({ ins_l: v }) }),
    t: lengthField({ label: t("Top margin"), min: 0, commit: (v) => body({ ins_t: v }) }),
    r: lengthField({ label: t("Right margin"), min: 0, commit: (v) => body({ ins_r: v }) }),
    b: lengthField({ label: t("Bottom margin"), min: 0, commit: (v) => body({ ins_b: v }) }),
  };
  const columns = numberField({ label: t("Columns"), min: 1, max: 16, step: 1, digits: 0, commit: (v) => body({ columns: v }) });
  const gap = lengthField({ label: t("Column spacing"), min: 0, commit: (v) => body({ column_gap_pt: v }) });

  sec.body.append(anchor.el, direction.el, fit.el, wrap.el, margins.l.el, margins.t.el, margins.r.el, margins.b.el, columns.el, gap.el);

  function update(s: RShape) {
    anchor.set(s.vAnchor);
    direction.set(s.vert ?? "horz");
    fit.set(s.autoFitMode ?? (s.autoFit ? "shape" : "none"));
    wrap.set(s.wrapText);
    margins.l.set(s.insets.l);
    margins.t.set(s.insets.t);
    margins.r.set(s.insets.r);
    margins.b.set(s.insets.b);
    columns.set(s.columns ?? 1);
    gap.set(s.columnGapPt ?? 36);
    gap.disable((s.columns ?? 1) < 2);
  }

  return { sec, applies: (s) => s.kind === "shape" && !!s.text, update };
}
