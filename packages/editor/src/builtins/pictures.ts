// Pictures, as an extension: where they come from (Insert, Image lists every picture source: from your computer, from
// a URL, and whatever extensions add), and what can be done to a selected one: crop (to a shape or an aspect ratio),
// rotate and flip, format (outline, effects, adjustments, recolour), position and layout, and reset.
import { defineExtension, msg, t, type Disposable, type Extension, type PictureSource } from "@docxtor/extensions";
import { PICTURE_SHAPES } from "../editor/picture";
import { PICTURE_ICONS } from "../ui/picture-icons";
import { ASPECTS, ASPECT_NAMES } from "../ui/picture-menus";
import { askPictureUrl } from "../ui/picture-url";
import type { Internals } from "./index";

/** The id of the source that opens a file chooser; the menu reaches it through the older `insert.image` command. */
const FILE_SOURCE = "docxtor.file";

/** Docxtor's own sources. */
function fileSource(): PictureSource {
  return {
    id: FILE_SOURCE,
    label: msg("From your computer…"),
    icon: "imageFile",
    pick() {
      return new Promise((resolve) => {
        const picker = document.createElement("input");
        picker.type = "file";
        picker.accept = "image/*,.emf,.wmf,.tif,.tiff";
        picker.addEventListener("change", async () => {
          const f = picker.files?.[0];
          resolve(f ? { bytes: new Uint8Array(await f.arrayBuffer()), name: f.name, type: f.type } : null);
        });
        picker.addEventListener("cancel", () => resolve(null));
        picker.click();
      });
    },
  };
}

function urlSource(): PictureSource {
  return {
    id: "docxtor.url",
    label: msg("From a URL…"),
    icon: "imageUrl",
    note: msg("Your browser fetches the address you type; the site it belongs to sees the request."),
    pick: () => askPictureUrl(),
  };
}

export function pictures({ fe, editor: e, images }: Internals): Extension {
  return defineExtension({
    id: "docxtor.pictures",
    name: msg("Pictures"),
    description: msg("Insert pictures from your computer or an address, and crop, turn, format and position them."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(PICTURE_ICONS)) ctx.icons.register({ id, svg });
      const editable = () => !!fe.engine && !fe.readOnly;
      const onPicture = () => editable() && images.isPicture;

      // -- where pictures come from ----------------------------------------------------------
      ctx.pictures.sources.register(fileSource());
      ctx.pictures.sources.register(urlSource());
      const shown = new Map<string, Disposable[]>();
      const sync = () => {
        const list = fe.extensions.pictureSources.list();
        for (const [id, ds] of shown) {
          if (list.some((s) => s.id === id)) continue;
          ds.forEach((d) => d.dispose());
          shown.delete(id);
        }
        for (const s of list) {
          // The file chooser is the older Insert, Image command.
          if (s.id === FILE_SOURCE || shown.has(s.id)) continue;
          const command = `insert.image.from.${s.id}`;
          const run = async () => {
            const abort = new AbortController();
            try {
              const got = await s.pick({ ui: ctx.ui, signal: abort.signal });
              for (const p of got ? (Array.isArray(got) ? got : [got]) : []) await e.insertPicture(p);
            } catch (err) {
              if ((err as Error).name !== "AbortError") ctx.ui.notify(String((err as Error).message ?? err));
            }
          };
          const title = s.id === "docxtor.url" ? t("Image from a URL…") : t("Image: {source}", { source: t(s.label) });
          shown.set(s.id, [
            ctx.commands.register(command, run, { title, category: t("Insert"), when: () => editable() && !e.story, icon: s.icon }),
            ctx.menus.register({ menu: "insert/image", group: "1", command, title: t(s.label) }),
          ] as unknown as Disposable[]);
        }
      };
      sync();
      ctx.subscriptions.push(fe.extensions.pictureSources.onChange(sync));

      // -- cropping --------------------------------------------------------------------------
      const cmd = (id: string, title: string, run: () => void, extra: { icon?: string; toggled?: () => boolean; when?: () => boolean } = {}) =>
        ctx.commands.register(id, run, { title, category: t("Image"), when: extra.when ?? onPicture, icon: extra.icon, toggled: extra.toggled });
      cmd("image.crop", t("Crop"), () => images.crop(), { icon: "crop", toggled: () => images.cropping });
      for (const [id, label] of PICTURE_SHAPES) cmd(`image.shape.${id}`, t(label), () => images.setShape(id), { toggled: () => (images.info()?.pic?.geom ?? "rect") === id });
      for (const [id, aw, ah] of ASPECTS) cmd(`image.aspect.${id.replace(":", "-")}`, t(ASPECT_NAMES[id]), () => images.cropAspect(aw, ah));
      cmd("image.rotateRight", t("Rotate right 90°"), () => images.rotateBy(90), { icon: "rotateRight" });
      cmd("image.rotateLeft", t("Rotate left 90°"), () => images.rotateBy(-90), { icon: "rotateLeft" });
      cmd("image.flipHorizontal", t("Flip horizontal"), () => images.flip("h"), { icon: "flipHorizontal", toggled: () => !!images.info()?.pic?.flipH });
      cmd("image.flipVertical", t("Flip vertical"), () => images.flip("v"), { icon: "flipVertical", toggled: () => !!images.info()?.pic?.flipV });
      cmd("image.format", t("Format picture…"), () => images.format(), { icon: "pictureFormat" });
      cmd("image.position", t("Position and layout…"), () => images.position(), { icon: "picturePosition" });
      cmd("image.reset", t("Reset picture"), () => images.reset(), { icon: "pictureReset" });

      const item = (menu: string, group: string, command: string, title?: string) => ctx.menus.register({ menu, group, command, title });
      item("format/image", "0a_crop", "image.crop");
      ctx.menus.register({ menu: "format/image", group: "0a_crop", submenu: "format/image-shape", title: t("Crop to shape"), icon: "cropShape" });
      for (const [id] of PICTURE_SHAPES) item("format/image-shape", "1", `image.shape.${id}`);
      ctx.menus.register({ menu: "format/image", group: "0a_crop", submenu: "format/image-aspect", title: t("Crop to aspect ratio"), icon: "cropAspect" });
      for (const [id] of ASPECTS) item("format/image-aspect", "1", `image.aspect.${id.replace(":", "-")}`);
      for (const c of ["rotateRight", "rotateLeft", "flipHorizontal", "flipVertical"]) item("format/image", "0b_turn", `image.${c}`);
      for (const c of ["format", "position", "reset"]) item("format/image", "0c_format", `image.${c}`);
      // The right-click menu on a picture.
      for (const c of ["crop", "format", "position"]) item("context", "4_picture", `image.${c}`);
    },
  });
}
