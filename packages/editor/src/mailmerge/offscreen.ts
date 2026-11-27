// An editor out of sight, for laying out a document without opening it: a merged document to print or write as a PDF,
// the pages of a gallery's designs.
import type { DocxtorEditor, DocxtorEditorOptions } from "../docxtor-editor";
import { overlayLayer } from "../ui/dom";

export interface Offscreen {
  fe: DocxtorEditor;
  /** Take the editor away. Safe to call twice. */
  dispose(): Promise<void>;
}

/** A second editor of the same kind (made from the first one's class, which imports this file), off to the side of the page. */
export async function offscreenEditor(s: { x: { fe: DocxtorEditor } }): Promise<Offscreen> {
  const DocxtorEditor = s.x.fe.constructor as new (options: DocxtorEditorOptions) => DocxtorEditor;
  const host = document.createElement("div");
  host.style.cssText = "position:fixed;left:-30000px;top:0;width:1400px;height:1000px;overflow:hidden;pointer-events:none";
  host.dataset.mmOffscreen = "";
  overlayLayer();
  document.body.append(host);
  const fe = new DocxtorEditor({ toolbar: false, menuBar: false, toolbarAsk: false, fonts: false, spelling: false, autoformat: false });
  let gone = false;
  const dispose = async () => {
    if (gone) return;
    gone = true;
    // The extensions go first, while the document they look at is still there.
    await fe.extensions.dispose();
    fe.destroy();
    host.remove();
  };
  try {
    fe.mount(host);
    await fe.ready;
  } catch (e) {
    await dispose();
    throw e;
  }
  return { fe, dispose };
}
