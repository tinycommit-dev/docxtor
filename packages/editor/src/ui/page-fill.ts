// Page color (Format, Page color…): a plain colour, or a fill effect: a gradient, a texture, a pattern or a picture from a
// file. The effects are written the way Word writes them (a VML `v:background` with a `v:fill`, and an image where the
// fill needs one), so the file opens with the same page.
import type { PageFill } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import { pageFillCss } from "../editor/page-art";
import type { Editor } from "../editor/editor";
import { PATTERNS, patternCanvas, pngBytes, TEXTURES } from "./fill-tiles";
import { h, modal } from "./dom";
import { field, selectOf, tabs } from "./form";
import { swatchGrid } from "./swatches";

/** Pale tints that keep black text easy to read, then a few stronger ones and greys. No colour means white. */
export const PAGE_COLORS = [
  "FFF9E6", "FDF0E0", "FCE4E4", "F6E6F7", "E8EBFA", "E3F1FC", "DFF5F3", "E4F4E4", "FFFBD6", "F2EFEA",
  "FFE08A", "F7B6C2", "C9B8F0", "9CCBF0", "A8DDB5", "000000", "434343", "999999", "D9D9D9", "FFFFFF",
];

type Style = "horizontal" | "vertical" | "diagonalDown" | "diagonalUp" | "center" | "corner";
const STYLES: [Style, string][] = [
  ["horizontal", msg("Horizontal")],
  ["vertical", msg("Vertical")],
  ["diagonalDown", msg("Diagonal down")],
  ["diagonalUp", msg("Diagonal up")],
  ["center", msg("From center")],
  ["corner", msg("From corner")],
];

/** VML degrees for each linear style: 0 changes from the top to the bottom, -90 from the left to the right. */
const ANGLE: Record<string, number> = { horizontal: 0, vertical: -90, diagonalDown: -45, diagonalUp: -135 };

