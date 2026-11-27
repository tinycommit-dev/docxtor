// The zoom control for a status bar: zoom out, a slider, zoom in, and the factor, which opens the presets (a factor,
// the page width, a whole page, several pages side by side). Every choice runs a command, so the control, the View
// menu and the keys agree. The Docxtor app puts it in its status bar; a host that embeds the editor can place it too.
import { t } from "@docxtor/extensions";
import type { DocxtorEditor } from "../docxtor-editor";
import { ZOOM_MAX, ZOOM_MIN, zoomLabel } from "../editor/zoom";
import { h, popover } from "./dom";

const CHOICES: (string | "-")[] = ["view.zoomTo.50", "view.zoomTo.75", "view.zoomReset", "view.zoomTo.125", "view.zoomTo.150", "view.zoomTo.200", "-", "view.fitWidth", "view.zoomWholePage", "view.zoomPages.2", "view.zoomPages.3", "view.zoomPages.4", "-", "view.zoomCustom"];

export class ZoomControl {
  readonly el: HTMLElement;
  private slider: HTMLInputElement;
  private readout: HTMLButtonElement;
  private off: () => void;

  constructor(private fe: DocxtorEditor) {
    const ed = fe.editor;
    const run = (id: string) => void fe.extensions.execute(id).catch(() => undefined);
    const tip = (label: string, id: string) => {
      const key = fe.extensions.shortcut(id);
      return key ? `${label} (${key})` : label;
    };
    const out = h("button", { type: "button", class: "zoom-step", title: tip(t("Zoom out"), "view.zoomOut"), "aria-label": t("Zoom out") }, "−");
    const into = h("button", { type: "button", class: "zoom-step", title: tip(t("Zoom in"), "view.zoomIn"), "aria-label": t("Zoom in") }, "+");
    out.addEventListener("click", () => run("view.zoomOut"));
    into.addEventListener("click", () => run("view.zoomIn"));
    this.slider = h("input", { type: "range", class: "zoom-slider", min: String(Math.round(ZOOM_MIN * 100)), max: String(Math.round(ZOOM_MAX * 100)), step: "5", "aria-label": t("Zoom") });
    this.slider.addEventListener("input", () => ed.zoomer.set(Number(this.slider.value) / 100, "centre"));
    this.readout = h("button", { type: "button", class: "readout zoom-readout", title: t("Zoom options"), "aria-haspopup": "menu", "aria-expanded": "false" }, "100%");
    this.readout.addEventListener("click", () => this.menu(run));
    this.el = h("div", { class: "zoom-ctl" }, out, this.slider, into, this.readout);
    this.off = ed.zoomer.onDidChange(() => this.sync());
    this.sync();
  }

  /** Show the factor (the slider, the readout). */
  sync() {
    const z = this.fe.editor.zoom;
    this.slider.value = String(Math.round(z * 100));
    this.readout.textContent = zoomLabel(z);
    const mode = this.fe.editor.zoomer.mode;
    this.readout.title = mode === "width" ? t("Page width") : mode === "page" ? t("Whole page") : mode === "pages" ? t("Several pages") : t("Zoom options");
  }

  destroy() {
    this.off();
  }

  private menu(run: (id: string) => void) {
    const host = this.fe.extensions;
    const list = h("div", { class: "zoom-menu", role: "menu" });
    let close = () => {};
    for (const id of CHOICES) {
      if (id === "-") {
        list.append(h("div", { class: "menu-sep", role: "separator" }));
        continue;
      }
      const c = host.command(id);
      if (!c) continue;
      const on = !!c.toggled?.();
      const b = h("button", { class: `menu-item${on ? " active" : ""}`, type: "button", role: "menuitemradio", "aria-checked": String(on) }, h("span", {}, c.title));
      b.addEventListener("click", () => {
        close();
        run(id);
      });
      list.append(b);
    }
    this.readout.setAttribute("aria-expanded", "true");
    close = popover(this.readout, list, { align: "right", onClose: () => this.readout.setAttribute("aria-expanded", "false") });
  }
}
