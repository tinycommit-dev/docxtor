// Hyphenation: the options (Tools, Hyphenation, Options…) and hyphenating by hand. Hyphenating by hand walks the words that
// start a line (they were pushed down by the one before), offers where each can break and puts an optional hyphen
// (U+00AD) there when the person agrees, as Word's Manual does. Where a word may break comes from the hyphenators
// extensions register (`ctx.hyphenation`); Docxtor's own rules are the fallback.
import type { Op } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal, overlayLayer } from "./dom";
import { check, field, grid, twipsOf, unitInput, unitLabel } from "./form";

/** The hyphen that shows only where a line breaks. */
export const SOFT_HYPHEN = "­";
/** The hyphen that never lets a line break. */
export const NON_BREAKING_HYPHEN = "‑";

/** Word's default hyphenation zone: a quarter of an inch. */
const DEFAULT_ZONE = 360;

export function openHyphenationOptions(ed: Editor) {
  const setup = ed.engine.pageSetup();
  const auto = check("hy-auto", t("Automatically hyphenate document"), !!setup.hyphenation);
  const caps = check("hy-caps", t("Hyphenate words in CAPS"), !setup.doNotHyphenateCaps);
  const zone = unitInput("hy-zone", setup.hyphenationZone ?? DEFAULT_ZONE, unitLabel(t("Hyphenation zone")));
  const limit = h("input", { id: "hy-limit", type: "number", min: "0", max: "100", step: "1", value: setup.consecutiveHyphenLimit ? String(setup.consecutiveHyphenLimit) : "", placeholder: t("No limit"), "aria-label": t("Limit consecutive hyphens to"), style: "width:100%;min-width:0" });
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const ok = h("button", { class: "btn primary", type: "button" }, t("OK"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Hyphenation options") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Hyphenation options")), h("p", { class: "sub" }, t("The zone, the limit and the capitals are kept in the file and followed by Word. The browser decides where lines break on screen and does not follow them."))),
    h(
      "div",
      { class: "dialog-body" },
      auto.row,
      caps.row,
      grid(
        h("div", { class: "field" }, h("label", { for: "hy-zone" }, unitLabel(t("Hyphenation zone"))), zone),
        field(t("Limit consecutive hyphens to"), limit, "hy-limit"),
      ),
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );
  const close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  ok.addEventListener("click", () => {
    const ops: Op[] = [];
    if (auto.input.checked !== !!setup.hyphenation) ops.push({ op: "set_hyphenation", on: auto.input.checked });
    const settings: Record<string, unknown> = {};
    if (caps.input.checked === !!setup.doNotHyphenateCaps) settings.do_not_hyphenate_caps = !caps.input.checked;
    const z = twipsOf(zone, setup.hyphenationZone ?? DEFAULT_ZONE);
    if (z !== (setup.hyphenationZone ?? DEFAULT_ZONE)) settings.hyphenation_zone = z === DEFAULT_ZONE ? 0 : z;
    const n = Math.max(0, Math.round(parseFloat(limit.value) || 0));
    if (n !== (setup.consecutiveHyphenLimit ?? 0)) settings.consecutive_hyphen_limit = n;
    if (Object.keys(settings).length) ops.push({ op: "set_layout_settings", ...settings });
    close();
    if (ops.length) {
      ed.exec(ops);
      ed.reloadPageSetup();
    }
  });
}

/** A word that starts a line, with where it is in the model and where it may break. */
interface Candidate {
  p: number;
  /** UTF-16 offsets of the word in its paragraph. */
  from: number;
  to: number;
  word: string;
  lang: string;
}

/**
 * The next word at or after a position that starts a line other than a paragraph's first: it did not fit on the line
 * above, so a hyphen could pull its first syllable up. Only words of six letters or more in one run of text that hold no
 * optional hyphen yet.
 */
