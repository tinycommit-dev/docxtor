// The Fill section of the Format shape panel: no fill, a solid colour, a gradient (a bar of stops), a picture (stretched or
// tiled) or a pattern, each with its own controls, and a transparency. Every change is one `set_shape` op carrying the whole
// fill, so the file always holds a complete fill.
import type { FillSpec, RFill, RShape, RStop } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import { isLineShape, toAlpha, toTransparency, type SectionCtx, type ShapeSection } from "./format-shape-ctx";
import { colorAt, gradientBar } from "./gradient-editor";
import { h } from "./dom";
import { actionButton, checkField, colorField, numberField, radioField, row, section, selectField, sliderField } from "./panel-controls";
import { patternGrid } from "./pattern-grid";

type Kind = "none" | "solid" | "gradient" | "picture" | "pattern";
type GradientSpec = Extract<FillSpec, { kind: "gradient" }>;
type PictureSpec = Extract<FillSpec, { kind: "picture" }>;

const DEFAULT_COLOR = "4472C4";
/** Where a tiled picture starts, as Word names the nine places. */
const alignOptions = (): [string, string][] => [
  ["tl", t("Top left")], ["t", t("Top")], ["tr", t("Top right")],
  ["l", t("Left")], ["ctr", t("Center")], ["r", t("Right")],
  ["bl", t("Bottom left")], ["b", t("Bottom")], ["br", t("Bottom right")],
];

const kindOf = (s: RShape): Kind => s.fill?.kind ?? "none";

/** A fill's stops, sorted: a gradient the file gives two colours and no list still has two stops. */
export function stopsOf(f: RFill): RStop[] {
  const list = f.stops?.length ? f.stops : [{ pos: 0, color: f.color, alpha: f.alpha }, { pos: 100, color: f.color2 ?? f.color }];
  return [...list].sort((a, b) => a.pos - b.pos);
}

const stopIn = (s: RStop) => ({ pos: s.pos, color: s.color, ...(s.alpha !== undefined && s.alpha < 1 ? { alpha: s.alpha } : {}) });

function gradientSpec(f: RFill): GradientSpec {
  return {
    kind: "gradient",
    stops: stopsOf(f).map(stopIn),
    ...(f.path ? { path: f.path, ...(f.fillTo ? { fill_to: f.fillTo } : {}) } : { angle: f.angle ?? 0 }),
    ...(f.scaled !== undefined ? { scaled: f.scaled } : {}),
    ...(f.rotWithShape !== undefined ? { rot_with_shape: f.rotWithShape } : {}),
  };
}

function pictureSpec(f: RFill): PictureSpec {
  return {
    kind: "picture",
    media: f.src ?? "",
    ...(f.tile ? { tile: { ...f.tile } } : { stretch: f.stretch ?? { l: 0, t: 0, r: 0, b: 0 } }),
    ...(f.crop ? { crop: f.crop } : {}),
    ...(f.alpha !== undefined && f.alpha < 1 ? { alpha: f.alpha } : {}),
  };
}

/** A lighter tint of a colour, for the second stop of a new gradient. */
function tint(hex: string, k: number): string {
  const c = (i: number) => {
    const v = parseInt(hex.slice(i, i + 2), 16) || 0;
    return Math.round(v + (255 - v) * k)
      .toString(16)
      .padStart(2, "0");
  };
  return (c(0) + c(2) + c(4)).toUpperCase();
}

