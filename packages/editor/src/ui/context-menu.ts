// The right-click menu in the editing area (the pages and the desk around them): clipboard, link,
// formatting, table actions, the assistant, and whatever extensions add to the `context` menu.
// Shift+right-click still opens the browser's own menu; the last line of the menu says so. Other parts of
// the editor (the spelling checker) can put lines at the top of the menu for what was clicked.
import { t, type Command, type ExtensionHost } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { caretRangeAt, domToModel } from "../editor/selection";
import type { StoryZone } from "../editor/layout";
import { h, popover, toast } from "./dom";
import { menuIcon } from "./menu-bar";
import { isLandscape } from "./page-setup";

export interface ContextMenuDeps {
  editor: Editor;
  host: ExtensionHost;
  /** Ask the AI about the selection (when the host has an assistant). */
  ask: ((prompt?: string) => void) | null;
  /** Called as the menu opens (to hide other floating tools). */
  onOpen?: () => void;
  /** Lines to put above the usual ones for what was clicked (a point on screen), or at the caret when the menu was opened from the keyboard (null). */
  leading?: () => ContextSource[];
}

/** One line of the right-click menu. */
export interface ContextItem {
  label: string;
  hint?: string;
  icon?: string;
  disabled?: boolean;
  checked?: boolean;
  /** Set in bold, like the suggestions in Google Docs. */
  strong?: boolean;
  /** A line of explanation, not a choice: shown as wrapped text, skipped by the arrow keys (the grammar message). */
  note?: boolean;
  run: () => unknown;
}
type Item = ContextItem;

/** Groups of lines for a right-click (or Menu key) at `at`; empty when it has nothing to add there. */
export type ContextSource = (at: { x: number; y: number } | null) => Promise<ContextItem[][]> | ContextItem[][];

