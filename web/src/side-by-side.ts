// View, Window, View side by side: another document in a pane beside the open one, fully editable, saved to its own file.
// Synchronous scrolling moves both by the same amount, as Word does (from where each was when it was turned on); Reset
// window position gives the panes equal widths and lines them up again. The second document is an editor of its own
// (no menu bar or toolbar), so closing the pane destroys it and everything it listened to.
import { assertEngineSound } from "@docxtor/core";
import { DocxtorEditor, h, t, toast } from "@docxtor/editor";
import { SaveQueue, type Snapshot } from "./save-queue";

/** The document to show beside: its name and bytes, and how to save it back (none: it came from the computer). */
export interface SideDoc {
  /** Where it lives (`library:Notes.docx`), as the app names open documents; none for a file from the computer until it is kept. */
  key?: string;
  name: string;
  bytes: Uint8Array;
  write?: (bytes: Uint8Array) => Promise<unknown>;
}

export interface SideHost {
  /** The open document's editor. */
  main: DocxtorEditor;
  /** The element the editors sit in, side by side. */
  workspace: HTMLElement;
  /** Keep a document that came from the computer in the Library; returns how to save it there from now on. */
  keep(name: string, bytes: Uint8Array): Promise<{ key: string; name: string; write: (bytes: Uint8Array) => Promise<unknown> }>;
  /** The key of the document open in the main editor, if any: the same document is never open in both. */
  mainKey(): string | null;
}

interface Taken extends Snapshot {
  bytes: Uint8Array;
}

/** How long after the last change the second document is saved. */
const SAVE_AFTER = 1500;

export class SideBySide {
  private pane: HTMLElement | null = null;
  private fe: DocxtorEditor | null = null;
  private doc: SideDoc | null = null;
  private ac: AbortController | null = null;
  private timer = 0;
  /** Saves the second document: the same queue the open document uses, so a write never runs twice at once and an edit during one is not lost. */
  private queue: SaveQueue<Taken> | null = null;
  private status: HTMLElement | null = null;
  /** Scroll offsets the two panes had when synchronous scrolling began. */
  private base: [number, number] = [0, 0];
  private following = false;
  sync = true;

  constructor(private host: SideHost) {}

  get on(): boolean {
    return !!this.fe;
  }

  /** The key of the document in the pane. */
  get key(): string | null {
    return this.doc?.key ?? null;
  }

  /** The name of the document in the pane. */
  get name(): string | null {
    return this.doc?.name ?? null;
  }

  /** The second document has changes that no finished write holds (the page-close warning covers them). */
  get unsaved(): boolean {
    return !!this.queue?.dirty;
  }

  /** Start saving the second document now, without waiting for the pause after typing. */
  flush() {
    window.clearTimeout(this.timer);
    this.timer = 0;
    void this.queue?.save(false);
  }

  /** The second document's editor (for tests and for commands that act on the pane with the keyboard). */
  get editor(): DocxtorEditor | null {
    return this.fe;
  }

