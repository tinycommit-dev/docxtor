// Format, Position and layout for a picture: where it sits as numbers (from the page, the margins, the column, the
// paragraph or the character), whether it moves with the text, how the text wraps around it and how far it keeps
// away. The same things Word's Layout dialog has.
import type { MPos } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { horizontalRef, verticalRef } from "../editor/object-move";
import { h, modal } from "./dom";
import type { ImageTool } from "./image-tool";

const H_REFS: [string, string][] = [
  ["margin", msg("Margin")],
  ["page", msg("Page")],
  ["column", msg("Column")],
  ["character", msg("Character")],
  ["leftMargin", msg("Left margin")],
  ["rightMargin", msg("Right margin")],
  ["insideMargin", msg("Inside margin")],
  ["outsideMargin", msg("Outside margin")],
];
const V_REFS: [string, string][] = [
  ["paragraph", msg("Paragraph")],
  ["line", msg("Line")],
  ["page", msg("Page")],
  ["margin", msg("Margin")],
  ["topMargin", msg("Top margin")],
  ["bottomMargin", msg("Bottom margin")],
  ["insideMargin", msg("Inside margin")],
  ["outsideMargin", msg("Outside margin")],
];
const H_ALIGNS: [string, string][] = [
  ["left", msg("Left")],
  ["center", msg("Centered")],
  ["right", msg("Right")],
  ["inside", msg("Inside")],
  ["outside", msg("Outside")],
];
const V_ALIGNS: [string, string][] = [
  ["top", msg("Top")],
  ["center", msg("Centered")],
  ["bottom", msg("Bottom")],
  ["inside", msg("Inside")],
  ["outside", msg("Outside")],
];
const WRAPS: [string, string][] = [
  ["inline", msg("In line with text")],
  ["square", msg("Square")],
  ["tight", msg("Tight")],
  ["through", msg("Through")],
  ["topBottom", msg("Top and bottom")],
  ["behind", msg("Behind text")],
  ["front", msg("In front of text")],
];
const SIDES: [string, string][] = [
  ["bothSides", msg("Both sides")],
  ["left", msg("Left only")],
  ["right", msg("Right only")],
  ["largest", msg("Largest side only")],
];

const r1 = (n: number) => Math.round(n * 10) / 10;

