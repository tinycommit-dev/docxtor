// Footnote and endnote options: how each kind is numbered (1, i, I, a, A, symbols), where numbering starts and starts again
// (each section, each page), where the notes go on the page or in the document, for the whole document or one section;
// a note with a mark of its own; and moving every note from one kind to the other.
import type { NotePr, Op, RNote } from "@docxtor/core";
import { msg, t, tn } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { pageLabel } from "../editor/layout";
import { h, modal, toast } from "./dom";
import { selectOf } from "./form";

type Format = "decimal" | "lowerRoman" | "upperRoman" | "lowerLetter" | "upperLetter" | "symbol";
type Kind = "footnote" | "endnote";
type Restart = "continuous" | "eachSect" | "eachPage";

const FORMATS: [Format, string][] = [
  ["decimal", msg("1, 2, 3")],
  ["lowerRoman", msg("i, ii, iii")],
  ["upperRoman", msg("I, II, III")],
  ["lowerLetter", msg("a, b, c")],
  ["upperLetter", msg("A, B, C")],
  ["symbol", msg("*, †, ‡")],
];

const SYMBOLS = "*†‡§‖¶#";

/** The marks people reach for first when a note has a mark of its own. */
const QUICK_MARKS = ["*", "†", "‡", "§", "¶", "#"];

/**
 * How a kind of note is numbered now, worked out from the numbers it shows (the engine reports a note's number, not
 * the setting): the format and first number that make every label come out right. Without notes, Word's defaults.
 */
export function noteFormatOf(notes: RNote[], kind: Kind): { format: Format; start: number } {
  const fallback = { format: (kind === "footnote" ? "decimal" : "lowerRoman") as Format, start: 1 };
  if (!notes.length) return fallback;
  const labels = notes.map((n) => n.label);
  if (SYMBOLS.includes(labels[0][0])) return { format: "symbol", start: 1 };
  const order: Format[] = kind === "footnote" ? ["decimal", "lowerLetter", "upperLetter", "lowerRoman", "upperRoman"] : ["lowerRoman", "decimal", "upperRoman", "lowerLetter", "upperLetter"];
  for (const format of order) {
    for (let start = 1; start <= 400; start++) {
      if (pageLabel(start, format) !== labels[0]) continue;
      if (labels.every((l, i) => pageLabel(start + i, format) === l)) return { format, start };
    }
  }
  return fallback;
}

/** What the engine reports for a kind of note, in the dialog's terms (Word's `chicago` is the symbols). */
function settingOf(pr: NotePr | undefined, kind: Kind): { format: Format; start: number; restart: Restart; pos: string } {
  const fmt = pr?.fmt === "chicago" ? "symbol" : (pr?.fmt as Format | undefined);
  return {
    format: FORMATS.some(([v]) => v === fmt) ? fmt! : kind === "footnote" ? "decimal" : "lowerRoman",
    start: pr?.start ?? 1,
    restart: pr?.restart ?? "continuous",
    pos: pr?.pos ?? (kind === "footnote" ? "pageBottom" : "docEnd"),
  };
}

