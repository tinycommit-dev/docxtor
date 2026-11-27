// The Size and position section of the Format shape panel: width, height (with a lock on the aspect ratio), rotation, flip and
// turn buttons, and for a shape that floats its place on the page. Lengths are shown in the document's unit (units.ts).
import type { RShape } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { h } from "./dom";
import { type SectionCtx, type ShapeSection } from "./format-shape-ctx";
import { checkField, lengthField, numberField, row, section } from "./panel-controls";
import { SHAPE_ICONS } from "./shape-icons";

/** What a floating shape's offsets count from. */
const FROM_X: Record<string, string> = {
  column: msg("the column"), page: msg("the page"), margin: msg("the margins"), character: msg("the character"),
  leftMargin: msg("the left margin"), rightMargin: msg("the right margin"), insideMargin: msg("the inside margin"), outsideMargin: msg("the outside margin"),
};
const FROM_Y: Record<string, string> = {
  paragraph: msg("the paragraph"), page: msg("the page"), margin: msg("the margins"), line: msg("the line"),
  topMargin: msg("the top margin"), bottomMargin: msg("the bottom margin"), insideMargin: msg("the inside margin"), outsideMargin: msg("the outside margin"),
};

/** Whether the aspect ratio stays when one side changes; remembered for the session. */
let locked = false;

export function sizeSection(ctx: SectionCtx): ShapeSection {
  const sec = section("size", t("Size and position"));
  const cur = () => ctx.shape();

  const width = lengthField({
    label: t("Width"),
    min: 0,
    commit(w) {
      const s = cur();
      if (!s) return;
      ctx.patch(locked && s.w > 0 && s.h > 0 ? { w, h: Math.round(((w * s.h) / s.w) * 10) / 10 } : { w });
    },
  });
  const height = lengthField({
    label: t("Height"),
    min: 0,
    commit(hh) {
      const s = cur();
      if (!s) return;
      ctx.patch(locked && s.w > 0 && s.h > 0 ? { h: hh, w: Math.round(((hh * s.w) / s.h) * 10) / 10 } : { h: hh });
    },
  });
  const lock = checkField({
    label: t("Lock aspect ratio"),
    commit: (on) => {
      locked = on;
    },
  });
  const rotation = numberField({
    label: t("Rotation"),
    unit: "°",
    min: 0,
    max: 359,
    commit: (rot) => ctx.patch({ rot }),
  });

  const tool = (label: string, icon: string, run: () => void) => {
    const b = h("button", { type: "button", class: "tb", title: label, "aria-label": label });
    b.innerHTML = icon;
    b.addEventListener("click", run);
    return b;
  };
  const flips = row(t("Rotate and flip"), null, ...[
    tool(t("Rotate left 90°"), SHAPE_ICONS.rotateLeft, () => ctx.rotate(-90)),
    tool(t("Rotate right 90°"), SHAPE_ICONS.rotateRight, () => ctx.rotate(90)),
    tool(t("Flip horizontal"), SHAPE_ICONS.flipHorizontal, () => ctx.flip("h")),
    tool(t("Flip vertical"), SHAPE_ICONS.flipVertical, () => ctx.flip("v")),
  ]);

  const posX = lengthField({ label: t("Horizontal position"), commit: (x) => ctx.patch({ x }) });
  const posY = lengthField({ label: t("Vertical position"), commit: (y) => ctx.patch({ y }) });
  const from = h("p", { class: "fs-hint" });
  const position = h("div", {}, posX.el, posY.el, from);

  sec.body.append(width.el, height.el, lock.el, rotation.el, flips, position);

  function update(s: RShape) {
    width.set(s.w);
    height.set(s.h);
    lock.set(locked);
    rotation.set(Math.round((s.rot ?? 0) * 10) / 10);
    // A shape in line with the text has no place of its own to set.
    const member = s.childX !== undefined;
    position.hidden = !member && s.wrap === "inline";
    if (!position.hidden) {
      const at = ctx.measured();
      posX.set(s.x ?? at?.x);
      posY.set(s.y ?? at?.y);
      from.textContent = member
        ? t("Counted from the top left corner of the group.")
        : t("Counted from {horizontal} and {vertical}.", { horizontal: t(FROM_X[s.relX ?? "column"] ?? FROM_X.column), vertical: t(FROM_Y[s.relY ?? "paragraph"] ?? FROM_Y.paragraph) });
    }
  }

  return { sec, applies: () => true, update };
}
