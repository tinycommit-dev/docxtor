// File, New from a template: the templates that come with Docxtor and the ones you saved, with Use, Delete, and a way to
// start from a template file (.dotx or .docx) on your computer.
import { h, modal, t, toast, type TemplateEntry } from "@docxtor/editor";

export type TemplateChoice = { entry: TemplateEntry } | { file: true };

export async function chooseTemplate(opts: { list(): Promise<TemplateEntry[]>; ask(id: string, ...args: unknown[]): Promise<boolean>; onClose?: () => void }): Promise<TemplateChoice | null> {
  return new Promise((resolve) => {
    let answer: TemplateChoice | null = null;
    const body = h("div", { class: "dialog-body tpl-body" });
    const useFile = h("button", { class: "btn", type: "button" }, t("From a file…"));
    const close = h("button", { class: "btn primary", type: "button" }, t("Cancel"));
    const dialog = h(
      "div",
      { class: "dialog tpl-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("New from a template") },
      h("div", { class: "dialog-head" }, h("h2", {}, t("New from a template")), h("p", { class: "sub" }, t("Start a document with the text, styles and page setup of a template."))),
      body,
      h("div", { class: "dialog-actions" }, useFile, h("span", { style: "flex:1" }), close),
    );
    const done = modal(dialog, () => {
      opts.onClose?.();
      resolve(answer);
    });
    close.addEventListener("click", done);
    useFile.addEventListener("click", () => {
      answer = { file: true };
      done();
    });

    const row = (entry: TemplateEntry) => {
      const use = h("button", { class: "btn", type: "button", "aria-label": t("Use {template}", { template: entry.name }) }, t("Use"));
      use.addEventListener("click", () => {
        answer = { entry };
        done();
      });
      const actions = h("div", { class: "tpl-actions" }, use);
      if (entry.remove) {
        const remove = h("button", { class: "btn ghost", type: "button", "aria-label": t("Delete {template}", { template: entry.name }) }, t("Delete"));
        remove.addEventListener("click", async () => {
          if (!(await opts.ask("templates.delete", entry.name))) return;
          try {
            await entry.remove?.();
            void draw();
          } catch (e) {
            toast(t("Couldn't delete the template: {reason}", { reason: (e as Error).message }));
          }
        });
        actions.append(remove);
      }
      return h("div", { class: "tpl-row", role: "listitem" }, h("div", { class: "tpl-name" }, h("b", {}, entry.name), entry.description ? h("span", {}, entry.description) : null), actions);
    };

    const section = (title: string, entries: TemplateEntry[]) =>
      entries.length ? h("section", { class: "tpl-section" }, h("h3", { class: "pref-heading" }, title), h("div", { role: "list" }, ...entries.map(row))) : null;

    const draw = async () => {
      const all = await opts.list();
      const mine = all.filter((x) => x.remove);
      const others = all.filter((x) => !x.remove);
      body.replaceChildren(
        ...[section(t("Your templates"), mine), section(t("Built in"), others)].filter((x): x is HTMLElement => !!x),
        ...(all.length ? [] : [h("p", { class: "empty-note" }, t("No templates yet. Save a document with File, Save as template."))]),
      );
      body.querySelector<HTMLElement>(".tpl-row button")?.focus();
    };
    void draw();
  });
}
