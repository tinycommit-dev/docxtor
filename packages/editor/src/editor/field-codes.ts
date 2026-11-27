// Field codes shown instead of results (View, Field codes, Alt+F9; Shift+F9 for the field at the caret). A field's pieces
// keep their text, which the model's positions count, and are hidden: the first piece shows `{ CODE }` through
// `data-code` (see editor.css). Double-clicking a code edits it in place.
import { t } from "@docxtor/extensions";

/** A field in the DOM: the first piece, which carries the instruction, and the pieces that follow it. */
interface Group {
  first: HTMLElement;
  rest: HTMLElement[];
}

export interface CodeHost {
  /** Change the code of the field that starts at this position. */
  setCode(p: number, o: number, instr: string): void;
}

export class FieldCodes {
  /** Every field shows its code. */
  all = false;
  /** Fields shown one by one (Shift+F9), by `paragraph:offset`. */
  private open = new Set<string>();
  /** Some piece carries code classes, so a later pass has something to undo. */
  private marked = false;

  constructor(private host: CodeHost) {}

  /** Whether anything is shown as a code or could be. */
  get active(): boolean {
    return this.all || this.open.size > 0 || this.marked;
  }

  /** Show or hide the codes of all fields. */
  setAll(on: boolean) {
    this.all = on;
    if (!on) this.open.clear();
  }

  key(p: number, o: number): string {
    return `${p}:${o}`;
  }

  /** Show or hide the code of one field. Returns whether it is shown now. */
  toggleOne(p: number, o: number): boolean {
    const k = this.key(p, o);
    if (this.open.delete(k)) return false;
    this.open.add(k);
    return true;
  }

  isOpen(p: number, o: number): boolean {
    return this.all || this.open.has(this.key(p, o));
  }

  private groups(root: HTMLElement): Group[] {
    const out: Group[] = [];
    let open: Group | null = null;
    for (const el of Array.from(root.querySelectorAll<HTMLElement>("[data-fld], [data-fi]"))) {
      if (el.dataset.fi) {
        out.push((open = { first: el, rest: [] }));
      } else if (open && el.dataset.fld && el.dataset.fld === open.first.dataset.fld && el.closest("[data-id]") === open.first.closest("[data-id]")) {
        open.rest.push(el);
      } else open = null;
    }
    return out;
  }

  /** Bring the DOM under `root` in line with what is shown. Returns whether it changed (the text may need laying out again). */
  apply(root: HTMLElement): boolean {
    if (!this.active) return false;
    let changed = false;
    const keep = new Set<HTMLElement>();
    for (const g of this.groups(root)) {
      const para = g.first.closest<HTMLElement>("[data-p], [data-id]");
      const p = Number(para?.dataset.p ?? para?.dataset.id);
      const o = Number(g.first.dataset.o);
      if (!this.isOpen(p, o)) continue;
      const code = `{ ${g.first.dataset.fi!.trim()} }`;
      keep.add(g.first);
      g.rest.forEach((r) => keep.add(r));
      if (g.first.classList.contains("fc-first") && g.first.dataset.code === code && g.rest.every((r) => r.classList.contains("fc-gone"))) continue;
      // The size the code is drawn in is the size the field's text had.
      if (!g.first.style.getPropertyValue("--fs")) g.first.style.setProperty("--fs", getComputedStyle(g.first).fontSize);
      g.first.classList.remove("fval");
      g.first.removeAttribute("data-val");
      g.first.dataset.code = code;
      g.first.classList.add("fc-first");
      for (const r of g.rest) r.classList.add("fc-gone");
      changed = true;
    }
    // Pieces that showed a code and no longer should.
    for (const el of Array.from(root.querySelectorAll<HTMLElement>(".fc-first, .fc-gone"))) {
      if (keep.has(el)) continue;
      el.classList.remove("fc-first", "fc-gone");
      el.removeAttribute("data-code");
      changed = true;
    }
    this.marked = keep.size > 0;
    return changed;
  }

  /** Edit the code of the piece that shows one: a text box over it; Enter keeps the change, Escape drops it. */
  edit(first: HTMLElement) {
    const para = first.closest<HTMLElement>("[data-p]");
    if (!para || !first.dataset.fi) return;
    const p = Number(para.dataset.p);
    const o = Number(first.dataset.o);
    const input = document.createElement("input");
    input.type = "text";
    input.className = "code-edit";
    input.value = first.dataset.fi.trim();
    input.spellcheck = false;
    input.setAttribute("aria-label", t("Field code"));
    const r = first.getBoundingClientRect();
    input.style.left = `${r.left}px`;
    input.style.top = `${r.top - 2}px`;
    input.style.width = `${Math.max(240, r.width + 24)}px`;
    (first.closest(".docxtor") ?? document.body).append(input);
    input.focus();
    input.select();
    let done = false;
    const finish = (keep: boolean) => {
      if (done) return;
      done = true;
      const value = input.value.trim();
      input.remove();
      if (keep && value && value !== first.dataset.fi!.trim()) this.host.setCode(p, o, value);
    };
    input.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.key === "Enter") {
        e.preventDefault();
        finish(true);
      } else if (e.key === "Escape") {
        e.preventDefault();
        finish(false);
      }
    });
    input.addEventListener("blur", () => finish(false));
  }
}
