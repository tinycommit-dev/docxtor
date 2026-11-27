// <docxtor-editor>: the editor as an HTML element, for any framework or none.
//
//   <docxtor-editor src="/files/contract.docx" theme="system" style="height: 80vh"></docxtor-editor>
//   el.addEventListener("docxtor-save", (e) => upload(e.detail.bytes));   // ⌘S inside the editor
//   el.addEventListener("docxtor-change", () => markDirty());
//   const bytes = el.save();
//
// Attributes: src (a .docx URL), name, theme (light | dark | system), icon-pack (docxtor | docxtor-light | an extension's), locale, readonly, no-toolbar,
// fonts (a stylesheet URL to self-host the fonts, or "none" if the page already loads them).
import { t } from "@docxtor/extensions";
import { DocxtorEditor } from "./docxtor-editor";
import type { SpellingProvider } from "./spelling/types";
import type { Theme } from "./ui/dom";

export class DocxtorEditorElement extends HTMLElement {
  static observedAttributes = ["src", "theme", "icon-pack", "readonly"];
  /** The editor behind the element (created when the element is first connected). */
  editor: DocxtorEditor | null = null;
  /** Dictionaries for Docxtor's own spelling checker (see the `spelling` option of DocxtorEditor). Set before the element is connected. */
  spelling: SpellingProvider | null = null;

  connectedCallback() {
    if (this.editor) return;
    this.editor = new DocxtorEditor({
      toolbar: !this.hasAttribute("no-toolbar"),
      fonts: this.getAttribute("fonts") === "none" ? false : (this.getAttribute("fonts") ?? undefined),
      locale: this.getAttribute("locale") ?? undefined,
      iconPack: this.getAttribute("icon-pack") ?? undefined,
      spelling: this.spelling ?? false,
      onSave: (bytes) => this.dispatchEvent(new CustomEvent("docxtor-save", { detail: { bytes } })),
      onError: (message) => this.dispatchEvent(new CustomEvent("docxtor-error", { detail: { message } })),
    }).mount(this, { theme: (this.getAttribute("theme") as Theme | null) ?? "system" });
    this.setReadonlyAttribute(this.hasAttribute("readonly"));
    this.editor.on("change", () => this.dispatchEvent(new CustomEvent("docxtor-change")));
    this.editor.on("selection", (format) => this.dispatchEvent(new CustomEvent("docxtor-selection", { detail: { format } })));
    const src = this.getAttribute("src");
    if (src) void this.loadUrl(src);
  }

  /** The attribute is a reason of its own, so it never undoes the host's `readOnly` option or a view's. */
  private releaseReadonly: (() => void) | null = null;

  private setReadonlyAttribute(on: boolean) {
    if (on && !this.releaseReadonly) this.releaseReadonly = this.editor?.lock("readonly attribute") ?? null;
    else if (!on) {
      this.releaseReadonly?.();
      this.releaseReadonly = null;
    }
  }

  disconnectedCallback() {
    this.releaseReadonly = null;
    this.editor?.destroy();
    this.editor = null;
  }

  attributeChangedCallback(name: string, old: string | null, value: string | null) {
    if (!this.editor || old === value) return;
    if (name === "src" && value) void this.loadUrl(value);
    if (name === "theme") this.editor.setTheme((value as Theme | null) ?? "system");
    if (name === "icon-pack") this.editor.setIconPack(value ?? "docxtor");
    if (name === "readonly") this.setReadonlyAttribute(value !== null);
  }

  /** Open .docx bytes. A password-protected file needs `password` (see `DocxtorEditor.open`). */
  async load(bytes: Uint8Array | ArrayBuffer, name = this.getAttribute("name") ?? "document.docx", password?: string) {
    if (!this.editor) throw new Error("<docxtor-editor> is not connected");
    await this.editor.open(bytes, name, password);
    this.dispatchEvent(new CustomEvent("docxtor-ready"));
  }

  /** The document as .docx bytes. */
  save(): Uint8Array {
    if (!this.editor) throw new Error("<docxtor-editor> is not connected");
    return this.editor.save();
  }

  /** The same bytes as `save()`, made in steps with the compressing in a worker (see `DocxtorEditor.saveInSteps`). */
  saveInSteps(): Promise<Uint8Array> {
    if (!this.editor) return Promise.reject(new Error("<docxtor-editor> is not connected"));
    return this.editor.saveInSteps();
  }

  private async loadUrl(src: string) {
    try {
      const res = await fetch(src);
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      await this.load(await res.arrayBuffer(), this.getAttribute("name") ?? decodeURIComponent(src.split("/").pop() ?? "document.docx"));
    } catch (e) {
      this.dispatchEvent(new CustomEvent("docxtor-error", { detail: { message: t("Couldn't open {src}: {reason}", { src, reason: (e as Error).message }) } }));
    }
  }
}

if (!customElements.get("docxtor-editor")) customElements.define("docxtor-editor", DocxtorEditorElement);

declare global {
  interface HTMLElementTagNameMap {
    "docxtor-editor": DocxtorEditorElement;
  }
}
