// What the Paragraph, Tabs and Borders and shading dialogs format. Usually it is the selection; the Modify style dialog
// points them at a style being edited instead, so the same dialogs serve both.
import type { Op, ParaFmt, ParaPatch, RunFmt, RunPatch } from "@docxtor/core";
import type { VisualAlign } from "../editor/align";
import type { Editor } from "../editor/editor";

export interface FormatTarget {
  /** The formatting the dialog starts from. */
  state(): { para: ParaFmt; run: RunFmt };
  /** Write a paragraph patch, the alignment by what shows on screen, and further ops that go with it. */
  setPara(patch: ParaPatch, align?: VisualAlign, more?: Op[]): void;
  /** The target belongs to the document's text (so document-wide settings such as the default tab width make sense here). */
  document: boolean;
  /** The dialog is done: give focus back. */
  done(): void;
}

/** The paragraphs the selection touches. */
export function selectionTarget(ed: Editor): FormatTarget {
  return {
    state: () => {
      const f = ed.format();
      return { para: f?.para ?? {}, run: f?.run ?? {} };
    },
    setPara: (patch, align, more) => ed.setParaWith(patch, align, more),
    document: true,
    done: () => ed.focus(),
  };
}

/** What the Font dialog formats: the selection, or a style being edited. */
export interface RunTarget {
  /** The formatting the dialog starts from. */
  state(): RunFmt;
  /** Write a run patch. */
  setRun(patch: RunPatch): void;
  /** The target is the document's text (so Set as default, which changes the document defaults, makes sense here). */
  defaults: boolean;
  /** The dialog is done: give focus back. */
  done(): void;
}

/** The selected text (or what is typed next). */
export function selectionRunTarget(ed: Editor): RunTarget {
  return { state: () => ed.formatWithPending()?.run ?? {}, setRun: (patch) => ed.setRun(patch), defaults: true, done: () => ed.focus() };
}
