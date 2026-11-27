// Bookmarks: the small popover at a bookmark (rename, copy its link, remove) and the Bookmarks dialog
// (every bookmark in the document, with Go to, Rename and Remove).
import type { BookmarkInfo } from "@docxtor/core";
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal, popover, toast } from "./dom";

/** Where a bookmark outside the text is, in words: "Header", "First page footer", "Footnote 3". Empty for one in the text. */
export function bookmarkWhere(ed: Editor, b: BookmarkInfo): string {
  const st = b.story;
  if (!st) return "";
  const info = ed.pageInfo;
  if (st.kind === "header" || st.kind === "footer") {
    const header = st.kind === "header";
    const variant = (info.sections ?? []).map((s) => Object.entries((header ? s.headers : s.footers) ?? {}).find(([, part]) => part === st.part)?.[0]).find(Boolean) ?? "default";
    if (variant === "first") return header ? t("First page header") : t("First page footer");
    if (variant === "even") return header ? t("Even page header") : t("Even page footer");
    return header ? t("Header") : t("Footer");
  }
  const notes = st.kind === "footnote" ? info.notes : info.endnotes;
  const label = notes?.find((n) => n.id === st.noteId)?.label ?? "";
  return st.kind === "footnote" ? t("Footnote {label}", { label }) : t("Endnote {label}", { label });
}

/** The text a link to a bookmark has: `#Name`. */
const linkTo = (name: string) => `#${name}`;

async function copyLink(name: string) {
  try {
    await navigator.clipboard.writeText(linkTo(name));
    toast(t("Link copied"));
  } catch {
    toast(t("Couldn't copy the link"));
  }
}

/** A name field with its message line: `error(text)` shows the engine's words under the field. */
function nameField(value: string, label: string, readOnly: boolean) {
  const input = h("input", { type: "text", class: "bm-input", value, spellcheck: "false", autocomplete: "off", "aria-label": label, disabled: readOnly });
  const message = h("p", { class: "bm-error", role: "alert", hidden: true });
  const error = (text: string) => {
    message.textContent = text;
    message.hidden = !text;
    input.setAttribute("aria-invalid", String(!!text));
  };
  input.addEventListener("input", () => error(""));
  return { input, message, error };
}

/**
 * The popover at a bookmark: its name (editable; the engine's message shows under it when the name
 * isn't allowed), Copy link and Remove. `anchor` is the marker in the margin, or a point.
 */
export function openBookmarkPopover(ed: Editor, name: string, anchor: HTMLElement | { x: number; y: number }) {
  let current = name;
  const field = nameField(name, t("Bookmark name"), ed.readOnly);
  const rename = h("button", { class: "btn primary", type: "button", disabled: true }, t("Rename"));
  const copy = h("button", { class: "btn ghost", type: "button" }, t("Copy link"));
  const remove = h("button", { class: "btn ghost", type: "button", disabled: ed.readOnly }, t("Remove"));
  const box = h(
    "div",
    { class: "bm-pop" },
    h("div", { class: "menu-title" }, t("Bookmark")),
    field.input,
    field.message,
    h("div", { class: "bm-actions" }, copy, remove, rename),
  );
  const close = popover(anchor, box, { onClose: () => ed.focus() });
  const commit = () => {
    const next = field.input.value.trim();
    if (!next || next === current) return;
    if (ed.renameBookmark(current, next, field.error)) close();
  };
  field.input.addEventListener("input", () => (rename.disabled = ed.readOnly || !field.input.value.trim() || field.input.value.trim() === current));
  field.input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    }
  });
  rename.addEventListener("click", commit);
  copy.addEventListener("click", () => {
    void copyLink(current);
    close();
  });
  remove.addEventListener("click", () => {
    close();
    void ed.command("bookmark.remove", current);
  });
  setTimeout(() => {
    field.input.focus();
    field.input.select();
  });
}

/** Add a bookmark at the selection and show its popover. */
export function addBookmark(ed: Editor) {
  const name = ed.addBookmark();
  if (!name) return;
  const marker = ed.bookmarkMarker(name);
  if (marker) return openBookmarkPopover(ed, name, marker);
  // Not drawn in the margin (a bookmark in a header, footer or note): anchor it to the caret.
  const rect = window.getSelection()?.rangeCount ? window.getSelection()!.getRangeAt(0).getBoundingClientRect() : null;
  openBookmarkPopover(ed, name, { x: rect?.left ?? 120, y: rect?.bottom ?? 120 });
}

/** The Bookmarks dialog. */
export function openBookmarks(ed: Editor) {
  const list = h("div", { class: "bm-list" });
  const done = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const dialog = h(
    "div",
    { class: "dialog bm-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Bookmarks") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Bookmarks")), h("p", { class: "sub" }, t("Link to a bookmark from anywhere in the document."))),
    h("div", { class: "dialog-body" }, list),
    h("div", { class: "dialog-actions" }, done),
  );
  const close = modal(dialog, () => ed.focus());
  done.addEventListener("click", close);
  let renaming: string | null = null;

  const row = (b: BookmarkInfo): HTMLElement => {
    if (renaming === b.name) {
      const field = nameField(b.name, t("Bookmark name"), false);
      const save = h("button", { class: "btn primary", type: "button" }, t("Rename"));
      const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
      const commit = () => {
        const next = field.input.value.trim();
        if (!next || next === b.name) {
          renaming = null;
          return render();
        }
        if (ed.renameBookmark(b.name, next, field.error)) {
          renaming = null;
          render();
        }
      };
      save.addEventListener("click", commit);
      cancel.addEventListener("click", () => {
        renaming = null;
        render();
      });
      field.input.addEventListener("keydown", (e) => {
        if (e.key === "Enter") commit();
        else if (e.key === "Escape") {
          e.stopPropagation();
          renaming = null;
          render();
        }
      });
      setTimeout(() => {
        field.input.focus();
        field.input.select();
      });
      return h("div", { class: "bm-row editing" }, h("div", { class: "bm-edit" }, field.input, field.message), h("div", { class: "bm-actions" }, cancel, save));
    }
    const reachable = ed.canSelectBookmark(b);
    const go = h("button", { class: "btn", type: "button", disabled: !reachable, title: reachable ? "" : t("No page shows this bookmark.") }, t("Go to"));
    const rename = h("button", { class: "btn ghost", type: "button", disabled: ed.readOnly }, t("Rename"));
    const remove = h("button", { class: "btn ghost", type: "button", disabled: ed.readOnly }, t("Remove"));
    go.addEventListener("click", () => {
      close();
      ed.selectBookmark(b);
    });
    rename.addEventListener("click", () => {
      renaming = b.name;
      render();
    });
    remove.addEventListener("click", () => {
      void ed.command("bookmark.remove", b.name).then(() => render());
    });
    return h(
      "div",
      { class: "bm-row" },
      h(
        "div",
        { class: "bm-info" },
        h("span", { class: "bm-head" }, h("span", { class: "bm-name" }, b.name), b.story ? h("span", { class: "bm-where" }, bookmarkWhere(ed, b)) : null),
        b.text ? h("span", { class: "bm-text" }, b.text) : null,
      ),
      h("div", { class: "bm-actions" }, go, rename, remove),
    );
  };

  function render() {
    const all = ed.bookmarks();
    if (!all.length) {
      list.replaceChildren(h("p", { class: "bm-empty" }, t("No bookmarks yet. Select some text and choose Insert, Bookmark, Add bookmark.")));
      return;
    }
    list.replaceChildren(...all.map(row));
  }
  render();
  done.focus();
}
