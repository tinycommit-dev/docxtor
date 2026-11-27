// Mailings, Find recipient: look for text in the list (in every column or one) and show the record that has it in place of the
// template; Find next goes on to the next match.
import { t, tn } from "@docxtor/extensions";
import { h, modal } from "../ui/dom";
import type { MergeSession } from "./session";

let seq = 0;

export function openFindRecipient(s: MergeSession) {
  const eng = s.engine;
  const info = s.info();
  if (!eng || !info || !s.needRecipients()) return;
  const id = `mmq-${++seq}`;
  const query = h("input", { type: "search", id: `${id}-q`, spellcheck: "false", autocomplete: "off" });
  const field = h("select", { id: `${id}-field` }, h("option", { value: "" }, t("All columns")), ...info.columns.map((c) => h("option", { value: c }, c)));
  const status = h("p", { class: "help", role: "status", "aria-live": "polite" });
  const next = h("button", { class: "btn primary", type: "button" }, t("Find next"));
  const done = h("button", { class: "btn ghost", type: "button" }, t("Close"));
  let matches: number[] = [];
  let at = -1;
  let last = "";
  const search = () => {
    matches = eng.mergeFind(query.value, field.value);
    at = -1;
    last = `${query.value}\u0000${field.value}`;
  };
  const go = () => {
    if (!query.value.trim()) {
      status.textContent = t("Type some of the recipient's details to look for.");
      return;
    }
    if (last !== `${query.value}\u0000${field.value}`) search();
    if (!matches.length) {
      status.textContent = t("No recipient has that.");
      return;
    }
    at = (at + 1) % matches.length;
    s.preview(matches[at]);
    status.textContent = tn(matches.length, "Showing the only recipient that has it.", "Showing {at} of {n} recipients that have it.", { at: at + 1 });
  };
  const dialog = h(
    "div",
    { class: "dialog mm-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Find recipient") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Find recipient")), h("p", { class: "sub" }, t("Shows the first record that has the text, then the next one each time you choose Find next."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: `${id}-q` }, t("Find")), query), h("div", { class: "field" }, h("label", { for: `${id}-field` }, t("In")), field), status),
    h("div", { class: "dialog-actions" }, done, next),
  );
  const close = modal(dialog, () => s.x.editor.focus());
  done.addEventListener("click", close);
  next.addEventListener("click", go);
  query.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      go();
    }
  });
  query.focus();
}
