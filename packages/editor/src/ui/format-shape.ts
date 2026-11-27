// Format shape: a panel beside the pages for everything about how a shape or text box looks (fill, line, effects, size and
// position, text options). It follows the selection, reads each value back from the engine's view model, and changes the
// shape with `set_shape` ops through the shape tool, so every change is one undo step. The panel code lives in this file and
// the files for its sections (format-fill.ts, format-line.ts, format-effects.ts, format-size.ts, format-textbox.ts).
import type { RShape } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { openDock, type Dock } from "./dock";
import { h } from "./dom";
import { effectsSection } from "./format-effects";
import { fillSection } from "./format-fill";
import { lineSection } from "./format-line";
import type { SectionCtx, ShapeSection } from "./format-shape-ctx";
import { sizeSection } from "./format-size";
import { textBoxSection } from "./format-textbox";
import type { ShapeTool } from "./shape-tool";

const ID = "format-shape";

/** A panel for one editor's shape tool. */
export class FormatShapePanel {
  private dock: Dock | null = null;
  private stop: (() => void) | null = null;
  private last = "";
  private sections: ShapeSection[] = [];
  private hint!: HTMLElement;
  private context!: HTMLElement;
  private groupHint!: HTMLElement;

  constructor(
    private ed: Editor,
    private shapes: ShapeTool,
  ) {}

  get isOpen(): boolean {
    return !!this.dock;
  }

  /** Open the panel (or bring it back) and show the selected shape. */
  open() {
    if (!this.dock) {
      this.dock = openDock(this.ed, ID, t("Format shape"), () => this.closed());
      this.build(this.dock);
      this.stop = this.shapes.onSelection(() => this.sync());
    }
    this.last = "";
    this.sync();
    // The panel is for the keyboard too: focus goes to its first heading.
    this.dock.el.querySelector<HTMLElement>(".fs-section:not([hidden]) .fs-toggle")?.focus({ preventScroll: true });
  }

  close() {
    this.dock?.close();
  }

  private closed() {
    this.stop?.();
    this.stop = null;
    this.dock = null;
    this.sections = [];
  }

  private build(dock: Dock) {
    const ed = this.ed;
    const ctx: SectionCtx = {
      get engine() {
        return ed.engine;
      },
      shape: () => this.shapes.current,
      measured: () => this.shapes.measured,
      flip: (axis) => this.shapes.flip(axis),
      rotate: (deg) => this.shapes.rotateBy(deg),
      patch: (patch, opts) => {
        // The op redraws the page and puts the caret back in it; the control the person is using keeps the focus.
        const active = document.activeElement as HTMLElement | null;
        this.shapes.patch(patch, opts);
        if (active && dock.el.contains(active) && active.isConnected && document.activeElement !== active) active.focus({ preventScroll: true });
      },
    };
    this.hint = h("p", { class: "fs-hint" }, t("Select a shape or text box to change how it looks."));
    this.context = h("p", { class: "fs-hint", hidden: true });
    this.groupHint = h("p", { class: "fs-hint", hidden: true }, t("A group has no fill or line of its own. Click a shape inside it to change how that shape looks."));
    this.sections = [fillSection(ctx), lineSection(ctx), effectsSection(ctx), sizeSection(ctx), textBoxSection(ctx)];
    dock.body.classList.add("fs-panel");
    // Undo and redo are the document's, also with the focus in one of the panel's fields (as in Word).
    dock.el.addEventListener("keydown", (e) => {
      const key = e.key.toLowerCase();
      if (!(e.metaKey || e.ctrlKey) || e.altKey || (key !== "z" && key !== "y")) return;
      e.preventDefault();
      if (key === "y" || e.shiftKey) this.ed.redo();
      else this.ed.undo();
    });
    dock.body.replaceChildren(this.hint, this.context, this.groupHint, ...this.sections.map((s) => s.sec.el));
  }

  /** Show the selected shape's values; does nothing when they are the ones already shown. */
  private sync() {
    const dock = this.dock;
    if (!dock) return;
    const s = this.shapes.current;
    const key = s ? JSON.stringify([{ ...s, text: undefined }, this.shapes.isMember, this.shapes.measured, this.ed.readOnly]) : "";
    if (key === this.last) return;
    this.last = key;
    dock.body.toggleAttribute("inert", this.ed.readOnly);
    this.hint.hidden = !!s;
    this.context.hidden = !s || !this.shapes.isMember;
    if (!this.context.hidden) this.context.textContent = t("A shape inside a group.");
    this.groupHint.hidden = !s || s.kind === "shape" || s.kind === "diagram";
    for (const sec of this.sections) {
      const on = !!s && sec.applies(s as RShape);
      sec.sec.setHidden(!on);
      if (on) sec.update(s as RShape);
    }
  }
}

const panels = new WeakMap<ShapeTool, FormatShapePanel>();

/** The Format shape panel of this editor's shape tool (made on first use). */
export function formatShapePanel(ed: Editor, shapes: ShapeTool): FormatShapePanel {
  let p = panels.get(shapes);
  if (!p) {
    p = new FormatShapePanel(ed, shapes);
    panels.set(shapes, p);
  }
  return p;
}
