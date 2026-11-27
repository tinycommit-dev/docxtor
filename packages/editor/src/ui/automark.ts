// AutoMark: mark every text listed in a file for the index. The file is a document with one table of two columns, as in
// Word: the text to find, then its index entry (Fruit:Apple puts Apple under Fruit).
import { Engine, type AutoMarkEntry } from "@docxtor/core";
import { t, tn } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { autoMark } from "../editor/lists";
import { h, modal, toast } from "./dom";

export function openAutoMark(ed: Editor) {
  if (ed.story) return;
  let entries: AutoMarkEntry[] = [];
  const file = h("input", { type: "file", accept: ".docx,application/vnd.openxmlformats-officedocument.wordprocessingml.document", id: "am-file", "aria-label": t("AutoMark file") });
  const message = h("p", { class: "help", role: "status", "aria-live": "polite" });
  const list = h("div", { class: "am-list", role: "group", "aria-label": t("Entries in the file") });
  const example = h("button", { class: "btn ghost", type: "button" }, t("Download an example file"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const mark = h("button", { class: "btn primary", type: "button" }, t("Mark"));
  mark.disabled = true;

  const paint = () => {
    list.replaceChildren(...entries.slice(0, 200).map((e) => h("div", { class: "am-row" }, h("span", {}, e.text), h("span", {}, e.entry || e.text))));
    mark.disabled = !entries.length;
  };
  file.addEventListener("change", async () => {
    const f = file.files?.[0];
    entries = [];
    message.textContent = "";
    if (f) {
      try {
        entries = Engine.readAutoMarkFile(new Uint8Array(await f.arrayBuffer()));
        message.textContent = tn(entries.length, "{n} entry in the file.", "{n} entries in the file.");
      } catch (e) {
        message.textContent = String((e as Error).message ?? e);
      }
    }
    paint();
  });
  example.addEventListener("click", () => {
    const bytes = Engine.newAutoMarkFile();
    const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document" }));
    const a = h("a", { href: url, download: "automark.docx" });
    document.body.append(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  });

  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("AutoMark") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("AutoMark")), h("p", { class: "sub" }, t("Marks the first place in each paragraph where a listed text appears, matching capitals. The file is a document with a table of two columns: the text to find, then its index entry."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: "am-file" }, t("File")), file, message, list), example),
    h("div", { class: "dialog-actions" }, cancel, mark),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  mark.addEventListener("click", () => {
    close();
    const n = autoMark(ed, entries);
    toast(n ? tn(n, "Marked {n} place for the index", "Marked {n} places for the index") : t("None of the listed texts is in the document"));
  });
}
