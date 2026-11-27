// The equation editor: a popover at the caret (Insert, Equation) or on an equation (double-click, or Enter on a
// selected one). A LaTeX field with a live preview, a small palette of structures and symbols that put LaTeX in
// the field, and a switch for an equation on its own line. The engine reads the LaTeX; this only asks it.
import { msg, t } from "@docxtor/extensions";
import type { MPos } from "@docxtor/core";
import type { Editor, EquationAtCaret } from "../editor/editor";
import { mathNode } from "../editor/math";
import { h, popover } from "./dom";

/** Where the caret goes in a snippet: the first field to fill in. */
const CARET = "\u0001";

/** A structure: its name, the LaTeX it puts in the field (the caret mark says where to type next) and what its button draws. */
const STRUCTURES: { name: string; latex: string; sample: string }[] = [
  { name: msg("Fraction"), latex: `\\frac{${CARET}}{}`, sample: "\\frac{a}{b}" },
  { name: msg("Superscript"), latex: `^{${CARET}}`, sample: "x^{2}" },
  { name: msg("Subscript"), latex: `_{${CARET}}`, sample: "x_{i}" },
  { name: msg("Square root"), latex: `\\sqrt{${CARET}}`, sample: "\\sqrt{x}" },
  { name: msg("Sum"), latex: `\\sum_{${CARET}}^{}`, sample: "\\sum_{i}^{n}" },
  { name: msg("Integral"), latex: `\\int_{${CARET}}^{}`, sample: "\\int_{a}^{b}" },
  { name: msg("Matrix"), latex: `\\begin{pmatrix} ${CARET} & \\\\ & \\end{pmatrix}`, sample: "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}" },
  { name: msg("Brackets"), latex: `\\left( ${CARET} \\right)`, sample: "\\left( x \\right)" },
];

/** Symbols: the character that is drawn and the LaTeX command behind it. */
const GREEK: [string, string][] = [
  ["α", "\\alpha"],
  ["β", "\\beta"],
  ["γ", "\\gamma"],
  ["δ", "\\delta"],
  ["ε", "\\varepsilon"],
  ["θ", "\\theta"],
  ["λ", "\\lambda"],
  ["μ", "\\mu"],
  ["π", "\\pi"],
  ["σ", "\\sigma"],
  ["φ", "\\varphi"],
  ["ω", "\\omega"],
  ["Δ", "\\Delta"],
  ["Σ", "\\Sigma"],
  ["Ω", "\\Omega"],
];
const OPERATORS: [string, string][] = [
  ["±", "\\pm"],
  ["×", "\\times"],
  ["÷", "\\div"],
  ["⋅", "\\cdot"],
  ["≤", "\\leq"],
  ["≥", "\\geq"],
  ["≠", "\\neq"],
  ["≈", "\\approx"],
  ["∞", "\\infty"],
  ["∂", "\\partial"],
  ["∇", "\\nabla"],
  ["→", "\\to"],
  ["⇒", "\\Rightarrow"],
  ["∈", "\\in"],
  ["∪", "\\cup"],
  ["∩", "\\cap"],
  ["∀", "\\forall"],
  ["∃", "\\exists"],
];

export interface EquationEditorOptions {
  /** Edit this equation (the button says Update); without it a new one goes at the caret. */
  edit?: EquationAtCaret;
}

/** Something to anchor the popover to near the caret. */
function caretAnchor(ed: Editor): { x: number; y: number } {
  const s = window.getSelection();
  const rect = s && s.rangeCount ? s.getRangeAt(0).getBoundingClientRect() : null;
  if (rect && (rect.width || rect.height)) return { x: rect.left, y: rect.bottom };
  const para = ed.sel ? ed.paraElement(ed.sel.focus.p) : null;
  const r = (para ?? ed.canvas).getBoundingClientRect();
  return { x: r.left + 24, y: r.top + 24 };
}

/** The sample drawing for a palette button, or its plain text when the engine can't draw it. */
function sampleNode(ed: Editor, latex: string): Node | null {
  try {
    return mathNode(ed.engine.latexToMathml(latex, false));
  } catch {
    return null;
  }
}