export function fillSection(ctx: SectionCtx): ShapeSection {
  const sec = section("fill", t("Fill"), true);
  let shape: RShape | null = null;
  /** A kind the person chose that the file can't hold yet (a picture fill needs a picture). */
  let pending: Kind | null = null;
  let pendingFrom: Kind = "none";
  let selStop = 0;

  const fill = () => shape?.fill;
  const solid = (color: string, transparency: number): FillSpec => ({ kind: "solid", color, ...(toAlpha(transparency) !== undefined ? { alpha: toAlpha(transparency) } : {}) });

  // -- the kind ----------------------------------------------------------------------------
  const kinds = radioField<Kind>({
    label: t("Fill type"),
    columns: 2,
    options: [
      ["none", t("No fill")],
      ["solid", t("Solid fill")],
      ["gradient", t("Gradient fill")],
      ["picture", t("Picture fill")],
      ["pattern", t("Pattern fill")],
    ],
    commit(k) {
      if (!shape) return;
      const f = fill();
      const base = f && f.kind !== "picture" ? f.color : DEFAULT_COLOR;
      pending = null;
      if (k === kindOf(shape)) return update(shape);
      if (k === "none") ctx.patch({ fill: "none" });
      else if (k === "solid") ctx.patch({ fill: solid(base, 0) });
      else if (k === "gradient") ctx.patch({ fill: { kind: "gradient", stops: [{ pos: 0, color: base }, { pos: 100, color: tint(base, 0.65) }], angle: 90, rot_with_shape: true } satisfies GradientSpec });
      else if (k === "pattern") ctx.patch({ fill: { kind: "pattern", pattern: "pct25", fg: base, bg: "FFFFFF" } });
      else {
        pending = "picture";
        pendingFrom = kindOf(shape);
        update(shape);
      }
    },
  });

  // -- solid -------------------------------------------------------------------------------
  const solidColor = colorField({ label: t("Color"), commit: (c) => c && ctx.patch({ fill: solid(c, toTransparency(fill()?.alpha)) }) });
  const solidAlpha = sliderField({
    label: t("Transparency"),
    unit: "%",
    min: 0,
    max: 100,
    live: (v, first) => fill() && ctx.patch({ fill: solid(fill()!.color, v) }, { merge: !first }),
    commit: (v) => fill() && ctx.patch({ fill: solid(fill()!.color, v) }),
  });
  const solidBox = h("div", { class: "fs-box" }, solidColor.el, solidAlpha.el);

  // -- gradient ----------------------------------------------------------------------------
  const editGradient = (edit: (g: GradientSpec) => GradientSpec, opts?: { merge?: boolean }) => {
    const f = fill();
    if (f?.kind === "gradient") ctx.patch({ fill: edit(gradientSpec(f)) }, opts);
  };
  const stops = () => (fill() ? stopsOf(fill()!) : []);
  const withStop = (g: GradientSpec, i: number, change: Partial<RStop>): GradientSpec => ({ ...g, stops: g.stops.map((s, j) => (j === i ? stopIn({ ...s, ...change } as RStop) : s)) });

  const bar = gradientBar({
    onSelect(i) {
      selStop = i;
      showStop();
    },
    onMove(sorted, index) {
      selStop = index;
      editGradient((g) => ({ ...g, stops: sorted.map(stopIn) }));
    },
    onAdd(pos) {
      const list = stops();
      const added = colorAt(list, pos);
      const next = [...list, added].sort((a, b) => a.pos - b.pos);
      selStop = next.indexOf(added);
      editGradient((g) => ({ ...g, stops: next.map(stopIn) }));
    },
    onRemove(i) {
      if (stops().length <= 2) return;
      selStop = Math.max(0, i - 1);
      editGradient((g) => ({ ...g, stops: g.stops.filter((_, j) => j !== i) }));
    },
  });
  const gradType = selectField<string>({
    label: t("Type"),
    options: [
      ["linear", t("Linear")],
      ["circle", t("Radial")],
      ["rect", t("Rectangular")],
      ["shape", t("Path")],
    ],
    commit(v) {
      editGradient((g) => {
        const { path: _p, fill_to: _f, ...rest } = g;
        return v === "linear" ? { ...rest, angle: fill()?.angle ?? 90 } : { ...rest, angle: undefined, path: v as "circle" | "rect" | "shape", fill_to: fill()?.fillTo ?? { l: 50, t: 50, r: 50, b: 50 } };
      });
    },
  });
  const gradAngle = numberField({ label: t("Angle"), unit: "°", min: 0, max: 359, commit: (v) => editGradient((g) => ({ ...g, angle: v })) });
  const addStop = actionButton(t("Add stop"), () => {
    const list = stops();
    // In the widest gap between two stops.
    let at = 50;
    let best = -1;
    for (let i = 0; i + 1 < list.length; i++) {
      if (list[i + 1].pos - list[i].pos > best) {
        best = list[i + 1].pos - list[i].pos;
        at = Math.round((list[i].pos + list[i + 1].pos) / 2);
      }
    }
    const added = colorAt(list, at);
    const next = [...list, added].sort((a, b) => a.pos - b.pos);
    selStop = next.indexOf(added);
    editGradient((g) => ({ ...g, stops: next.map(stopIn) }));
  });
  const removeStop = actionButton(t("Remove stop"), () => {
    if (stops().length > 2) {
      const i = selStop;
      selStop = Math.max(0, i - 1);
      editGradient((g) => ({ ...g, stops: g.stops.filter((_, j) => j !== i) }));
    }
  });
  const stopPos = numberField({ label: t("Stop position"), unit: "%", min: 0, max: 100, commit: (v) => editGradient((g) => ({ ...g, stops: withStop(g, selStop, { pos: v }).stops.slice().sort((a, b) => a.pos - b.pos) })) });
  const stopColor = colorField({ label: t("Stop color"), commit: (c) => c && editGradient((g) => withStop(g, selStop, { color: c })) });
  const stopAlpha = sliderField({
    label: t("Stop transparency"),
    unit: "%",
    min: 0,
    max: 100,
    live: (v, first) => editGradient((g) => withStop(g, selStop, { alpha: toAlpha(v) ?? 1 }), { merge: !first }),
    commit: (v) => editGradient((g) => withStop(g, selStop, { alpha: toAlpha(v) ?? 1 })),
  });
  const rotate = checkField({ label: t("Rotate with shape"), commit: (on) => editGradient((g) => ({ ...g, rot_with_shape: on })) });
  const gradBox = h("div", { class: "fs-box" }, gradType.el, gradAngle.el, bar.el, h("div", { class: "fs-actions" }, addStop, removeStop), stopPos.el, stopColor.el, stopAlpha.el, rotate.el);

  const showStop = () => {
    const list = stops();
    if (!list.length) return;
    selStop = Math.min(selStop, list.length - 1);
    const s = list[selStop];
    bar.set(list, selStop);
    stopPos.set(s.pos);
    stopColor.set(s.color);
    stopAlpha.set(toTransparency(s.alpha));
    removeStop.disabled = list.length <= 2;
  };

  // -- picture -----------------------------------------------------------------------------
  const preview = h("img", { class: "fs-thumb", alt: t("The picture in the fill"), hidden: true });
  const file = h("input", { type: "file", accept: "image/png,image/jpeg,image/gif,image/webp,image/svg+xml", hidden: true, "aria-label": t("Picture file") });
  file.addEventListener("change", async () => {
    const chosen = file.files?.[0];
    file.value = "";
    if (!chosen || !shape) return;
    const ext = (chosen.name.split(".").pop() ?? "png").toLowerCase().replace("jpeg", "jpg");
    const media = ctx.engine.addMedia(ext, new Uint8Array(await chosen.arrayBuffer()));
    pending = null;
    const keep = fill()?.kind === "picture" ? pictureSpec(fill()!) : null;
    ctx.patch({ fill: { ...(keep ?? { stretch: { l: 0, t: 0, r: 0, b: 0 } }), kind: "picture", media } satisfies PictureSpec });
  });
  const choose = actionButton(t("Insert picture…"), () => file.click());
  const editPicture = (edit: (p: PictureSpec) => PictureSpec, opts?: { merge?: boolean }) => {
    const f = fill();
    if (f?.kind === "picture" && f.src) ctx.patch({ fill: edit(pictureSpec(f)) }, opts);
  };
  const mode = radioField<"stretch" | "tile">({
    label: t("Picture placement"),
    options: [
      ["stretch", t("Stretch picture to fill")],
      ["tile", t("Tile picture as texture")],
    ],
    commit(v) {
      editPicture((p) => {
        const { tile: _t, stretch: _s, ...rest } = p;
        return v === "tile" ? { ...rest, tile: { tx: 0, ty: 0, sx: 100, sy: 100, align: "tl", flip: "none" } } : { ...rest, stretch: { l: 0, t: 0, r: 0, b: 0 } };
      });
    },
  });
  const edge = (key: "l" | "t" | "r" | "b", label: string) =>
    numberField({ label, unit: "%", step: 1, digits: 1, commit: (v) => editPicture((p) => ({ ...p, stretch: { ...(p.stretch ?? { l: 0, t: 0, r: 0, b: 0 }), [key]: v } })) });
  const offsets = { l: edge("l", t("Offset left")), r: edge("r", t("Offset right")), t: edge("t", t("Offset top")), b: edge("b", t("Offset bottom")) };
  const stretchBox = h("div", {}, offsets.l.el, offsets.r.el, offsets.t.el, offsets.b.el);
  const tileNum = (key: "tx" | "ty" | "sx" | "sy", label: string, unit: string) =>
    numberField({ label, unit, step: key[0] === "t" ? 1 : 5, digits: 1, commit: (v) => editPicture((p) => ({ ...p, tile: { ...(p.tile ?? {}), [key]: v } })) });
  const tiles = { tx: tileNum("tx", t("Offset x"), "pt"), ty: tileNum("ty", t("Offset y"), "pt"), sx: tileNum("sx", t("Scale x"), "%"), sy: tileNum("sy", t("Scale y"), "%") };
  const tileAlign = selectField<string>({ label: t("Alignment"), options: alignOptions(), commit: (v) => editPicture((p) => ({ ...p, tile: { ...(p.tile ?? {}), align: v } })) });
  const tileFlip = selectField<string>({
    label: t("Mirror type"),
    options: [
      ["none", t("None")],
      ["x", t("Horizontal")],
      ["y", t("Vertical")],
      ["xy", t("Both")],
    ],
    commit: (v) => editPicture((p) => ({ ...p, tile: { ...(p.tile ?? {}), flip: v as "none" | "x" | "y" | "xy" } })),
  });
  const tileBox = h("div", {}, tiles.tx.el, tiles.ty.el, tiles.sx.el, tiles.sy.el, tileAlign.el, tileFlip.el);
  const picAlpha = sliderField({
    label: t("Transparency"),
    unit: "%",
    min: 0,
    max: 100,
    live: (v, first) => editPicture((p) => ({ ...p, alpha: toAlpha(v) }), { merge: !first }),
    commit: (v) => editPicture((p) => ({ ...p, alpha: toAlpha(v) })),
  });
  const picBox = h("div", { class: "fs-box" }, row(t("Picture"), null, preview, choose), file, mode.el, stretchBox, tileBox, picAlpha.el);

  // -- pattern -----------------------------------------------------------------------------
  const editPattern = (change: { pattern?: string; fg?: string; bg?: string }) => {
    const f = fill();
    if (f?.kind !== "pattern") return;
    ctx.patch({ fill: { kind: "pattern", pattern: change.pattern ?? f.pattern ?? "pct25", fg: change.fg ?? f.color, bg: change.bg ?? f.color2 ?? "FFFFFF", ...(f.alpha !== undefined && f.alpha < 1 ? { alpha: f.alpha } : {}) } });
  };
  const grid = patternGrid((p) => editPattern({ pattern: p }));
  const fg = colorField({ label: t("Foreground color"), commit: (c) => c && editPattern({ fg: c }) });
  const bg = colorField({ label: t("Background color"), commit: (c) => c && editPattern({ bg: c }) });
  const patBox = h("div", { class: "fs-box" }, grid.el, fg.el, bg.el);

  sec.body.append(kinds.el, solidBox, gradBox, picBox, patBox);

  function update(s: RShape) {
    shape = s;
    const f = s.fill;
    // The picture chosen but not yet inserted stays on screen until the shape's fill changes or the person picks another kind.
    if (pending && kindOf(s) !== pendingFrom) pending = null;
    const kind: Kind = pending ?? kindOf(s);
    kinds.set(kind);
    solidBox.hidden = kind !== "solid";
    gradBox.hidden = kind !== "gradient";
    picBox.hidden = kind !== "picture";
    patBox.hidden = kind !== "pattern";
    if (f?.kind === "solid") {
      solidColor.set(f.color);
      solidAlpha.set(toTransparency(f.alpha));
    }
    if (f?.kind === "gradient") {
      gradType.set(f.path ?? "linear");
      gradAngle.set(f.angle ?? 0);
      gradAngle.disable(!!f.path);
      rotate.set(f.rotWithShape);
      showStop();
    }
    const hasPicture = f?.kind === "picture" && !!f.src;
    mode.el.hidden = stretchBox.hidden = tileBox.hidden = picAlpha.el.hidden = !hasPicture;
    if (f?.kind === "picture" && f.src) {
      preview.hidden = false;
      preview.src = ctx.engine.mediaUrl(f.src);
      mode.set(f.tile ? "tile" : "stretch");
      stretchBox.hidden = !!f.tile;
      tileBox.hidden = !f.tile;
      const r = f.stretch ?? { l: 0, t: 0, r: 0, b: 0 };
      offsets.l.set(r.l);
      offsets.r.set(r.r);
      offsets.t.set(r.t);
      offsets.b.set(r.b);
      if (f.tile) {
        tiles.tx.set(f.tile.tx);
        tiles.ty.set(f.tile.ty);
        tiles.sx.set(f.tile.sx);
        tiles.sy.set(f.tile.sy);
        tileAlign.set(f.tile.align);
        tileFlip.set(f.tile.flip);
      }
      picAlpha.set(toTransparency(f.alpha));
    } else preview.hidden = true;
    if (f?.kind === "pattern") {
      const bgc = f.color2 ?? "FFFFFF";
      grid.set(f.pattern, f.color, bgc);
      fg.set(f.color);
      bg.set(bgc);
    }
  }

  return { sec, applies: (s) => s.kind === "shape" && !isLineShape(s), update };
}
