// Insert, Page elements, Watermark: text behind every page (DRAFT, CONFIDENTIAL…), diagonal or
// horizontal, in a colour, font and transparency, or a picture with a scale and Word's washout, with a
// small page that shows how it will look.
import type { Op, Watermark, WatermarkLayout } from "@docxtor/core";
import { currentLocale, msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { tw } from "../editor/render";
import { watermarkElement } from "../editor/watermark";
import { MENU_FONTS } from "../fonts";
import { h, modal } from "./dom";
import { swatchGrid } from "./swatches";
import { TEXT_COLORS } from "./toolbar";

/** What the quick choices write: Word's own presets are in capitals. */
const QUICK = [msg("Draft"), msg("Confidential"), msg("Sample"), msg("Do not copy")];
const GREYS = ["EFEFEF", "D9D9D9", "C0C0C0", "A6A6A6", "808080", "595959", "404040", "000000"];
const COLORS = [...GREYS, ...TEXT_COLORS.slice(8)];
const DEFAULT_COLOR = "C0C0C0";
const SEMI = 0.5;
/** Picture scales: automatic fits the picture to the page, the rest are of its own size. */
const SCALES = [["auto", msg("Auto")], ["0.5", "50%"], ["1", "100%"], ["1.5", "150%"], ["2", "200%"]] as const;
const PT = 0.75;

/** A picture to use as the watermark: one already in the document, or a file the person chose. */
interface Picture {
  /** The part name of an image already in the document. */
  media?: string;
  file?: File;
  name: string;
  /** What the preview loads. */
  url: string;
  /** The picture's size in points at 100%. */
  width: number;
  height: number;
}

/** Load an image to learn its size in points at 100% (96 pixels to the inch). */
async function measure(url: string): Promise<{ width: number; height: number }> {
  const img = new Image();
  img.src = url;
  await img.decode().catch(() => undefined);
  return { width: (img.naturalWidth || 400) * PT, height: (img.naturalHeight || 300) * PT };
}

export function openWatermark(ed: Editor) {
  const old: Watermark | undefined = ed.engine.pageSetup().watermark;
  const isText = old?.kind === "text";
  let kind: "text" | "picture" = old?.kind === "picture" ? "picture" : "text";
  let picture: Picture | null = null;
  let pictureScale: string = "auto";
  let washout = old?.kind === "picture" ? old.washout !== false : true;
  let blobUrl = "";
  let text = isText ? old.text ?? "" : "";
  let layout: WatermarkLayout = old?.layout ?? "diagonal";
  let color = (old?.color ?? DEFAULT_COLOR).toUpperCase();
  let solid = old ? old.opacity === undefined || old.opacity > 0.75 : false;
  let font = isText ? old.font ?? "" : "";

  const radio = (items: [string, string][], get: () => string, set: (v: string) => void, label: string) => {
    const group = h("div", { class: "seg", role: "radiogroup", "aria-label": label });
    const buttons = items.map(([v, name]) => {
      const b = h("button", { type: "button", role: "radio" }, name);
      b.addEventListener("click", () => {
        set(v);
        refresh();
      });
      group.append(b);
      return [v, b] as const;
    });
    const paint = () => {
      for (const [v, b] of buttons) {
        b.classList.toggle("on", v === get());
        b.setAttribute("aria-checked", String(v === get()));
      }
    };
    return { el: group, paint };
  };

  const input = h("input", { type: "text", id: "wm-text", value: text, spellcheck: "false", autocomplete: "off", placeholder: t("Watermark text") });
  input.addEventListener("input", () => {
    text = input.value;
    refresh();
  });
  const quick = h("div", { class: "wm-quick" });
  for (const name of QUICK) {
    const shown = t(name);
    const b = h("button", { class: "btn", type: "button" }, shown);
    b.addEventListener("click", () => {
      text = shown.toLocaleUpperCase(currentLocale());
      input.value = text;
      refresh();
      input.focus();
    });
    quick.append(b);
  }

  const layouts = radio([["diagonal", t("Diagonal")], ["horizontal", t("Horizontal")]], () => layout, (v) => (layout = v as WatermarkLayout), t("Layout"));
  const weights = radio([["semi", t("Semi-transparent")], ["solid", t("Solid")]], () => (solid ? "solid" : "semi"), (v) => (solid = v === "solid"), t("Transparency"));
  const kinds = radio([["text", t("Text")], ["picture", t("Picture")]], () => kind, (v) => (kind = v as "text" | "picture"), t("Kind"));
  const scales = radio(SCALES.map(([v, name]) => [v, t(name)] as [string, string]), () => pictureScale, (v) => (pictureScale = v), t("Scale"));
  const washoutBox = h("input", { type: "checkbox", id: "wm-washout", checked: washout });
  washoutBox.addEventListener("change", () => {
    washout = washoutBox.checked;
    refresh();
  });
  const chosen = h("span", { class: "wm-file", id: "wm-file" }, t("No picture chosen"));
  const picker = h("input", { type: "file", accept: "image/*", hidden: true, id: "wm-picture", "aria-label": t("Picture file") });
  const choose = h("button", { class: "btn", type: "button" }, t("Choose picture…"));
  choose.addEventListener("click", () => picker.click());
  const usePicture = (p: Picture) => {
    picture = p;
    chosen.textContent = p.name;
    refresh();
  };
  picker.addEventListener("change", async () => {
    const file = picker.files?.[0];
    picker.value = "";
    if (!file) return;
    if (blobUrl) URL.revokeObjectURL(blobUrl);
    blobUrl = URL.createObjectURL(file);
    usePicture({ file, name: file.name, url: blobUrl, ...(await measure(blobUrl)) });
  });
  // The picture the document has now can be kept and given another scale or washout.
  if (old?.kind === "picture" && old.media) {
    const url = ed.engine.mediaUrl(old.media);
    const name = old.media.split("/").pop() ?? old.media;
    chosen.textContent = name;
    void measure(url).then((size) => {
      if (!picture) usePicture({ media: old.media, name, url, ...size });
    });
  }
  const colors = swatchGrid(COLORS, color, (c) => {
    color = c;
    refresh();
  }, (c) => `#${c}`, { more: true });

  // The document's own fonts first, then the common ones.
  const select = h("select", { id: "wm-font", "aria-label": t("Font") });
  select.append(h("option", { value: "" }, t("Default")));
  const used = ed.engine.fontsUsed().map((f) => f.family);
  const families = [...new Set([...(font ? [font] : []), ...MENU_FONTS, ...used])];
  for (const f of families) select.append(h("option", { value: f }, f));
  select.value = font;
  select.addEventListener("change", () => {
    font = select.value;
    refresh();
  });

  // A small page drawn the way the real one will be: the same watermark, scaled down.
  const sec = ed.caretSectionInfo();
  const pageW = tw(sec.width);
  const pageH = tw(sec.height);
  const margins = sec.margin.map(tw);
  const WIDTH = 150;
  const scale = WIDTH / pageW;
  const page = h("div", { class: "wm-page", "aria-hidden": "true", style: `width:${WIDTH}px;height:${Math.round(pageH * scale)}px` });
  const sheet = h("div", { class: "wm-sheet", style: `width:${pageW}px;height:${pageH}px;transform:scale(${scale})` });
  page.append(sheet);
  const preview = h("div", { class: "wm-preview" }, page);

  const areaW = pageW - margins[1] - margins[3];
  const areaH = pageH - margins[0] - margins[2];
  /** The picture's size in points: its own size shrunk to fit the text area, or of its own size by the scale. */
  const pictureSize = (p: Picture) => {
    if (pictureScale !== "auto") return { width: p.width * Number(pictureScale), height: p.height * Number(pictureScale) };
    const k = Math.min((areaW * PT) / p.width, (areaH * PT) / p.height, 1);
    return { width: p.width * k, height: p.height * k };
  };
  const current = (): Watermark => {
    if (kind === "picture" && picture) return { kind: "picture", layout: "horizontal", angle: 0, media: picture.url, washout, ...pictureSize(picture) };
    return { kind: "text", text: text.trim() || t("Preview"), font: font || undefined, color, opacity: solid ? 1 : SEMI, layout, angle: layout === "diagonal" ? 315 : 0 };
  };
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  function refresh() {
    kinds.paint();
    layouts.paint();
    weights.paint();
    scales.paint();
    colors.select(color);
    for (const el of textFields) el.hidden = kind !== "text";
    for (const el of pictureFields) el.hidden = kind !== "picture";
    apply.disabled = kind === "text" ? !text.trim() : !picture;
    const box = watermarkElement(current(), areaW, areaH, (m) => (kind === "picture" && picture && m === picture.url ? m : ed.engine.mediaUrl(m)));
    box.style.left = `${margins[3]}px`;
    box.style.top = `${margins[0]}px`;
    // Lines of text behind and around the mark, so it reads as a page.
    const lines = h("div", { class: "wm-lines", style: `left:${margins[3]}px;top:${margins[0]}px;right:${margins[1]}px;bottom:${margins[2]}px` });
    for (let i = 0; i < 40; i++) lines.append(h("i", { class: i % 6 === 5 ? "short" : "" }));
    sheet.replaceChildren(box, lines);
  }

  const textFields = [
    h("div", { class: "field" }, h("label", { for: "wm-text" }, t("Text")), input, quick),
    h("div", { class: "field" }, h("label", {}, t("Layout")), layouts.el),
    h("div", { class: "field" }, h("label", { for: "wm-font" }, t("Font")), select),
    h("div", { class: "field" }, h("label", {}, t("Color")), colors.el),
    h("div", { class: "field" }, h("label", {}, t("Transparency")), weights.el),
  ];
  const pictureFields = [
    h("div", { class: "field" }, h("label", { for: "wm-picture-button" }, t("Picture")), h("div", { class: "wm-pick" }, choose, chosen, picker)),
    h("div", { class: "field" }, h("label", {}, t("Scale")), scales.el),
    h("label", { class: "check", for: "wm-washout" }, washoutBox, h("span", {}, t("Washout"))),
  ];
  choose.id = "wm-picture-button";
  const remove = old ? h("button", { class: "btn ghost danger", type: "button" }, t("Remove watermark")) : null;
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const dialog = h(
    "div",
    { class: "dialog wm-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Watermark") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Watermark"))),
    h(
      "div",
      { class: "dialog-body wm-body" },
      h(
        "div",
        { class: "wm-fields" },
        h("div", { class: "field" }, h("label", {}, t("Kind")), kinds.el),
        ...textFields,
        ...pictureFields,
      ),
      preview,
    ),
    h("div", { class: "dialog-actions" }, remove, h("span", { class: "wm-spacer" }), cancel, apply),
  );
  refresh();
  const closeModal = modal(dialog, () => ed.focus());
  const close = () => {
    if (blobUrl) URL.revokeObjectURL(blobUrl);
    closeModal();
  };
  cancel.addEventListener("click", close);
  remove?.addEventListener("click", async () => {
    // The command asks first; the dialog closes only when the person said yes.
    const ran = await ed.command("watermark.remove");
    if (ran.status === "done") close();
  });
  const go = async () => {
    if (kind === "picture") {
      if (!picture) return;
      const chosen = picture;
      let media = chosen.media;
      if (chosen.file) {
        const ext = (chosen.file.name.split(".").pop() || chosen.file.type.split("/")[1] || "png").toLowerCase();
        media = ed.engine.addMedia(ext === "jpeg" ? "jpg" : ext, new Uint8Array(await chosen.file.arrayBuffer()));
      }
      if (!media) return;
      const op: Op = { op: "set_picture_watermark", media, width: Math.round(chosen.width), height: Math.round(chosen.height), washout, ...(pictureScale === "auto" ? {} : { scale: Number(pictureScale) }) };
      close();
      ed.exec(op);
      ed.reloadPageSetup();
      return;
    }
    if (!text.trim()) return;
    const op: Op = { op: "set_watermark", text: text.trim(), layout, color, opacity: solid ? 1 : SEMI, ...(font ? { font } : {}) };
    close();
    ed.exec(op);
    ed.reloadPageSetup();
  };
  apply.addEventListener("click", () => void go());
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void go();
    }
  });
  kinds.el.addEventListener("click", () => (kind === "text" ? input : choose).focus());
  (kind === "text" ? input : choose).focus();
  if (kind === "text") input.select();
}