export function openEquationEditor(ed: Editor, opts: EquationEditorOptions = {}, anchor?: HTMLElement | { x: number; y: number }) {
  if (ed.readOnly) return;
  const editing = opts.edit;
  const saved = ed.sel;
  const where: MPos | null = editing ? editing.at : null;
  const at = anchor ?? (editing ? ed.equationElement(editing.at) : null) ?? caretAnchor(ed);

  const field = h("textarea", { class: "eq-field", id: "eq-latex", rows: 2, spellcheck: "false", autocomplete: "off", autocapitalize: "off", "aria-describedby": "eq-error", placeholder: "\\frac{a}{b}" });
  field.value = editing?.latex ?? "";
  const error = h("p", { class: "eq-error", id: "eq-error", "aria-live": "polite" });
  const preview = h("div", { class: "eq-preview", role: "img", "aria-label": t("Preview of the equation") });
  const display = h("input", { type: "checkbox", id: "eq-display" });
  display.checked = editing?.display ?? false;
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const go = h("button", { class: "btn primary", type: "button" }, editing ? t("Update") : t("Insert"));

  const button = (label: Node | string, name: string, onclick: () => void, extra = "") => {
    const b = h("button", { class: `eq-key ${extra}`.trim(), type: "button", title: name, "aria-label": name }, label);
    // Pressing a key must not take focus (and the caret) away from the field.
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", onclick);
    return b;
  };

  /** Put LaTeX at the cursor (around the selected text when the snippet has a place for it), and leave the cursor in the first blank. */
  const insertSnippet = (snippet: string) => {
    const a = field.selectionStart;
    const b = field.selectionEnd;
    const chosen = field.value.slice(a, b);
    const mark = snippet.indexOf(CARET);
    const text = snippet.replace(CARET, chosen);
    field.setRangeText(text, a, b, "end");
    const caret = mark < 0 ? a + text.length : a + mark + chosen.length;
    field.setSelectionRange(caret, caret);
    field.focus();
    update();
  };

  const structures = h("div", { class: "eq-keys", role: "group", "aria-label": t("Structures") });
  for (const s of STRUCTURES) {
    const name = t(s.name);
    const drawn = sampleNode(ed, s.sample);
    structures.append(button(drawn ?? name, name, () => insertSnippet(s.latex), "eq-struct"));
  }
  const symbols = (list: [string, string][], label: string) => {
    const row = h("div", { class: "eq-keys", role: "group", "aria-label": label });
    for (const [glyph, cmd] of list) row.append(button(glyph, cmd, () => insertSnippet(`${cmd} `)));
    return row;
  };

  const box = h(
    "div",
    { class: "eq-pop", role: "group", "aria-label": t("Equation") },
    h("div", { class: "menu-title" }, t("Equation")),
    structures,
    symbols(GREEK, t("Greek letters")),
    symbols(OPERATORS, t("Operators")),
    h("div", { class: "eq-fields" }, h("label", { class: "cite-label", for: "eq-latex" }, t("LaTeX")), field, error),
    h("div", { class: "eq-fields" }, h("span", { class: "cite-label" }, t("Preview")), preview),
    h("label", { class: "cite-check" }, display, h("span", {}, t("Display on its own line"))),
    h("div", { class: "bm-actions" }, cancel, go),
  );

  // ---- the live preview ---------------------------------------------------------------------------------

  let timer = 0;
  let valid = false;
  const draw = () => {
    const latex = field.value.trim();
    if (!latex) {
      valid = false;
      error.textContent = "";
      field.removeAttribute("aria-invalid");
      preview.replaceChildren(h("span", { class: "eq-hint" }, t("Your equation appears here")));
      preview.classList.remove("stale");
    } else {
      try {
        const node = mathNode(ed.engine.latexToMathml(latex, display.checked));
        if (!node) throw new Error(t("Docxtor could not draw this equation."));
        preview.replaceChildren(node);
        preview.classList.remove("stale");
        error.textContent = "";
        field.removeAttribute("aria-invalid");
        valid = true;
      } catch (e) {
        // The engine says what is wrong and where; the last good drawing stays, faded.
        error.textContent = String((e as Error).message ?? e);
        field.setAttribute("aria-invalid", "true");
        preview.classList.add("stale");
        valid = false;
      }
    }
    go.disabled = !valid;
  };
  const update = () => {
    window.clearTimeout(timer);
    go.disabled = true;
    timer = window.setTimeout(draw, 140);
  };
  field.addEventListener("input", update);
  display.addEventListener("change", draw);

  // ---- leaving ----------------------------------------------------------------------------------------------

  let done = false;
  const close = popover(at, box, {
    onClose: () => {
      window.clearTimeout(timer);
      // Leaving without a change puts the caret back where it was; after one, the caret is where the edit left it.
      if (saved && !done) ed.select(saved);
      ed.focus();
    },
  });
  cancel.addEventListener("click", close);
  const commit = () => {
    window.clearTimeout(timer);
    draw();
    if (!valid) return;
    const latex = field.value.trim();
    if (saved) ed.select(saved);
    const fail = (m: string) => {
      error.textContent = m;
      field.setAttribute("aria-invalid", "true");
    };
    const ok = where ? ed.setEquation(where, latex, display.checked === editing!.display ? undefined : display.checked, fail) : ed.insertEquation(latex, display.checked, fail);
    if (ok) {
      done = true;
      close();
    }
  };
  go.addEventListener("click", commit);
  field.addEventListener("keydown", (e) => {
    // Enter commits; Shift+Enter makes a new line (a matrix reads easier over several).
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      commit();
    }
  });
  draw();
  setTimeout(() => {
    field.focus();
    // Editing starts with everything selected, so typing replaces the equation.
    if (editing) field.select();
  });
}