  async open(doc: SideDoc): Promise<boolean> {
    if (!(await this.close())) return false;
    // One document is edited in one place: the same file open in both panes would save over each other.
    if (doc.key && doc.key === this.host.mainKey()) {
      toast(t("{name} is already open in the main window. Choose another document to show beside it.", { name: doc.name.replace(/\.docx$/i, "") }), 6000);
      return false;
    }
    const pane = h("section", { class: "sbs-pane", "aria-label": t("{name}, beside the open document", { name: doc.name.replace(/\.docx$/i, "") }) });
    const status = h("span", { class: "sbs-status", role: "status", "aria-live": "polite" });
    const close = h("button", { class: "btn sm", type: "button" }, t("Close"));
    const head = h("div", { class: "sbs-head" }, h("span", { class: "sbs-name" }, doc.name.replace(/\.docx$/i, "")), status, close);
    const body = h("div", { class: "sbs-body" });
    pane.append(head, body);
    this.host.workspace.classList.add("side-by-side");
    this.host.main.surface.after(pane);
    const fe = new DocxtorEditor({ toolbar: false, menuBar: false, toolbarAsk: false, fonts: false });
    fe.mount(body);
    try {
      await fe.ready;
      await fe.open(doc.bytes, doc.name);
    } catch (e) {
      await fe.extensions.dispose();
      fe.destroy();
      pane.remove();
      this.host.workspace.classList.remove("side-by-side");
      toast(t("Couldn't open {name}: {reason}", { name: doc.name, reason: (e as Error).message }), 5000);
      return false;
    }
    this.pane = pane;
    this.fe = fe;
    this.doc = doc;
    this.status = status;
    this.queue = this.newQueue(fe, doc);
    this.ac = new AbortController();
    const { signal } = this.ac;
    close.addEventListener("click", () => void this.close(), { signal });
    const off = fe.on("change", () => this.changed());
    signal.addEventListener("abort", off);
    // Ctrl+S in the second pane saves the second document.
    pane.addEventListener(
      "keydown",
      (e: KeyboardEvent) => {
        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
          e.preventDefault();
          e.stopPropagation();
          void this.saveNow();
        }
      },
      { signal, capture: true },
    );
    const a = this.host.main.editor.canvas;
    const b = fe.editor.canvas;
    a.addEventListener("scroll", () => this.follow(a, b, 0), { signal, passive: true });
    b.addEventListener("scroll", () => this.follow(b, a, 1), { signal, passive: true });
    this.reset();
    this.setStatus(doc.write ? t("Saved") : t("Not saved yet"));
    return true;
  }

  /**
   * Close the second document, saving what it still has to save. When that save fails the pane stays open, with its
   * changes, and this resolves false.
   */
  async close(): Promise<boolean> {
    const fe = this.fe;
    const queue = this.queue;
    if (!fe || !queue) return true;
    window.clearTimeout(this.timer);
    this.timer = 0;
    if (queue.dirty || queue.writing) {
      const done = await queue.save(false);
      if (!done.ok || queue.dirty) {
        toast(t("Couldn't save {name}, so it stays open beside this document.", { name: (this.doc?.name ?? "").replace(/\.docx$/i, "") }), 6000);
        return false;
      }
    }
    await queue.idle();
    queue.close();
    this.queue = null;
    this.ac?.abort();
    this.ac = null;
    this.fe = null;
    // The extensions go first, while the document they look at is still there.
    await fe.extensions.dispose();
    fe.destroy();
    this.pane?.remove();
    this.pane = null;
    this.doc = null;
    this.status = null;
    this.host.workspace.classList.remove("side-by-side");
    return true;
  }

  setSync(on: boolean) {
    this.sync = on;
    this.rebase();
  }

  /** Line the two up again where they are (the panes always share the width equally). */
  reset() {
    this.rebase();
  }

  private rebase() {
    if (!this.fe) return;
    this.base = [this.host.main.editor.canvas.scrollTop, this.fe.editor.canvas.scrollTop];
  }

  private follow(from: HTMLElement, to: HTMLElement, side: 0 | 1) {
    if (!this.sync || this.following) return;
    this.following = true;
    to.scrollTop = this.base[1 - side] + (from.scrollTop - this.base[side]);
    // The other pane's own scroll event comes next; it must not move this one back.
    requestAnimationFrame(() => (this.following = false));
  }

  private changed() {
    const q = this.queue;
    if (!q?.dirty) return;
    q.touch();
    window.clearTimeout(this.timer);
    this.timer = window.setTimeout(() => {
      this.timer = 0;
      void this.saveNow(false);
    }, SAVE_AFTER);
  }

  private async saveNow(explicit = true) {
    window.clearTimeout(this.timer);
    this.timer = 0;
    await this.queue?.save(explicit);
  }

  /** The queue for the document in the pane. It names its target when it takes the bytes, so a document that was just kept in the Library is written there. */
  private newQueue(fe: DocxtorEditor, doc: SideDoc): SaveQueue<Taken> {
    const engine = fe.engine!;
    return new SaveQueue<Taken>(
      {
        revision: () => engine.revision,
        // In steps, like the main document's saves: the compressing runs in a worker, and the engine never holds the whole file at once.
        take: () => engine.saveInSteps(),
        write: async ({ bytes }) => {
          // Not after the engine stopped, even for bytes made just before (see crash.ts in @docxtor/core).
          assertEngineSound();
          if (!doc.write) {
            const kept = await this.host.keep(doc.name, bytes);
            doc.write = kept.write;
            doc.name = kept.name;
            doc.key = kept.key;
            toast(t("{name} is now in your Library.", { name: kept.name.replace(/\.docx$/i, "") }));
          } else await doc.write(bytes);
        },
        onState: (state, error) => {
          if (this.doc !== doc) return;
          this.setStatus(state === "saved" ? t("Saved") : state === "saving" ? t("Saving…") : state === "unsaved" ? t("Unsaved changes") : t("Couldn't save: {reason}", { reason: error?.message ?? "" }));
        },
      },
      engine.revision,
    );
  }

  private setStatus(text: string) {
    if (this.status) this.status.textContent = text;
  }
}
