// Clipboard actions the Edit menu and the right-click menu share. They read and write the
// clipboard themselves (the browser asks permission), so a refusal ends in a toast.
import { t } from "@docxtor/extensions";
import { toast } from "../ui/dom";
import type { Editor } from "./editor";
import { clipFromClipboard, clipFromText } from "./paste-convert";
import type { PasteChoice } from "./paste-flow";

/** Read the clipboard and insert it at the caret (needs the browser's clipboard permission). */
export async function pasteFromClipboard(ed: Editor, plain: boolean, key: string, choice?: PasteChoice) {
  try {
    const clip = plain && navigator.clipboard.readText ? clipFromText(await navigator.clipboard.readText()) : await clipFromClipboard();
    const image = clip.files.find((f) => f.type.startsWith("image/"));
    if (image && !clip.get("text/plain") && !clip.get("text/html")) return void (await ed.insertImage(image));
    await ed.pasteFlow.paste(clip, { plain, choice });
  } catch {
    toast(t("Your browser didn't let Docxtor read the clipboard. Press {key} to paste.", { key }));
  }
}

/** Put the selection on the clipboard as Markdown, for chat apps, issue trackers and notes. */
export async function copyAsMarkdown(ed: Editor) {
  try {
    await navigator.clipboard.writeText(ed.selectionMarkdown());
    toast(t("Copied as Markdown"));
  } catch {
    toast(t("Your browser didn't let Docxtor use the clipboard."));
  }
}

/** Insert the clipboard's text as Markdown at the caret, without converting it first. */
export async function pasteFromMarkdown(ed: Editor, key: string) {
  try {
    const md = await navigator.clipboard.readText();
    if (md) ed.insertMarkdownAtCaret(md);
  } catch {
    toast(t("Your browser didn't let Docxtor read the clipboard. Press {key} to paste.", { key }));
  }
}