export function nextLineStartWord(ed: Editor, after: { p: number; o: number } | null): Candidate | null {
  const paras = Array.from(ed.root.querySelectorAll<HTMLElement>(".flow .fp[data-p]"));
  const start = after ? paras.findIndex((p) => Number(p.dataset.p) === after.p) : 0;
  const range = document.createRange();
  for (let i = Math.max(0, start); i < paras.length; i++) {
    const para = paras[i];
    const lang = para.dataset.lang || ed.pageInfo.lang || "en";
    const minOffset = i === start && after ? after.o : 0;
    let prevTop: number | null = null;
    for (const seg of Array.from(para.querySelectorAll<HTMLElement>("span.s[data-o]"))) {
      const text = seg.firstChild;
      if (!text || text.nodeType !== Node.TEXT_NODE) continue;
      const data = (text as Text).data;
      const base = Number(seg.dataset.o);
      const re = /\p{L}[\p{L}­]*/gu;
      for (let m = re.exec(data); m; m = re.exec(data)) {
        const word = m[0];
        const at = m.index;
        range.setStart(text, at);
        range.setEnd(text, at + 1);
        const top = range.getClientRects()[0]?.top;
        if (top === undefined) continue;
        const starts = prevTop !== null && top > prevTop + 3 && (at === 0 || /\s/.test(data[at - 1]));
        prevTop = top;
        if (!starts || word.length < 6 || word.includes(SOFT_HYPHEN) || base + at < minOffset) continue;
        return { p: Number(para.dataset.p), from: base + at, to: base + at + word.length, word, lang };
      }
    }
  }
  return null;
}

/**
 * Hyphenate by hand: a small panel (the text stays in view) that goes from word to word. `points` says where a word may
 * break; the person can move the break with the arrows, hyphenate there, skip the word or stop.
 */
export function openManualHyphenation(ed: Editor, points: (word: string, lang: string) => Promise<number[]>) {
  const panel = h("div", { class: "hy-panel", role: "dialog", "aria-label": t("Hyphenate manually") });
  overlayLayer().append(panel);
  let cursor = ed.range()?.[0] ?? null;
  let current: Candidate | null = null;
  let options: number[] = [];
  let at = 0;
  let closed = false;
  const stop = () => {
    closed = true;
    panel.remove();
    ed.focus();
  };

  const word = h("p", { class: "hy-word", "aria-live": "polite" });
  const left = h("button", { class: "btn", type: "button", "aria-label": t("Move the hyphen left") }, t("Earlier"));
  const right = h("button", { class: "btn", type: "button", "aria-label": t("Move the hyphen right") }, t("Later"));
  const yes = h("button", { class: "btn primary", type: "button" }, t("Hyphenate"));
  const skip = h("button", { class: "btn", type: "button" }, t("Skip"));
  const done = h("button", { class: "btn ghost", type: "button" }, t("Done"));
  const status = h("p", { class: "help" });
  panel.append(h("div", { class: "hy-head" }, h("strong", {}, t("Hyphenate manually"))), word, h("div", { class: "hy-buttons" }, left, right), status, h("div", { class: "hy-actions" }, done, skip, yes));
  panel.addEventListener("keydown", (e) => {
    if (e.key === "Escape") stop();
  });

  const show = () => {
    if (!current) {
      word.textContent = "";
      status.textContent = t("Hyphenation is complete.");
      for (const b of [left, right, yes, skip]) b.disabled = true;
      return;
    }
    const point = options[at];
    word.textContent = `${current.word.slice(0, point)}-${current.word.slice(point)}`;
    status.textContent = t("Break {n} of {total}", { n: at + 1, total: options.length });
    left.disabled = at <= 0;
    right.disabled = at >= options.length - 1;
    yes.disabled = false;
    skip.disabled = false;
  };
  const next = async () => {
    for (let guard = 0; guard < 400 && !closed; guard++) {
      const c = nextLineStartWord(ed, cursor);
      if (!c) {
        current = null;
        return show();
      }
      cursor = { p: c.p, o: c.to };
      let pts = await points(c.word, c.lang);
      if (!pts.length) pts = [];
      if (!pts.length) continue;
      current = c;
      options = pts;
      // The break nearest the middle of the word is the first offered: the first syllable pulls up, the rest stays down.
      at = Math.max(0, options.findIndex((p) => p >= Math.min(3, c.word.length - 3)));
      ed.select({ anchor: { p: c.p, o: c.from }, focus: { p: c.p, o: c.to } });
      show();
      return;
    }
    current = null;
    show();
  };
  left.addEventListener("click", () => {
    at = Math.max(0, at - 1);
    show();
  });
  right.addEventListener("click", () => {
    at = Math.min(options.length - 1, at + 1);
    show();
  });
  skip.addEventListener("click", () => void next());
  done.addEventListener("click", stop);
  yes.addEventListener("click", () => {
    if (!current) return;
    const c = current;
    ed.exec({ op: "insert_text", at: { p: c.p, o: c.from + options[at] }, text: SOFT_HYPHEN });
    cursor = { p: c.p, o: c.to + 1 };
    void next();
  });
  void next();
}