export function openPicturePosition(_ed: Editor, tool: ImageTool, _at: MPos) {
  const im = tool.info();
  if (!im) return;
  const place = im.place;
  const startWrap = im.wrap === "tight" && place?.through ? "through" : im.wrap;
  let wrap = startWrap;
  let sides: string = place?.wrapText ?? "bothSides";
  const dist0: [number, number, number, number] = place?.dist ?? [0, 0, 9, 9];
  const dist = [...dist0] as [number, number, number, number];
  let relX = im.relX ?? "column";
  let relY = im.relY ?? "paragraph";
  let hMode: "align" | "offset" = place?.alignH ? "align" : "offset";
  let hAlign = place?.alignH ?? im.align ?? "left";
  let vMode: "align" | "offset" = im.alignY ? "align" : "offset";
  let vAlign = im.alignY ?? "top";
  const here = tool.offsetsIn(horizontalRef(relX), verticalRef(relY));
  let x = im.x ?? here?.x ?? 0;
  let y = im.y ?? here?.y ?? 0;
  let lock = !!place?.locked;
  let overlap = place?.overlap ?? true;
  const start = { relX, relY, hMode, hAlign, vMode, vAlign, x, y, lock, overlap, sides, wrap };

  const sync: Array<() => void> = [];
  const redraw = () => sync.forEach((fn) => fn());
  const seg = (items: [string, string][], get: () => string, set: (v: string) => void, label: string) => {
    const group = h("div", { class: "seg pp-seg", role: "radiogroup", "aria-label": label });
    for (const [v, text] of items) {
      const b = h("button", { type: "button", role: "radio" }, t(text));
      b.addEventListener("click", () => {
        set(v);
        redraw();
      });
      sync.push(() => {
        b.classList.toggle("on", get() === v);
        b.setAttribute("aria-checked", String(get() === v));
      });
      group.append(b);
    }
    return group;
  };
  const select = (items: [string, string][], get: () => string, set: (v: string) => void, label: string) => {
    const el = h("select", { "aria-label": label }, ...items.map(([v, text]) => h("option", { value: v }, t(text))));
    el.addEventListener("change", () => {
      set(el.value);
      redraw();
    });
    sync.push(() => (el.value = get()));
    return el;
  };
  const number = (get: () => number, set: (v: number) => void, label: string, step = "0.1") => {
    const el = h("input", { type: "number", step, value: String(r1(get())), "aria-label": label, class: "pp-num" });
    el.addEventListener("change", () => {
      const v = parseFloat(el.value);
      if (Number.isFinite(v)) set(v);
      redraw();
    });
    sync.push(() => {
      if (document.activeElement !== el) el.value = String(r1(get()));
    });
    return el;
  };
  const check = (label: string, get: () => boolean, set: (v: boolean) => void) => {
    const el = h("input", { type: "checkbox" });
    el.addEventListener("change", () => {
      set(el.checked);
      redraw();
    });
    sync.push(() => (el.checked = get()));
    return h("label", { class: "pp-check" }, el, h("span", {}, label));
  };

  /** The picture stays where it is on the page when what its position counts from changes. */
  const remeasure = (axis: "h" | "v") => {
    const o = tool.offsetsIn(horizontalRef(relX), verticalRef(relY));
    if (!o) return;
    if (axis === "h" && hMode === "offset") x = o.x;
    if (axis === "v" && vMode === "offset") y = o.y;
  };

  // -- position ---------------------------------------------------------------------------------
  const horizontal = h(
    "fieldset",
    { class: "pp-group" },
    h("legend", {}, t("Horizontal")),
    seg([["align", t("Alignment")], ["offset", t("Position")]], () => hMode, (v) => ((hMode = v as "align" | "offset"), remeasure("h")), t("Horizontal mode")),
    h("div", { class: "pp-row" }, select(H_ALIGNS, () => hAlign, (v) => (hAlign = v), t("Horizontal alignment")), h("span", { class: "pp-word" }, t("relative to"))),
    h("div", { class: "pp-row" }, number(() => x, (v) => (x = v), t("Horizontal position (points)")), h("span", { class: "pp-unit" }, t("pt from")), select(H_REFS, () => relX, (v) => ((relX = v), remeasure("h")), t("Horizontal position is relative to"))),
  );
  const vertical = h(
    "fieldset",
    { class: "pp-group" },
    h("legend", {}, t("Vertical")),
    seg([["align", t("Alignment")], ["offset", t("Position")]], () => vMode, (v) => ((vMode = v as "align" | "offset"), remeasure("v")), t("Vertical mode")),
    h("div", { class: "pp-row" }, select(V_ALIGNS, () => vAlign, (v) => (vAlign = v), t("Vertical alignment")), h("span", { class: "pp-word" }, t("relative to"))),
    h("div", { class: "pp-row" }, number(() => y, (v) => (y = v), t("Vertical position (points)")), h("span", { class: "pp-unit" }, t("pt below")), select(V_REFS, () => relY, (v) => ((relY = v), remeasure("v")), t("Vertical position is relative to"))),
  );
  const moveWith = check(
    t("Move object with text"),
    () => relY === "paragraph" || relY === "line",
    (on) => {
      relY = on ? "paragraph" : "page";
      remeasure("v");
    },
  );
  const lockBox = check(t("Lock anchor"), () => lock, (v) => (lock = v));
  const overlapBox = check(t("Allow overlap"), () => overlap, (v) => (overlap = v));
  const positionPanel = h("div", { class: "pp-panel" }, horizontal, vertical, h("div", { class: "pp-options" }, moveWith, lockBox, overlapBox));
  const inlineNote = h("p", { class: "help pp-note" }, t("This picture is in line with the text, so it has no position of its own. Choose a wrapping style on the Text wrapping tab first."));

  // -- wrapping ---------------------------------------------------------------------------------
  const wrapSeg = seg(WRAPS, () => wrap, (v) => (wrap = v), t("Wrapping style"));
  const sidesSeg = seg(SIDES, () => sides, (v) => (sides = v), t("Wrap text"));
  const distances = h(
    "div",
    { class: "pp-dist" },
    ...(
      [
        [0, t("Top")],
        [1, t("Bottom")],
        [2, t("Left")],
        [3, t("Right")],
      ] as const
    ).map(([i, label]) => h("div", { class: "field", style: "margin:0" }, h("label", {}, label), number(() => dist[i], (v) => (dist[i] = Math.max(0, v)), t("{side} distance from text (points)", { side: label }), "0.5"))),
  );
  const tightNote = h("p", { class: "help" }, t("Tight and through wrapping follow the picture's box here; Word follows its outline."));
  const wrapPanel = h("div", { class: "pp-panel" }, h("div", { class: "field" }, h("label", {}, t("Wrapping style")), wrapSeg), h("div", { class: "field", "data-for": "sides" }, h("label", {}, t("Wrap text")), sidesSeg), h("div", { class: "field" }, h("label", {}, t("Distance from text (points)")), distances), tightNote);

  // -- tabs -------------------------------------------------------------------------------------
  let tab: "position" | "wrap" = wrap === "inline" ? "wrap" : "position";
  const tabs = seg([["position", t("Position")], ["wrap", t("Text wrapping")]], () => tab, (v) => (tab = v as "position" | "wrap"), t("Section"));
  const error = h("p", { class: "pf-error", role: "alert" });
  sync.push(() => {
    const floating = wrap !== "inline";
    positionPanel.hidden = tab !== "position" || !floating;
    inlineNote.hidden = tab !== "position" || floating;
    wrapPanel.hidden = tab !== "wrap";
    const sidesOn = wrap === "square" || wrap === "tight" || wrap === "through";
    (wrapPanel.querySelector('[data-for="sides"]') as HTMLElement).hidden = !sidesOn;
    tightNote.hidden = wrap !== "tight" && wrap !== "through";
    distances.querySelectorAll<HTMLInputElement>("input").forEach((el, i) => {
      el.disabled = !(wrap === "square" || wrap === "tight" || wrap === "through" || (wrap === "topBottom" && i < 2));
    });
    // A vertical alignment makes the number meaningless, and the other way round.
    horizontal.querySelectorAll<HTMLElement>(".pp-row")[0].classList.toggle("off", hMode !== "align");
    horizontal.querySelectorAll<HTMLElement>(".pp-row")[1].classList.toggle("off", hMode !== "offset");
    vertical.querySelectorAll<HTMLElement>(".pp-row")[0].classList.toggle("off", vMode !== "align");
    vertical.querySelectorAll<HTMLElement>(".pp-row")[1].classList.toggle("off", vMode !== "offset");
    const lockable = wrap !== "inline";
    lockBox.querySelector("input")!.disabled = !lockable;
    overlapBox.querySelector("input")!.disabled = !lockable;
  });

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog pp-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Position and layout") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Position and layout"))),
    h("div", { class: "dialog-body" }, tabs, positionPanel, inlineNote, wrapPanel, error),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  redraw();
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    const patch: Record<string, unknown> = {};
    if (wrap !== start.wrap) patch.wrap = wrap;
    if (wrap !== "inline") {
      if (sides !== start.sides && (wrap === "square" || wrap === "tight" || wrap === "through")) patch.wrap_text = sides;
      if (dist.some((v, i) => v !== dist0[i])) patch.dist = dist;
      if (lock !== start.lock) patch.lock_anchor = lock;
      if (overlap !== start.overlap) patch.allow_overlap = overlap;
      const hChanged = relX !== start.relX || hMode !== start.hMode || (hMode === "align" ? hAlign !== start.hAlign : x !== start.x);
      const vChanged = relY !== start.relY || vMode !== start.vMode || (vMode === "align" ? vAlign !== start.vAlign : y !== start.y);
      if (hChanged) {
        patch.rel_x = relX;
        if (hMode === "align") patch.align = hAlign;
        else patch.x = x;
      }
      if (vChanged) {
        patch.rel_y = relY;
        if (vMode === "align") patch.align_y = vAlign;
        else patch.y = y;
      }
    }
    if (!Object.keys(patch).length) return close();
    error.textContent = "";
    if (tool.apply(patch, { onError: (m) => (error.textContent = m) })) close();
  });
}

