// Quick parts: save the selection with a name, gallery, category and description; and one dialog to browse, insert,
// change and delete the saved parts (Word's gallery and Building Blocks Organizer in one).
import { t, tn, type ExtensionHost } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { AUTO_TEXT, listParts, QUICK_PARTS, removePart, savePart, insertPart, snippetName, stores, updatePart, type Part } from "../editor/quick-parts";
import { h, modal, onChoose, toast } from "./dom";

/** The galleries the dialogs offer for saving; any other name can be typed in the gallery box. */
const GALLERIES = [QUICK_PARTS, AUTO_TEXT, "Cover pages"];

let seq = 0;

/** Save the selection as a part, to the gallery `preset` unless the person changes it. */
export function openSaveQuickPart(ed: Editor, host: ExtensionHost, preset = QUICK_PARTS) {
  if (ed.story || !ed.engine) return;
  const all = stores(host);
  if (!all.length) {
    toast(t("There is nowhere to save quick parts yet. Turn on a place for them under Extensions."));
    return;
  }
  if (ed.collapsed()) {
    toast(t("Select the text, table or picture to save first."));
    return;
  }
  const id = `qp-${++seq}`;
  const name = h("input", { type: "text", id: `${id}-name`, value: snippetName(ed.selectedText()), autocomplete: "off", spellcheck: "false" });
  const gallery = h("input", { type: "text", id: `${id}-gallery`, value: preset, list: `${id}-galleries`, autocomplete: "off", spellcheck: "false" });
  const galleries = h("datalist", { id: `${id}-galleries` }, ...GALLERIES.map((g) => h("option", { value: g }, t(g))));
  const category = h("input", { type: "text", id: `${id}-category`, autocomplete: "off", spellcheck: "false", placeholder: t("General") });
  const description = h("textarea", { id: `${id}-description`, rows: "2" });
  const where = h("select", { id: `${id}-store` }, ...all.map((s) => h("option", { value: s.id }, s.label)));
  const note = h("p", { class: "help" }, all[0].note ?? "");
  where.addEventListener("change", () => (note.textContent = all.find((s) => s.id === where.value)?.note ?? ""));

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const save = h("button", { class: "btn primary", type: "button" }, t("Save"));
  const check = () => (save.disabled = !name.value.trim());
  name.addEventListener("input", check);
  check();
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Save selection to Quick parts") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Save selection to Quick parts")), h("p", { class: "sub" }, t("Keeps the text, formatting, tables and pictures you selected, to insert again later."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: `${id}-name` }, t("Name")), name),
      h("div", { class: "field" }, h("label", { for: `${id}-gallery` }, t("Gallery")), gallery, galleries, h("span", { class: "help" }, t("AutoText parts can be typed as their name and expanded with Ctrl+Alt+F3."))),
      h("div", { class: "field" }, h("label", { for: `${id}-category` }, t("Category")), category),
      h("div", { class: "field" }, h("label", { for: `${id}-description` }, t("Description")), description),
      all.length > 1 ? h("div", { class: "field" }, h("label", { for: `${id}-store` }, t("Save in")), where, note) : note,
    ),
    h("div", { class: "dialog-actions" }, cancel, save),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  const go = async () => {
    if (!name.value.trim()) return;
    close();
    try {
      const s = await savePart(host, ed, where.value, { name: name.value, gallery: gallery.value, category: category.value, description: description.value });
      if (s) toast(t("Saved to {gallery}", { gallery: t(s.gallery) }));
    } catch (e) {
      ed.hooks.onError?.(String((e as Error).message ?? e));
    }
  };
  save.addEventListener("click", () => void go());
  name.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void go();
    }
  });
  name.focus();
  name.select();
}