export function openNoteOptions(ed: Editor) {
  const setup = ed.engine.pageSetup();
  const several = (setup.sections?.length ?? 1) > 1;
  const section = ed.sectionAnchor(ed.caretSection());
  const info = setup.sections?.[ed.caretSection()];
  let scope: "document" | "section" = "document";
  const read = (kind: Kind) => settingOf(scope === "section" ? (kind === "footnote" ? info?.footnotePr : info?.endnotePr) : kind === "footnote" ? setup.footnotePr : setup.endnotePr, kind);
  let now = { footnote: read("footnote"), endnote: read("endnote") };

  const picks: Record<Kind, { format: HTMLSelectElement; start: HTMLInputElement; restart: HTMLSelectElement; pos: HTMLSelectElement; mark: HTMLInputElement; insert: HTMLButtonElement }> = {
    footnote: made("footnote", now.footnote),
    endnote: made("endnote", now.endnote),
  };
  function made(kind: Kind, v: ReturnType<typeof settingOf>) {
    const restartItems: [Restart, string][] = [["continuous", t("Continuous")], ["eachSect", t("Restart each section")], ...(kind === "footnote" ? ([["eachPage", t("Restart each page")]] as [Restart, string][]) : [])];
    const posItems: [string, string][] = kind === "footnote" ? [["pageBottom", t("Bottom of page")], ["beneathText", t("Below text")]] : [["sectEnd", t("End of section")], ["docEnd", t("End of document")]];
    const mark = h("input", { type: "text", maxlength: "8", autocomplete: "off", spellcheck: "false", id: `no-${kind}-mark`, "aria-label": kind === "footnote" ? t("Custom mark for a footnote") : t("Custom mark for an endnote"), style: "width:100%;min-width:0" });
    const insert = h("button", { class: "btn", type: "button", disabled: true }, kind === "footnote" ? t("Insert footnote with this mark") : t("Insert endnote with this mark"));
    mark.addEventListener("input", () => (insert.disabled = !mark.value.trim()));
    return {
      format: selectOf(`no-${kind}-format`, FORMATS.map(([f, l]) => [f, t(l)] as const), v.format, undefined),
      start: h("input", { type: "number", min: "1", step: "1", value: String(v.start), id: `no-${kind}-start`, "aria-label": kind === "footnote" ? t("Footnotes start at") : t("Endnotes start at") }),
      restart: selectOf(`no-${kind}-restart`, restartItems, v.restart),
      pos: selectOf(`no-${kind}-pos`, posItems, v.pos),
      mark,
      insert,
    };
  }
  const syncStart = (k: Kind) => {
    // Symbols repeat from the first: there is no number to start at.
    picks[k].start.disabled = picks[k].format.value === "symbol";
  };
  const fill = () => {
    for (const k of ["footnote", "endnote"] as const) {
      const v = now[k];
      picks[k].format.value = v.format;
      picks[k].start.value = String(v.start);
      picks[k].restart.value = v.restart;
      picks[k].pos.value = v.pos;
      syncStart(k);
    }
  };
  for (const k of ["footnote", "endnote"] as const) {
    picks[k].format.addEventListener("change", () => syncStart(k));
    syncStart(k);
  }

  const toEndnotes = h("button", { class: "btn", type: "button" }, t("Convert all footnotes to endnotes"));
  const toFootnotes = h("button", { class: "btn", type: "button" }, t("Convert all endnotes to footnotes"));
  const counts = () => {
    const s = ed.engine.pageSetup();
    toEndnotes.disabled = !s.notes.length;
    toFootnotes.disabled = !s.endnotes.length;
  };
  counts();
  const convert = (to: Kind) => {
    if (ed.story) ed.exitStory(false);
    const res = ed.exec({ op: "convert_notes", to });
    if (!res) return;
    const n = res[0]?.count ?? 0;
    toast(to === "endnote" ? tn(n, "{n} footnote is now an endnote", "{n} footnotes are now endnotes") : tn(n, "{n} endnote is now a footnote", "{n} endnotes are now footnotes"));
    ed.reloadPageSetup();
    counts();
    // The numbers the dialog shows follow the notes that are left.
    const s = ed.engine.pageSetup();
    for (const [k, list] of [["footnote", s.notes], ["endnote", s.endnotes]] as const) {
      if (!list.length) continue;
      const f = noteFormatOf(list, k);
      picks[k].format.value = f.format;
      picks[k].start.value = String(f.start);
      syncStart(k);
    }
  };
  toEndnotes.addEventListener("click", () => convert("endnote"));
  toFootnotes.addEventListener("click", () => convert("footnote"));

  // A note with a mark of its own: typed, or one of the usual symbols.
  let close = () => {};
  for (const k of ["footnote", "endnote"] as const) {
    picks[k].insert.addEventListener("click", () => {
      const mark = picks[k].mark.value.trim();
      if (!mark) return;
      close();
      if (ed.story) ed.exitStory(false);
      if (k === "footnote") ed.insertFootnote(mark);
      else ed.insertEndnote(mark);
    });
  }

  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const column = (title: string, k: Kind) =>
    h(
      "div",
      { class: "note-col" },
      h("h3", {}, title),
      h("div", { class: "field" }, h("label", { for: `no-${k}-pos` }, t("Position")), picks[k].pos),
      h("div", { class: "field" }, h("label", { for: `no-${k}-format` }, t("Number format")), picks[k].format),
      h("div", { class: "field" }, h("label", { for: `no-${k}-start` }, t("Start at")), picks[k].start),
      h("div", { class: "field" }, h("label", { for: `no-${k}-restart` }, t("Numbering")), picks[k].restart),
      h(
        "div",
        { class: "field" },
        h("label", { for: `no-${k}-mark` }, t("Custom mark")),
        picks[k].mark,
        h(
          "div",
          { class: "note-marks", role: "group", "aria-label": t("Marks") },
          ...QUICK_MARKS.map((m) => {
            const b = h("button", { class: "btn ghost note-mark", type: "button", "aria-label": m }, m);
            b.addEventListener("click", () => {
              picks[k].mark.value = m;
              picks[k].insert.disabled = false;
              picks[k].mark.focus();
            });
            return b;
          }),
        ),
        picks[k].insert,
      ),
    );
  // The scope: a document of several sections can set its own notes for each.
  const scopeSel = several
    ? selectOf("no-scope", [["document", t("Whole document")], ["section", t("This section")]], scope)
    : null;
  scopeSel?.addEventListener("change", () => {
    scope = scopeSel.value as typeof scope;
    now = { footnote: read("footnote"), endnote: read("endnote") };
    fill();
  });
  const dialog = h(
    "div",
    { class: "dialog note-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Footnote and endnote options") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Footnote and endnote options")), h("p", { class: "sub" }, t("Footnotes sit at the foot of the page. Endnotes are listed at the end of the document."))),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "note-cols" }, column(t("Footnotes"), "footnote"), column(t("Endnotes"), "endnote")),
      scopeSel && h("div", { class: "field" }, h("label", { for: "no-scope" }, t("Apply changes to")), scopeSel),
      h("div", { class: "field" }, h("label", {}, t("Convert")), h("div", { class: "note-convert" }, toEndnotes, toFootnotes), h("p", { class: "help" }, t("Converting happens at once and can be undone."))),
    ),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  close = modal(dialog, () => ed.focus());
  cancel.addEventListener("click", close);
  apply.addEventListener("click", () => {
    const ops: Op[] = [];
    for (const k of ["footnote", "endnote"] as const) {
      const format = picks[k].format.value as Format;
      const start = format === "symbol" ? undefined : Math.max(1, Math.round(parseFloat(picks[k].start.value) || 1));
      const before = now[k];
      const restart = picks[k].restart.value as Restart;
      const pos = picks[k].pos.value;
      const patch: Record<string, unknown> = {};
      if (format !== before.format) patch.format = format;
      if (start !== undefined && start !== before.start) patch.start = start;
      if (restart !== before.restart) patch.restart = restart;
      if (pos !== before.pos) patch.pos = pos;
      if (!Object.keys(patch).length) continue;
      ops.push({ op: "set_note_props", kind: k, ...(scope === "section" && section !== undefined ? { section } : {}), ...patch });
    }
    close();
    if (ops.length) {
      if (ed.story) ed.exitStory(false);
      ed.exec(ops);
      ed.reloadPageSetup();
    }
    ed.focus();
  });
}
