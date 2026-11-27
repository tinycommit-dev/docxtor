import type { Editor } from "../editor/editor";
import { msg, t } from "@docxtor/extensions";
import { h, overlayLayer, popover, svg } from "./dom";
import { I } from "./icons";

const QUICK: [string, string][] = [
  [msg("Improve writing"), msg("Improve the writing of the selected text. Keep meaning and tone.")],
  [msg("Fix spelling and grammar"), msg("Fix spelling and grammar in the selected text only.")],
  [msg("Make shorter"), msg("Make the selected text more concise.")],
  [msg("Make longer"), msg("Expand the selected text with more detail, in the same voice.")],
  [msg("Simplify"), msg("Rewrite the selected text in plain, simple language.")],
  [msg("Make it a list"), msg("Turn the selected text into a bulleted list.")],
  [msg("Translate…"), ""],
];

/** Floating toolbar above a non-empty selection. */
export class Bubble {
  private el: HTMLElement;
  private visible = false;

  constructor(
    private ed: Editor,
    /** Ask the AI about the selection; without it the bubble has no Ask button. */
    private ask: ((prompt?: string) => void) | null,
    /** Open the link editor for the selection (the toolbar's link popover). */
    private link: () => void,
  ) {
    const btn = (content: string | Node, title: string, fn: (b: HTMLButtonElement) => void, cls = "") => {
      const b = h("button", { class: `tb ${cls}`, type: "button", title, "aria-label": title });
      if (typeof content === "string" && content.startsWith("<svg")) b.append(svg(content));
      else b.append(content);
      b.addEventListener("mousedown", (e) => e.preventDefault());
      b.addEventListener("click", () => fn(b));
      return b;
    };
    const askBtn = ask ? btn(h("span", { class: "lamp" }), t("Ask AI"), (b) => this.menu(b), "ask") : null;
    askBtn?.append(t("Ask"));
    this.el = h(
      "div",
      { class: "bubble", style: "display:none", role: "toolbar", "aria-label": t("Selection") },
      btn("B", t("Bold"), () => this.ed.toggleMark("bold"), "glyph"),
      btn("I", t("Italic"), () => this.ed.toggleMark("italic"), "glyph i"),
      btn("U", t("Underline"), () => this.ed.toggleMark("underline"), "glyph u"),
      btn(I.link, t("Link"), () => this.link()),
      askBtn ? h("div", { class: "sep" }) : null,
      askBtn,
    );
    overlayLayer().append(ed.life.own(this.el));
    ed.canvas.addEventListener("scroll", () => this.hide());
    // While a button is held on the page the selection is still being made (a drag, the second press of a double-click): the bubble comes
    // when it is let go. Shown in the middle of a click, it could land under the pointer and take the release and the double-click (Safari
    // selects the word on the second press, and the next frame drew the bubble over the header being double-clicked).
    ed.life.on(ed.canvas, "pointerdown", (e) => {
      if ((e as PointerEvent).button !== 0) return;
      this.pressed = true;
      this.hide();
    });
    // In the next frame, when the click has changed the selection (its selection event comes after the release), and not if a second
    // press of a double-click has come by then.
    const release = () => {
      if (!this.pressed) return;
      this.pressed = false;
      requestAnimationFrame(() => {
        if (!this.pressed) this.update();
      });
    };
    ed.life.on(window, "pointerup", release);
    ed.life.on(window, "pointercancel", release);
  }

  /** A button is held down on the page. */
  private pressed = false;

  private menu(anchor: HTMLElement) {
    const list = h("div", {});
    const close = popover(anchor, list);
    for (const [label, prompt] of QUICK) {
      const it = h("button", { class: "menu-item", type: "button" }, t(label));
      it.addEventListener("click", () => {
        close();
        this.hide();
        this.ask?.(prompt ? t(prompt) : undefined);
      });
      list.append(it);
    }
  }

  hide() {
    if (!this.visible) return;
    this.visible = false;
    this.el.style.display = "none";
  }

  update() {
    if (this.pressed) return this.hide();
    // A caret with no bubble showing has nothing to decide, and asking the browser for the selection after the text changed makes it lay the page out first: a keystroke must not.
    if (!this.visible && this.ed.collapsed()) return;
    const s = window.getSelection();
    if (!s || s.isCollapsed || !s.rangeCount || !this.ed.root.contains(s.anchorNode) || !this.ed.hasFocus()) return this.hide();
    // The right-click menu covers the same ground.
    if (overlayLayer().querySelector(".ctx-menu")) return this.hide();
    // The spelling check panel selects each word it shows; the bubble would only get in its way.
    if (this.ed.canvas.parentElement?.querySelector(".sp-panel")) return this.hide();
    // Cell ranges and images have their own tools.
    if (this.ed.flow.querySelector(".cells-selected")) return this.hide();
    // An image selection has its own tool.
    const frag = s.getRangeAt(0).cloneContents();
    // A chart's own labels and data table are not text the selection holds.
    frag.querySelectorAll(".chart-svg, .chart-sr").forEach((n) => n.remove());
    if (frag.querySelector(".atom.img") && !frag.textContent?.trim()) return this.hide();
    // So does a shape or text box that is selected as a whole.
    if (frag.querySelector(".atom.shape")) {
      const rest = frag.cloneNode(true) as DocumentFragment;
      rest.querySelectorAll(".atom.shape").forEach((a) => a.remove());
      if (!rest.textContent?.trim()) return this.hide();
    }
    const r = s.getRangeAt(0).getBoundingClientRect();
    if (!r.width && !r.height) return this.hide();
    this.el.style.display = "flex";
    this.visible = true;
    const w = this.el.offsetWidth;
    const left = Math.max(8, Math.min(window.innerWidth - w - 8, r.left + r.width / 2 - w / 2));
    const top = r.top - this.el.offsetHeight - 8 < 60 ? r.bottom + 8 : r.top - this.el.offsetHeight - 8;
    this.el.style.left = `${left}px`;
    this.el.style.top = `${top}px`;
    const f = this.ed.format();
    const tbs = this.el.querySelectorAll(".tb");
    tbs[0].classList.toggle("on", !!f?.run.bold);
    tbs[1].classList.toggle("on", !!f?.run.italic);
    tbs[2].classList.toggle("on", !!f?.run.underline && f.run.underline !== "none");
  }
}