/** Browse, insert, change and delete the saved parts. */
export async function openQuickParts(ed: Editor, host: ExtensionHost) {
  if (!ed.engine) return;
  const id = `qpg-${++seq}`;
  let parts = await listParts(host, ed);
  let active: Part | null = null;
  let editing = false;

  const filter = h("input", { type: "text", "aria-label": t("Search quick parts"), placeholder: t("Search"), autocomplete: "off", spellcheck: "false" });
  const list = h("div", { class: "qp-list", role: "listbox", "aria-label": t("Quick parts") });
  const detail = h("div", { class: "qp-detail", "aria-live": "polite" });
  const insert = h("button", { class: "btn primary", type: "button" }, t("Insert"));
  const edit = h("button", { class: "btn", type: "button" }, t("Edit properties…"));
  const remove = h("button", { class: "btn", type: "button" }, t("Delete"));
  const closeBtn = h("button", { class: "btn ghost", type: "button" }, t("Close"));

  const paintDetail = () => {
    const p = active;
    insert.disabled = !p || editing;
    edit.disabled = !p || editing || p.store === null;
    remove.disabled = !p || editing || p.store === null;
    if (!p) {
      detail.replaceChildren(h("p", { class: "help" }, parts.length ? t("Choose a part to see it.") : t("Nothing saved yet. Select some text and use Insert, Quick parts, Save selection to Quick parts.")));
      return;
    }
    if (editing) {
      const name = h("input", { type: "text", id: `${id}-n`, value: p.name, autocomplete: "off" });
      const gallery = h("input", { type: "text", id: `${id}-g`, value: p.gallery, list: `${id}-gl`, autocomplete: "off" });
      const galleries = h("datalist", { id: `${id}-gl` }, ...GALLERIES.map((g) => h("option", { value: g }, t(g))));
      const category = h("input", { type: "text", id: `${id}-c`, value: p.category ?? "", autocomplete: "off" });
      const description = h("textarea", { id: `${id}-d`, rows: "2" }, p.description ?? "");
      const save = h("button", { class: "btn primary", type: "button" }, t("Save"));
      const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
      save.addEventListener("click", async () => {
        if (!name.value.trim()) return;
        await updatePart(host, p, { name: name.value, gallery: gallery.value, category: category.value, description: description.value });
        editing = false;
        await reload(p.id);
      });
      cancel.addEventListener("click", () => {
        editing = false;
        paintDetail();
      });
      detail.replaceChildren(
        h("div", { class: "field" }, h("label", { for: `${id}-n` }, t("Name")), name),
        h("div", { class: "field" }, h("label", { for: `${id}-g` }, t("Gallery")), gallery, galleries),
        h("div", { class: "field" }, h("label", { for: `${id}-c` }, t("Category")), category),
        h("div", { class: "field" }, h("label", { for: `${id}-d` }, t("Description")), description),
        h("div", { class: "qp-edit-actions" }, cancel, save),
      );
      name.focus();
      return;
    }
    const lines: (HTMLElement | null)[] = [
      h("h3", {}, p.name),
      h("p", { class: "qp-meta" }, [t(p.gallery), p.category].filter(Boolean).join(", ")),
      p.description ? h("p", {}, p.description) : null,
      h("p", { class: "qp-preview" }, p.text),
      p.store === null ? h("p", { class: "help" }, t("This part is in the document's own list, so it can be inserted but not changed.")) : null,
    ];
    detail.replaceChildren(...lines.filter((x): x is HTMLElement => !!x));
  };

  const paintList = () => {
    const words = filter.value.toLowerCase().split(/\s+/).filter(Boolean);
    const shown = parts.filter((p) => words.every((w) => `${p.name} ${p.gallery} ${p.category ?? ""} ${p.description ?? ""} ${p.text}`.toLowerCase().includes(w)));
    const groups = new Map<string, Part[]>();
    for (const p of shown) groups.set(p.gallery, [...(groups.get(p.gallery) ?? []), p]);
    const nodes: HTMLElement[] = [];
    for (const [gallery, items] of groups) {
      nodes.push(h("div", { class: "qp-group", role: "presentation" }, t(gallery)));
      for (const p of items) {
        const b = h("button", { type: "button", role: "option", class: `qp-item${active?.id === p.id ? " on" : ""}`, "aria-selected": String(active?.id === p.id) }, h("span", { class: "qp-name" }, p.name), p.category ? h("span", { class: "qp-cat" }, p.category) : null);
        onChoose(
          b,
          () => {
            active = p;
            editing = false;
            paintList();
            paintDetail();
          },
          go,
        );
        nodes.push(b);
      }
    }
    // A list box with no options is a plain box with a note in it.
    if (nodes.length) {
      list.setAttribute("role", "listbox");
      list.setAttribute("aria-label", t("Quick parts"));
    } else {
      list.removeAttribute("role");
      list.removeAttribute("aria-label");
    }
    list.replaceChildren(...(nodes.length ? nodes : [h("p", { class: "qp-empty" }, parts.length ? t("Nothing matches.") : t("Nothing saved yet."))]));
  };

  const reload = async (keep?: string) => {
    parts = await listParts(host, ed);
    active = parts.find((p) => p.id === keep) ?? null;
    paintList();
    paintDetail();
  };

  const dialog = h(
    "div",
    { class: "dialog qp-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Quick parts") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Quick parts")), h("p", { class: "sub" }, tn(parts.length, "{n} part you can insert", "{n} parts you can insert"))),
    h("div", { class: "dialog-body qp-body" }, h("div", { class: "qp-side" }, filter, list), detail),
    h("div", { class: "dialog-actions" }, remove, edit, closeBtn, insert),
  );
  const close = modal(dialog, () => ed.focus());
  closeBtn.addEventListener("click", close);
  const go = () => {
    if (!active) return;
    const p = active;
    close();
    insertPart(ed, p);
  };
  insert.addEventListener("click", go);
  edit.addEventListener("click", () => {
    editing = true;
    paintDetail();
  });
  remove.addEventListener("click", async () => {
    if (!active || !(await host.ask("quickParts.delete", active.name))) return;
    await removePart(host, active);
    await reload();
  });
  filter.addEventListener("input", paintList);
  filter.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      go();
    }
  });
  paintList();
  paintDetail();
  filter.focus();
}
