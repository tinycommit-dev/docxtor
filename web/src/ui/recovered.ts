// File, Recovered documents: copies of documents whose changes were not saved when the page closed (a crash, a killed tab).
// Each can be opened as a new document in the Library, or let go.
import { currentLocale, h, modal, t, tn } from "@docxtor/editor";
import type { RecoveryCopy } from "../files/recovery";

function when(at: number): string {
  const options: Intl.DateTimeFormatOptions = { dateStyle: "medium", timeStyle: "short" };
  try {
    return new Intl.DateTimeFormat(currentLocale(), options).format(new Date(at));
  } catch {
    return new Intl.DateTimeFormat(undefined, options).format(new Date(at));
  }
}

export interface RecoveredHost {
  /** The copies to offer now. */
  list(): Promise<RecoveryCopy[]>;
  /** Make the copy a document of its own and open it. */
  open(copy: RecoveryCopy): Promise<void>;
  discard(copy: RecoveryCopy): Promise<void>;
}

export function openRecovered(host: RecoveredHost, onClose?: () => void) {
  const list = h("div", { class: "rec-list", role: "list" });
  const ok = h("button", { class: "btn primary", type: "button" }, t("Close"));
  const dialog = h(
    "div",
    { class: "dialog rec-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Recovered documents") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Recovered documents")), h("p", { class: "sub" }, t("Docxtor kept these changes when the page closed before they were saved."))),
    h("div", { class: "dialog-body" }, list),
    h("div", { class: "dialog-actions" }, ok),
  );
  const close = modal(dialog, onClose);
  ok.addEventListener("click", close);

  const draw = async () => {
    const copies = await host.list();
    if (!copies.length) {
      list.replaceChildren(h("p", { class: "empty-note" }, t("Nothing to recover.")));
      return;
    }
    list.replaceChildren(
      ...copies.map((c) => {
        const open = h("button", { class: "btn", type: "button", "aria-label": t("Open {name}", { name: c.name }) }, t("Open"));
        const drop = h("button", { class: "btn ghost", type: "button", "aria-label": t("Discard {name}", { name: c.name }) }, t("Discard"));
        open.addEventListener("click", async () => {
          close();
          await host.open(c);
        });
        drop.addEventListener("click", async () => {
          await host.discard(c);
          void draw();
        });
        return h(
          "div",
          { class: "tpl-row", role: "listitem" },
          h("div", { class: "tpl-name" }, h("b", {}, c.name), h("span", {}, `${c.place}, ${when(c.at)}, ${tn(c.words, "{n} word", "{n} words")}`)),
          h("div", { class: "tpl-actions" }, open, drop),
        );
      }),
    );
    list.querySelector<HTMLElement>("button")?.focus();
  };
  void draw();
}
