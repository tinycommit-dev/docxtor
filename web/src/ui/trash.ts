// The trash in the Files view: documents deleted from the Library or a folder, which stay for 30 days and can be put back.
import { h, svg, t, tn, toast } from "@docxtor/editor";
import { I } from "@docxtor/editor";
import type { Backend } from "../files/fs";
import { baseName } from "../files/fs";
import { daysLeft, emptyTrash, forever, KEEP_DAYS, listTrash, restore, type TrashItem } from "../files/trash";

export interface TrashHost {
  /** Ask before a risky action, by its caution id (web/src/cautions.ts): true to go ahead. */
  ask(id: string, ...args: unknown[]): Promise<boolean>;
  /** The place a deleted document came from, if it is connected right now. */
  backendFor(id: string): Backend | undefined;
  /** A document was put back: the Files view shows it again. */
  restored(backend: Backend, path: string): void;
}

export class TrashSection {
  readonly el: HTMLElement;
  private open = false;

  constructor(private host: TrashHost) {
    this.el = h("section", { class: "fs-section trash" });
  }

  /** Open the list (File, Trash). */
  show() {
    this.open = true;
  }

  async render(): Promise<void> {
    const items = await listTrash();
    const toggle = h(
      "button",
      { class: "panel-title trash-toggle", type: "button", "aria-expanded": String(this.open), title: t("Documents you deleted stay here for {days} days.", { days: KEEP_DAYS }) },
      h("span", {}, t("Trash")),
      h("span", { class: "trash-count" }, items.length ? String(items.length) : ""),
    );
    toggle.addEventListener("click", () => {
      this.open = !this.open;
      void this.render();
    });
    const body = h("div", { class: "trash-body" });
    if (this.open) {
      if (!items.length) body.append(h("div", { class: "empty-note" }, t("The trash is empty.")));
      for (const item of items) body.append(this.row(item));
      if (items.length) {
        const empty = h("button", { class: "btn ghost", type: "button" }, svg(I.trash), t("Empty trash"));
        empty.addEventListener("click", async () => {
          if (!(await this.host.ask("trash.empty", items.length))) return;
          await emptyTrash();
          void this.render();
        });
        body.append(h("div", { class: "fs-actions" }, empty));
      }
    }
    this.el.replaceChildren(toggle, body);
  }

  private row(item: TrashItem): HTMLElement {
    const left = daysLeft(item);
    const back = h("button", { class: "btn ghost sm", type: "button", "aria-label": t("Restore {name}", { name: item.name }) }, t("Restore"));
    back.addEventListener("click", async () => {
      try {
        const backend = this.host.backendFor(item.backend);
        const path = await restore(item, backend);
        this.host.restored(backend!, path);
        toast(t("Put “{name}” back in {place}.", { name: baseName(path), place: item.place }));
      } catch (e) {
        toast((e as Error).message, 5000);
      }
      void this.render();
    });
    const gone = h("button", { class: "btn ghost sm", type: "button", "aria-label": t("Delete {name} for good", { name: item.name }) }, t("Delete for good"));
    gone.addEventListener("click", async () => {
      if (!(await this.host.ask("trash.deleteForever", item.name))) return;
      await forever(item);
      void this.render();
    });
    return h(
      "div",
      { class: "trash-row" },
      h("div", { class: "trash-name" }, h("span", { class: "nm", title: item.path }, item.name.replace(/\.(docx|docm|dotx)$/i, "")), h("span", { class: "meta" }, `${item.place}, ${tn(left, "{n} day left", "{n} days left")}`)),
      h("div", { class: "trash-actions" }, back, gone),
    );
  }
}
