// The strip above the page while the Outline view is on: promote and demote, move up and down, expand and
// collapse, which levels to show, first line only, and the way back to the print layout. Every button runs a
// command, so the strip, the menus and the keys always agree.
import { msg, t, type ExtensionHost } from "@docxtor/extensions";
import type { OutlineOps } from "../editor/outline-view";
import { h, svg } from "./dom";

interface Button {
  command: string;
  icon: string;
  label: string;
}

const GROUPS: Button[][] = [
  [
    { command: "outline.promote", icon: "promote", label: msg("Promote") },
    { command: "outline.demote", icon: "demote", label: msg("Demote") },
    { command: "outline.toBody", icon: "demoteBody", label: msg("Demote to body text") },
  ],
  [
    { command: "outline.moveUp", icon: "moveUp", label: msg("Move up") },
    { command: "outline.moveDown", icon: "moveDown", label: msg("Move down") },
  ],
  [
    { command: "outline.expand", icon: "expand", label: msg("Expand") },
    { command: "outline.collapse", icon: "collapse", label: msg("Collapse") },
  ],
];

export class OutlineBar {
  readonly el: HTMLElement;
  private level: HTMLSelectElement;
  private first: HTMLButtonElement;

  constructor(
    host: ExtensionHost,
    private ops: OutlineOps,
    done: () => void,
  ) {
    const icon = (name: string) => svg(host.icon(name) ?? "<svg></svg>");
    const button = (b: Button) => {
      const label = t(b.label);
      const tip = host.shortcut(b.command);
      const title = tip ? `${label} (${tip})` : label;
      const el = h("button", { class: "tb", type: "button", title, "aria-label": label, "data-command": b.command }, icon(b.icon));
      el.addEventListener("click", () => void host.execute(b.command).catch(() => undefined));
      return el;
    };
    this.level = h(
      "select",
      { class: "ob-level", "aria-label": t("Show level") },
      h("option", { value: "0" }, t("All levels")),
      ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => h("option", { value: String(n) }, t("Level {n}", { n }))),
    );
    this.level.addEventListener("change", () => ops.showLevel(Number(this.level.value)));
    // The select is a text field for the keyboard: the editor keeps its selection while it is used.
    this.first = h("button", { class: "tb ob-first", type: "button", "aria-pressed": "false", title: t("Show only the first line of each paragraph") }, icon("firstLine"), h("span", {}, t("First line only")));
    this.first.addEventListener("click", () => {
      ops.setFirstLineOnly(!ops.firstLineOnly);
      this.sync();
    });
    const close = h("button", { class: "tb ob-close", type: "button", title: t("Back to the print layout") }, t("Close outline view"));
    close.addEventListener("click", done);
    this.el = h(
      "div",
      { class: "outline-bar", role: "toolbar", "aria-label": t("Outline") },
      ...GROUPS.map((g) => h("div", { class: "ob-group" }, ...g.map(button))),
      h("div", { class: "ob-group" }, h("label", { class: "ob-label" }, t("Show level"), this.level), this.first),
      h("span", { class: "grow" }),
      close,
    );
    // Clicking a button must not take the selection out of the text.
    this.el.addEventListener("mousedown", (e) => {
      if (!(e.target as HTMLElement).closest("select")) e.preventDefault();
    });
    this.sync();
  }

  /** Show what the view is set to (the level, first line only). */
  sync() {
    this.level.value = String(this.ops.level);
    this.first.classList.toggle("on", this.ops.firstLineOnly);
    this.first.setAttribute("aria-pressed", String(this.ops.firstLineOnly));
  }

  destroy() {
    this.el.remove();
  }
}
