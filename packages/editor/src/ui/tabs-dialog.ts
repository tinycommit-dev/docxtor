// The Tabs dialog: the tab stops of the selected paragraphs (position, alignment, leader), and the document's default
// tab width. The list is worked on in the dialog and written when OK is pressed, as `w:tabs` (see ParaPatch.tabs).
import type { Op, TabStop } from "@docxtor/core";
import { msg, t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { formatUnits, parseLength, unitName, unitStep } from "../units";
import { h, modal } from "./dom";
import { selectionTarget, type FormatTarget } from "./format-target";

/** Alignments in the order the ruler's tab selector cycles through them. */
export const TAB_KINDS: [string, string][] = [
  ["left", msg("Left")],
  ["center", msg("Center")],
  ["right", msg("Right")],
  ["decimal", msg("Decimal")],
  ["bar", msg("Bar")],
];

const LEADERS: [string, string][] = [
  ["none", msg("None")],
  ["dot", msg("Dots")],
  ["hyphen", msg("Dashes")],
  ["underscore", msg("Line")],
  ["heavy", msg("Heavy line")],
  ["middleDot", msg("Middle dots")],
];

/** The longest distance Word allows for a tab stop (22 inches). */
const MAX = 31680;

const same = (a: TabStop[], b: TabStop[]) => a.length === b.length && a.every((s, i) => s.pos === b[i].pos && s.val === b[i].val && (s.leader ?? "none") === (b[i].leader ?? "none"));

/** The tab stops of the selection, or of whatever `target` stands for (a style being edited). */
export function openTabs(ed: Editor, target: FormatTarget = selectionTarget(ed)) {
  const f = target.state();
  const start: TabStop[] = (f.para.tabs ?? []).filter((s) => s.val !== "clear" && s.val !== "num").map((s) => ({ ...s }));
  const defaultStart = ed.engine.pageSetup().defaultTab || 720;
  let stops: TabStop[] = start.map((s) => ({ ...s }));
  let current = -1;
  /** Something was typed in the position box that no stop has yet. */
  let typed = false;
  /** The default width was typed in; until then its exact twips stand, not the rounded text. */
  let widthTyped = false;

  const list = h("select", { id: "tb-list", size: "7", "aria-label": t("Tab stops") });
  const position = h("input", { id: "tb-pos", type: "text", inputmode: "decimal", autocomplete: "off", "aria-describedby": "tb-error" });
  const error = h("p", { id: "tb-error", class: "help", role: "alert", style: "color:var(--danger);min-height:1.2em;margin:0" });
  const defaults = h("input", { id: "tb-default", type: "text", inputmode: "decimal", autocomplete: "off", value: formatUnits(defaultStart) });
  const kinds = h("div", { class: "seg", role: "radiogroup", "aria-label": t("Alignment") });
  let kind = "left";
  for (const [id, label] of TAB_KINDS) {
    const b = h("button", { type: "button", role: "radio", "data-kind": id }, t(label));
    b.addEventListener("click", () => {
      kind = id;
      paint();
    });
    kinds.append(b);
  }
  const leader = h("select", { id: "tb-leader", "aria-label": t("Leader") }, ...LEADERS.map(([id, label]) => h("option", { value: id }, t(label))));

  const describe = (s: TabStop) => {
    const k = TAB_KINDS.find(([id]) => id === s.val)?.[1] ?? s.val;
    const lead = s.leader && s.leader !== "none" ? `, ${t(LEADERS.find(([id]) => id === s.leader)?.[1] ?? s.leader)}` : "";
    return `${formatUnits(s.pos)} ${unitName()}, ${t(k)}${lead}`;
  };

  function paint() {
    kinds.querySelectorAll<HTMLElement>("button").forEach((b) => {
      const on = b.dataset.kind === kind;
      b.classList.toggle("on", on);
      b.setAttribute("aria-checked", String(on));
    });
    list.replaceChildren(...stops.map((s, i) => h("option", { value: String(i), selected: i === current }, describe(s))));
    clear.disabled = current < 0;
    clearAll.disabled = !stops.length;
  }

  list.addEventListener("change", () => {
    current = Number(list.value);
    const s = stops[current];
    if (!s) return;
    position.value = formatUnits(s.pos);
    typed = false;
    kind = s.val;
    leader.value = s.leader ?? "none";
    error.textContent = "";
    paint();
  });

  const set = h("button", { class: "btn", type: "button" }, t("Set"));
  const clear = h("button", { class: "btn", type: "button" }, t("Clear"));
  const clearAll = h("button", { class: "btn", type: "button" }, t("Clear all"));
  set.addEventListener("click", () => {
    // A stop picked from the list and not moved keeps its exact place (the box shows it rounded).
    if (current >= 0 && !typed && stops[current]) {
      stops[current] = { val: kind, pos: stops[current].pos, ...(leader.value !== "none" ? { leader: leader.value } : {}) };
      paint();
      return;
    }
    const pos = parseLength(position.value);
    if (pos === null || pos < 0 || pos > MAX) {
      error.textContent = t("Type a distance, like 1.5 or 3 cm.");
      position.focus();
      return;
    }
    error.textContent = "";
    typed = false;
    const next: TabStop = { val: kind, pos, ...(leader.value !== "none" ? { leader: leader.value } : {}) };
    const at = stops.findIndex((s) => s.pos === pos);
    if (at >= 0) stops[at] = next;
    else stops.push(next);
    stops.sort((a, b) => a.pos - b.pos);
    current = stops.findIndex((s) => s.pos === pos);
    paint();
  });
  clear.addEventListener("click", () => {
    if (current < 0) return;
    stops.splice(current, 1);
    current = -1;
    position.value = "";
    typed = false;
    paint();
  });
  clearAll.addEventListener("click", () => {
    stops = [];
    current = -1;
    position.value = "";
    typed = false;
    paint();
  });

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog tabs-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Tabs") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Tabs"))),
    h(
      "div",
      { class: "dialog-body" },
      h(
        "div",
        { class: "bs-cols" },
        h("div", { class: "field" }, h("label", { for: "tb-pos" }, t("Tab stop position ({unit})", { unit: unitName() })), position, h("label", { for: "tb-list" }, t("Tab stops")), list),
        h(
          "div",
          {},
          h("div", { class: "field" }, h("label", {}, t("Alignment")), kinds),
          h("div", { class: "field" }, h("label", { for: "tb-leader" }, t("Leader")), leader),
          target.document ? h("div", { class: "field" }, h("label", { for: "tb-default" }, t("Default tab stops ({unit})", { unit: unitName() })), defaults) : null,
        ),
      ),
      error,
      h("div", { class: "seg tb-actions" }, set, clear, clearAll),
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  position.step = String(unitStep());
  paint();
  const close = modal(dialog);
  position.focus();
  cancel.addEventListener("click", close);
  position.addEventListener("input", () => (typed = true));
  defaults.addEventListener("input", () => (widthTyped = true));
  position.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      set.click();
    }
  });
  ok.addEventListener("click", () => {
    // Whatever is typed in the position box and not yet set counts, as it does in Word.
    if (typed && position.value.trim()) set.click();
    if (error.textContent && typed) return;
    const width = target.document && widthTyped ? parseLength(defaults.value) : defaultStart;
    if (width === null || width < 20 || width > MAX) {
      error.textContent = t("Type a distance, like 1.5 or 3 cm.");
      defaults.focus();
      return;
    }
    const more: Op[] = width !== defaultStart ? [{ op: "set_default_tab_stop", twips: width }] : [];
    close();
    if (!same(stops, start)) target.setPara({ tabs: stops }, undefined, more);
    else if (more.length) ed.exec(more, { select: ed.sel, keepExtra: true });
    target.done();
  });
}