/** A colour field: swatches and a hex box that follow each other. */
function colorPick(id: string, label: string, initial: string, change: () => void) {
  let value = initial.toUpperCase();
  const swatches = swatchGrid(PAGE_COLORS, value, (c) => set(c), (c) => `#${c}`, { more: true });
  const hex = h("input", { id, type: "text", value: `#${value}`, "aria-label": label, autocomplete: "off", spellcheck: "false", maxlength: "7", style: "width:100%;min-width:0" });
  const set = (c: string) => {
    value = c.toUpperCase();
    swatches.select(value);
    hex.value = `#${value}`;
    change();
  };
  hex.addEventListener("input", () => {
    const v = hex.value.replace(/^#/, "").toUpperCase();
    if (/^[0-9A-F]{6}$/.test(v)) {
      value = v;
      swatches.select(v);
      change();
    }
  });
  return {
    el: h("div", { class: "field" }, h("label", { for: id }, label), swatches.el, hex),
    get value() {
      return value;
    },
    set,
  };
}

export function openPageFill(ed: Editor) {
  const setup = ed.engine.pageSetup();
  const was: PageFill | undefined = setup.pageFill ?? (setup.pageColor ? { kind: "solid", color: setup.pageColor.replace(/^#/, "").toUpperCase() } : undefined);
  const url = (part: string) => ed.engine.mediaUrl(part);
  const firstTab = !was || was.kind === "solid" ? 0 : was.kind === "gradient" || was.kind === "radial" ? 1 : was.kind === "texture" ? 2 : was.kind === "pattern" ? 3 : 4;

  // What each tab would make.
  let solid: string | null = was?.kind === "solid" ? (was.color ?? null) : null;
  let texture: string | null = null;
  let pattern: string | null = null;
  let picture: { bytes: Uint8Array; ext: string; url: string; name: string } | null = null;
  let active = firstTab;

  const preview = h("div", { class: "ft-preview", role: "img", "aria-label": t("Preview of the page") });

  // Color
  const color = swatchGrid(PAGE_COLORS, solid, (c) => {
    solid = c;
    hex.value = `#${c}`;
    update();
  }, (c) => `#${c}`, { more: true });
  const none = h("button", { type: "button", "aria-pressed": "false" }, t("None"));
  none.addEventListener("click", () => {
    solid = null;
    hex.value = "";
    update();
  });
  const hex = h("input", { id: "pf-hex", type: "text", placeholder: "#RRGGBB", value: solid ? `#${solid}` : "", "aria-label": t("Custom color"), autocomplete: "off", spellcheck: "false", maxlength: "7" });
  hex.addEventListener("input", () => {
    const v = hex.value.replace(/^#/, "").toUpperCase();
    if (/^[0-9A-F]{6}$/.test(v)) {
      solid = v;
      update();
    }
  });
  const colorPane = h("div", {}, h("div", { class: "field" }, h("label", {}, t("Page color")), color.el, h("div", { class: "pb-hex-row" }, none, hex)));

  // Gradient
  const c1 = colorPick("pf-c1", t("Color 1"), was?.kind === "gradient" || was?.kind === "radial" ? (was.color ?? "FFFFFF") : "FFFFFF", () => update());
  const c2 = colorPick("pf-c2", t("Color 2"), was?.kind === "gradient" || was?.kind === "radial" ? (was.color2 ?? "9CCBF0") : "9CCBF0", () => update());
  const wasStyle: Style =
    was?.kind === "radial" ? (was.from && was.from !== "center" ? "corner" : "center") : was?.kind === "gradient" ? ((Object.entries(ANGLE).find(([, a]) => a === (was.angle ?? 0))?.[0] as Style | undefined) ?? "horizontal") : "horizontal";
  const style = selectOf("pf-style", STYLES.map(([v, l]) => [v, t(l)] as const), wasStyle);
  style.addEventListener("change", update);
  const swap = h("button", { class: "btn", type: "button" }, t("Swap colors"));
  swap.addEventListener("click", () => {
    const a = c1.value;
    c1.set(c2.value);
    c2.set(a);
  });
  const gradientPane = h("div", {}, c1.el, c2.el, h("div", { class: "bs-cols" }, field(t("Shading style"), style, "pf-style"), h("div", { class: "field" }, h("label", {}, " "), swap)));

  // Texture
  const tile = (id: string, name: string, src: string, pick: () => void, on: () => boolean) => {
    const b = h("button", { class: "ft-tile", type: "button", title: name, "aria-label": name, "aria-pressed": "false", style: `background-image:url(${src})` });
    b.addEventListener("click", pick);
    return { id, b, on };
  };
  const textureTiles = TEXTURES.map((x) => tile(x.id, t(x.name), x.draw().toDataURL("image/png"), () => ((texture = x.id), update()), () => texture === x.id));
  const texturePane = h("div", {}, h("div", { class: "field" }, h("label", {}, t("Texture")), h("div", { class: "ft-grid", role: "group", "aria-label": t("Texture") }, ...textureTiles.map((x) => x.b))));

  // Pattern
  const fore = colorPick("pf-fore", t("Foreground"), was?.kind === "pattern" ? (was.color ?? "000000") : "000000", () => update());
  const back = colorPick("pf-back", t("Background"), was?.kind === "pattern" ? (was.color2 ?? "FFFFFF") : "FFFFFF", () => update());
  const patternTiles = PATTERNS.map((x) => tile(x.id, t(x.name), patternCanvas(x, "000000", "FFFFFF").toDataURL("image/png"), () => ((pattern = x.id), update()), () => pattern === x.id));
  const patternPane = h("div", {}, h("div", { class: "field" }, h("label", {}, t("Pattern")), h("div", { class: "ft-grid", role: "group", "aria-label": t("Pattern") }, ...patternTiles.map((x) => x.b))), h("div", { class: "bs-cols" }, fore.el, back.el));

  // Picture
  const file = h("input", { id: "pf-file", type: "file", accept: "image/*", "aria-label": t("Choose a picture") });
  const fileName = h("span", { class: "help" }, was?.kind === "picture" ? t("The picture the page has now") : t("No picture chosen"));
  file.addEventListener("change", async () => {
    const f = file.files?.[0];
    if (!f) return;
    const bytes = new Uint8Array(await f.arrayBuffer());
    const ext = (f.name.split(".").pop() || f.type.split("/")[1] || "png").toLowerCase().replace("jpeg", "jpg");
    if (picture) URL.revokeObjectURL(picture.url);
    picture = { bytes, ext, url: URL.createObjectURL(f), name: f.name };
    fileName.textContent = f.name;
    update();
  });
  const picturePane = h("div", {}, h("div", { class: "field" }, h("label", { for: "pf-file" }, t("Picture")), file, fileName));

  let ready = false;
  const view = tabs("pf", [[t("Color"), colorPane], [t("Gradient"), gradientPane], [t("Texture"), texturePane], [t("Pattern"), patternPane], [t("Picture"), picturePane]], firstTab, (n) => {
    active = n;
    if (ready) update();
  });

  /** The fill the open tab would make, with no image yet (images are added when the dialog is applied). */
  const current = (): PageFill | null => {
    if (active === 0) return solid ? { kind: "solid", color: solid } : null;
    if (active === 1) {
      const s = style.value as Style;
      if (s === "center" || s === "corner") return { kind: "radial", color: c1.value, color2: c2.value, from: s === "center" ? "center" : "topLeft" };
      return { kind: "gradient", color: c1.value, color2: c2.value, angle: ANGLE[s] };
    }
    return null;
  };
  function update() {
    const fill = current();
    let css = "";
    if (active <= 1) css = fill ? pageFillCss(fill, undefined, url) : "";
    else if (active === 2 && texture) css = `background-image:url(${TEXTURES.find((x) => x.id === texture)!.draw().toDataURL("image/png")});background-repeat:repeat`;
    else if (active === 3 && pattern) css = `background-image:url(${patternCanvas(PATTERNS.find((x) => x.id === pattern)!, fore.value, back.value).toDataURL("image/png")});background-repeat:repeat;image-rendering:pixelated`;
    else if (active === 4 && picture) css = `background-image:url(${picture.url});background-size:100% 100%`;
    preview.setAttribute("style", css);
    color.select(solid);
    none.classList.toggle("on", !solid);
    none.setAttribute("aria-pressed", String(!solid));
    for (const x of [...textureTiles, ...patternTiles]) {
      x.b.classList.toggle("on", x.on());
      x.b.setAttribute("aria-pressed", String(x.on()));
    }
  }
  ready = true;
  update();

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog para-dialog pf-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Page color") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Page color"))),
    view.list,
    h("div", { class: "dialog-body" }, h("div", { class: "pf-row" }, h("div", { class: "pf-panes" }, ...view.panes), preview)),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  const close = modal(dialog);
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    let fill: PageFill | null = current();
    if (active === 2 && texture) {
      const x = TEXTURES.find((y) => y.id === texture)!;
      fill = { kind: "texture", color: "FFFFFF", media: ed.engine.addMedia("png", pngBytes(x.draw())), title: t(x.name) };
    } else if (active === 3 && pattern) {
      const x = PATTERNS.find((y) => y.id === pattern)!;
      fill = { kind: "pattern", color: fore.value, color2: back.value, media: ed.engine.addMedia("png", pngBytes(patternCanvas(x, fore.value, back.value))), title: t(x.name) };
    } else if (active === 4 && picture) {
      fill = { kind: "picture", color: "FFFFFF", media: ed.engine.addMedia(picture.ext, picture.bytes) };
    } else if (active >= 2) {
      // Nothing chosen on that tab: leave the page as it is.
      close();
      ed.focus();
      return;
    }
    close();
    ed.exec({ op: "set_page_background", fill });
    ed.reloadPageSetup();
    ed.focus();
  });
}
