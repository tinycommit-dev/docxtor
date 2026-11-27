// What the citations panel and the source library ask of the app that hosts them (the app satisfies it; the citations code never imports the app).
import type { DocxtorEditor, Editor } from "@docxtor/editor";
import type { Sidebar } from "./ui/sidebar";

export interface CitationsHost {
  /** The editor component: its engine, extensions and read-only state. */
  fe: DocxtorEditor;
  /** The editing surface of the open document. */
  editor: Editor;
  sidebar: Sidebar;
  showPanel(id: string): void;
  download(name: string, bytes: Uint8Array, mime?: string): void;
}
