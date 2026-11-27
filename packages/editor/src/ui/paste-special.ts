// Edit, Paste special: choose which format on the clipboard to paste, and how much of its formatting to keep.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { clipFromClipboard, converterLabel, convertersFor, type Clip } from "../editor/paste-convert";
import { PASTE_CHOICES, type PasteChoice } from "../editor/paste-flow";
import { h, modal, toast } from "./dom";

export async function openPasteSpecial(ed: Editor, key: string) {
  let clip: Clip;
  try {
    clip = await clipFromClipboard();
  } catch {
    toast(t("Your browser didn't let Docxtor read the clipboard. Press {key} to paste.", { key }));
    return;
  }
  const sources = convertersFor(clip, ed.pasteFlow.list());
  const hasText = !!clip.get("text/plain");
  const image = clip.files.find((f) => f.type.startsWith("image/"));
  if (!sources.length && !image) {
    toast(t("There is nothing on the clipboard to paste."));
    return;
  }

  // What to paste: one of the formats, or the plain text read as Markdown.
  type Source = { id: string; label: string; formatted: boolean };
  const options: Source[] = sources.map((c) => ({ id: c.id, label: converterLabel(c), formatted: c.id !== "docxtor.text" }));
  if (hasText) options.push({ id: "markdown", label: t("Text read as Markdown"), formatted: false });
  if (image) options.unshift({ id: "image", label: t("Picture"), formatted: false });
  let source = options[0].id;
  let choice: PasteChoice = ed.pasteFlow.choice;

  const sourceRows = options.map((o) => {
    const input = h("input", { type: "radio", name: "paste-source", id: `ps-${o.id}`, checked: o.id === source });
    input.addEventListener("change", () => {
      source = o.id;
      sync();
    });
    return h("div", { class: "pref-check" }, h("label", { class: "check", for: `ps-${o.id}` }, input, h("span", {}, o.label)));
  });
  const choiceInputs = new Map<PasteChoice, HTMLInputElement>();
  const choiceRows = PASTE_CHOICES.map((c) => {
    const input = h("input", { type: "radio", name: "paste-format", id: `pf-${c.id}`, checked: c.id === choice });
    input.addEventListener("change", () => (choice = c.id));
    choiceInputs.set(c.id, input);
    return h("div", { class: "pref-check" }, h("label", { class: "check", for: `pf-${c.id}` }, input, h("span", {}, c.label())), h("p", { class: "pref-help" }, c.help()));
  });
  const formats = h("div", { class: "ps-formats" }, h("h3", { class: "ps-heading" }, t("Formatting")), ...choiceRows);
  const sync = () => {
    const o = options.find((x) => x.id === source);
    const on = !!o?.formatted;
    for (const i of choiceInputs.values()) i.disabled = !on;
    formats.classList.toggle("off", !on);
  };

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const paste = h("button", { class: "btn primary", type: "button" }, t("Paste"));
  const dialog = h(
    "div",
    { class: "dialog ps-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Paste special") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Paste special")), h("p", { class: "sub" }, t("Choose what to paste from the clipboard."))),
    h("div", { class: "dialog-body" }, h("h3", { class: "ps-heading" }, t("Paste as")), ...sourceRows, formats),
    h("div", { class: "dialog-actions" }, cancel, paste),
  );
  const close = modal(dialog, () => ed.focus());
  sync();
  cancel.addEventListener("click", close);
  paste.addEventListener("click", () => {
    close();
    if (source === "image" && image) void ed.insertImage(image);
    else if (source === "markdown") ed.insertMarkdownAtCaret(clip.get("text/plain") ?? "");
    else void ed.pasteFlow.paste(clip, { choice, only: source });
  });
}