export function attachContextMenu({ editor: ed, host, ask, onOpen, leading }: ContextMenuDeps): () => void {
  const ac = new AbortController();
  const fromCommand = (c: Command, label = c.title): Item => ({ label, hint: host.shortcut(c.id), icon: host.icon(c.icon), run: () => host.execute(c.id) });

  const items = (): Item[][] => {
    const ro = ed.readOnly;
    const empty = ed.collapsed();
    const groups: Item[][] = [];
    // The same commands as the menu bar. `all` keeps unavailable ones, greyed out; otherwise only
    // the ones that apply here are listed.
    const commands = (ids: string[], all = false) =>
      ids.flatMap((id) => {
        const c = host.command(id);
        if (!c || (!all && !host.enabled(c))) return [];
        return [{ ...fromCommand(c), disabled: !host.enabled(c), checked: !!c.toggled?.() }];
      });
    groups.push(commands(["edit.cut", "edit.copy", "edit.paste", "edit.pastePlain", "edit.pasteSpecial", "edit.delete"], true));
    groups.push(commands(["edit.copyMarkdown", "edit.pasteMarkdown"], true));
    const link = ed.linkAt();
    const edit: Item[] = [];
    if (link) {
      edit.push({ label: link.href.startsWith("#") ? t("Go to link target") : t("Open link"), icon: host.icon("link"), run: () => ed.followLink() });
      const editLink = host.command("insert.link");
      if (!ro && editLink) edit.push(fromCommand(editLink, t("Edit link…")));
      if (!ro) edit.push({ label: t("Remove link"), icon: host.icon("close"), run: () => ed.removeLink() });
    } else {
      const add = host.command("insert.link");
      if (add && host.enabled(add)) edit.push(fromCommand(add, t("Link…")));
    }
    groups.push(edit);
    groups.push(commands(["format.copyFormat", "format.pasteFormat", "format.clear", "edit.selectAllMatching"]));
    // In a numbered list: start the numbers again, or carry on from the list above.
    groups.push(commands(["format.restartNumbering", "format.continueNumbering", "format.setNumberingValue"]));
    // On a checklist item: tick it or untick it.
    const done = host.command("format.toggleChecked");
    if (done && host.enabled(done)) groups.push([fromCommand(done, done.toggled?.() ? t("Mark as not done") : t("Mark as done"))]);
    groups.push(commands(["chart.edit", "image.altText", "image.replace"]));
    // A shape or text box: its text, colours and wrapping.
    groups.push(commands(["shape.editText", "shape.fill", "shape.outline", "shape.format"]));
    groups.push(commands(["shape.wrap.inline", "shape.wrap.square", "shape.wrap.topBottom", "shape.wrap.behind", "shape.wrap.front"]));
    groups.push(commands(["shape.altText", "shape.delete"]));
    // A caption for the selected image or the table the click is in.
    const caption = host.command("insert.caption");
    const target = ed.captionTarget();
    if (caption && host.enabled(caption) && (target?.kind === "image" || target?.kind === "table")) groups.push([fromCommand(caption, t("Insert caption…"))]);
    groups.push(commands(["table.rowAbove", "table.rowBelow", "table.columnLeft", "table.columnRight"]));
    groups.push(commands(["table.merge", "table.split", "table.splitCells", "table.distributeRows", "table.distributeColumns", "table.headerRow", "table.design"]));
    groups.push(commands(["table.sort"]));
    groups.push(commands(["table.deleteRow", "table.deleteColumn", "table.deleteTable"]));
    // The properties and the borders of the table, where the right-click was in one.
    const tableProps = host.command("table.properties");
    groups.push(commands(["table.properties", ...(tableProps && host.enabled(tableProps) ? ["format.bordersShading"] : [])]));
    if (ask && !ro) groups.push([{ label: empty ? t("Ask AI") : t("Ask AI about the selection"), hint: host.shortcut("view.assistant"), icon: host.icon("caret"), run: () => ask() }]);
    // Google Docs offers to turn the page from here.
    const turn = host.command(isLandscape(ed) ? "format.portrait" : "format.landscape");
    if (turn && host.enabled(turn)) groups.push([fromCommand(turn, isLandscape(ed) ? t("Change page to portrait") : t("Change page to landscape"))]);
    // Extensions' items, grouped as they asked (only the ones that apply here).
    for (const g of host.menu("context")) groups.push(g.filter((e) => e.enabled && e.command).map((e) => ({ label: e.title, hint: e.shortcut, icon: e.icon, checked: e.checked, run: () => host.run(e) })));
    return groups.filter((g) => g.length);
  };

  const storyItems = (zone: StoryZone, page: number): Item[][] => [
    [
      {
        label: zone === "header" ? t("Edit header") : zone === "footer" ? t("Edit footer") : zone === "endnotes" ? t("Edit endnotes") : t("Edit footnotes"),
        hint: t("Double-click"),
        run: () => ed.enterStory(zone, page),
      },
    ],
  ];

  const open = (at: { x: number; y: number }, keyboard: boolean, groups = items()) => {
    onOpen?.();
    const list = h("div", { class: "ctx-menu" });
    // Keep the document's selection while the menu is used with the mouse.
    list.addEventListener("mousedown", (e) => e.preventDefault());
    let close = () => {};
    for (const [gi, group] of groups.entries()) {
      if (gi) list.append(h("div", { class: "menu-sep", role: "separator" }));
      for (const it of group) {
        if (it.note) {
          list.append(h("div", { class: "menu-note" }, it.label));
          continue;
        }
        const b = h(
          "button",
          { class: "menu-item", type: "button", role: "menuitem", disabled: !!it.disabled },
          menuIcon(host, it.icon, !!it.checked),
          h("span", { class: it.strong ? "menu-label strong" : "menu-label" }, it.label),
          it.hint ? h("span", { class: "hint" }, it.hint) : null,
        );
        b.addEventListener("click", () => {
          close();
          // Commands read the selection from the document, so give it back focus first.
          ed.focus();
          Promise.resolve(it.run()).catch((err) => toast(String((err as Error)?.message ?? err)));
        });
        list.append(b);
      }
    }
    list.append(h("div", { class: "menu-hint" }, ed.spell.active ? t("Shift+right-click for your browser's menu") : t("Shift+right-click for your browser's menu and spelling suggestions")));
    list.addEventListener("keydown", (e) => {
      const buttons = Array.from(list.querySelectorAll<HTMLButtonElement>(".menu-item:not(:disabled)"));
      const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
      const go = (n: number) => {
        e.preventDefault();
        buttons[(n + buttons.length) % buttons.length]?.focus();
      };
      if (e.key === "ArrowDown") go(i + 1);
      else if (e.key === "ArrowUp") go(i < 0 ? -1 : i - 1);
      else if (e.key === "Home") go(0);
      else if (e.key === "End") go(-1);
      else if (e.key === "Tab") e.preventDefault();
    });
    close = popover(at, list, { onClose: () => keyboard && ed.focus() });
    // The menu's name belongs on the menu itself (the popover's wrapper), not on a plain box inside it.
    list.closest(".pop")?.setAttribute("aria-label", t("Document actions"));
    if (keyboard) list.querySelector<HTMLButtonElement>(".menu-item:not(:disabled)")?.focus();
  };

  /** What the other parts of the editor add above the usual lines. */
  const lead = async (at: { x: number; y: number } | null): Promise<Item[][]> => {
    const out: Item[][] = [];
    for (const source of leading?.() ?? []) {
      try {
        out.push(...(await source(at)).filter((g) => g.length));
      } catch {
        /* a source that fails adds nothing */
      }
    }
    return out;
  };

  const atCaret = () => {
    const s = window.getSelection();
    const range = s?.rangeCount ? s.getRangeAt(0) : null;
    const r = range ? (range.getClientRects()[0] ?? range.getBoundingClientRect()) : null;
    if (r && (r.width || r.height)) return { x: r.left, y: r.bottom };
    // A caret in an empty spot has no box of its own: use its paragraph.
    const p = ed.sel ? ed.paraElement(ed.sel.focus.p) : null;
    const b = (p ?? ed.root).getBoundingClientRect();
    return { x: b.left + 8, y: Math.min(b.bottom, b.top + 24) };
  };
  /**
   * A right-click puts the caret where it lands, unless it lands in what is selected, so the menu's commands (Insert caption, Comment,
   * the table commands) work on what was clicked. Chrome and Safari do this themselves; Firefox leaves the caret where it was.
   */
  const caretToPointer = (x: number, y: number, target: HTMLElement) => {
    if (ed.readOnly || ed.extra.ranges.length || !target.closest(".fp") || target.closest(".atom, .obj-frame")) return;
    const sel = window.getSelection();
    if (sel && sel.rangeCount && !sel.isCollapsed) {
      for (const r of Array.from(sel.getRangeAt(0).getClientRects())) if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return;
    }
    const at = caretRangeAt(x, y);
    const pos = at && ed.flow.contains(at.startContainer) ? domToModel(ed.flow, at.startContainer, at.startOffset) : null;
    if (!pos) return;
    const now = ed.sel;
    if (now && ed.collapsed() && now.focus.p === pos.p && now.focus.o === pos.o) return;
    ed.select(pos);
  };
  // Shift+F10 and the Menu key open it at the caret. Some platforms also send a contextmenu event
  // for them; it's ignored right after.
  let byKey = 0;
  ed.canvas.addEventListener(
    "keydown",
    (e) => {
      if (!((e.shiftKey && e.key === "F10") || e.key === "ContextMenu") || !ed.engine || !ed.root.contains(e.target as Node)) return;
      e.preventDefault();
      byKey = Date.now();
      const at = atCaret();
      void lead(null).then((extra) => open(at, true, [...extra, ...items()]));
    },
    { signal: ac.signal },
  );
  ed.canvas.addEventListener(
    "contextmenu",
    (e) => {
      if (Date.now() - byKey < 400) return e.preventDefault();
      if (e.shiftKey || !ed.engine) return;
      const target = e.target as HTMLElement;
      // A contextmenu event from the keyboard opens at the caret instead of the pointer.
      if (e.button !== 2 || (e.clientX === 0 && e.clientY === 0)) {
        e.preventDefault();
        const at = atCaret();
        void lead(null).then((extra) => open(at, true, [...extra, ...items()]));
        return;
      }
      // Anywhere in the editing area: the pages and the desk around them. (The ruler and the
      // tools floating over the pages keep their own behaviour.)
      if (target.closest(".ruler")) return;
      const inStory = !!ed.story && ed.root.contains(target);
      e.preventDefault();
      // A right-click on one of several selected ranges keeps all of them (the menu is for the whole selection). Chrome and Firefox leave
      // the selection alone when the mouse-down is cancelled; Safari selects the word under the pointer just before this event whatever
      // the mouse-down did, which the editor would read as a new selection and drop the other ranges. Its selection event comes after
      // this one, so writing the real selection back now makes it a no-op.
      if (ed.multi && ed.extra.hit(e.clientX, e.clientY)) ed.focus();
      if (!inStory) caretToPointer(e.clientX, e.clientY, target);
      // A header, footer or footnotes area that isn't being edited offers to edit it.
      const onPage = !!target.closest(".flow, .sheet, .stage");
      const z = ed.zoneAt(e.clientY, e.clientX);
      const at = { x: e.clientX, y: e.clientY };
      const base = () => (inStory || !onPage || z.zone === "body" || ed.readOnly ? items() : storyItems(z.zone, z.page));
      const here = inStory || !onPage || z.zone === "body" || ed.readOnly;
      void (here ? lead(at) : Promise.resolve([] as Item[][])).then((extra) => open(at, false, [...extra, ...base()]));
    },
    { signal: ac.signal },
  );
  return () => ac.abort();
}
