// The undo list: a dropdown on the Undo button (and one on Redo) that names the steps that can be taken back, newest
// first. Pointing at a step marks it and every step after it; choosing it undoes all of them in one go.
import { t, tn } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, popover } from "./dom";
import { opLabel } from "./op-labels";

export type HistoryKind = "undo" | "redo";

/** The steps a list would show, nearest first: what Undo would take back first, or what Redo would do first. */
export function historySteps(ed: Editor, kind: HistoryKind): string[] {
  const engine = ed.engine;
  if (!engine) return [];
  return kind === "undo" ? engine.undoLabels().reverse().map(opLabel) : engine.redoLabels().map(opLabel);
}

export function openHistoryList(ed: Editor, anchor: HTMLElement, kind: HistoryKind) {
  const steps = historySteps(ed, kind);
  if (!steps.length) return;
  const note = h("div", { class: "menu-hint history-foot", "aria-live": "polite" });
  const items: HTMLButtonElement[] = [];
  const mark = (upTo: number) => {
    items.forEach((b, i) => b.classList.toggle("active", i <= upTo));
    const n = upTo + 1;
    note.textContent = kind === "undo" ? tn(n, "Undo {n} action", "Undo {n} actions") : tn(n, "Redo {n} action", "Redo {n} actions");
  };
  let close = () => {};
  const list = h("div", { class: "history-items", role: "menu", "aria-label": kind === "undo" ? t("Undo history") : t("Redo history") });
  steps.forEach((label, i) => {
    const b = h("button", { class: "menu-item", type: "button", role: "menuitem" }, h("span", {}, label));
    b.addEventListener("pointerenter", () => mark(i));
    b.addEventListener("focus", () => mark(i));
    b.addEventListener("click", () => {
      close();
      if (kind === "undo") ed.undo(i + 1);
      else ed.redo(i + 1);
      ed.focus();
    });
    b.addEventListener("keydown", (ev) => {
      const to = ev.key === "ArrowDown" ? i + 1 : ev.key === "ArrowUp" ? i - 1 : ev.key === "Home" ? 0 : ev.key === "End" ? items.length - 1 : -1;
      if (to < 0 || to >= items.length) return;
      ev.preventDefault();
      items[to]!.focus();
    });
    items.push(b);
    list.append(b);
  });
  const box = h("div", { class: "history-list" }, list, note);
  anchor.setAttribute("aria-expanded", "true");
  close = popover(anchor, box, {
    onClose: () => anchor.setAttribute("aria-expanded", "false"),
  });
  mark(0);
  items[0]!.focus();
}
