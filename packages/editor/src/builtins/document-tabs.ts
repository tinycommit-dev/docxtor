// Document tabs, as in Google Docs: Insert, Tab, the next and previous tab, and Print all tabs (Print prints the open
// one). The list of tabs is in the navigation pane (ui/doc-tabs.ts); every change is a `tab` op.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { promptDialog } from "../ui/dom";
import type { Internals } from "./index";

export function documentTabs({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.documentTabs",
    name: msg("Document tabs"),
    description: msg("Split a document into tabs you switch between, as in Google Docs. Word shows the tabs one after another."),
    activate(ctx) {
      const editable = () => !!fe.engine && !fe.readOnly && !e.story;
      // The tabs are shown by the pane that has the keyboard: each pane of a split has its own open tab.
      const several = () => !!fe.engine && fe.editor.tabs.tabs.length > 1;
      const step = (by: number) => {
        const pane = fe.editor;
        const { tabs, active } = pane.tabs;
        const i = tabs.findIndex((x) => x.id === active);
        const next = tabs[(Math.max(0, i) + by + tabs.length) % tabs.length];
        if (!next) return;
        // Showing a tab is not an edit, so it works in a read-only document too.
        pane.openTab(next.id);
        const el = pane.root.querySelector<HTMLElement>(`[data-id="${next.start}"]`);
        const p = el?.dataset.p ?? el?.querySelector<HTMLElement>("[data-p]")?.dataset.p;
        if (p) pane.select({ p: Number(p), o: 0 });
      };
      ctx.commands.register(
        "insert.tab",
        async () => {
          const name = await promptDialog({ title: t("Add tab"), label: t("Tab name"), value: t("Untitled tab"), ok: t("Add") });
          const pane = fe.editor;
          if (!name) return pane.focus();
          pane.exec({ op: "tab", action: "add", name, ...(pane.tabs.tabs.length ? {} : { first_name: t("Tab {n}", { n: 1 }) }) });
          step(0);
          pane.focus();
        },
        { title: t("Tab…"), category: t("Insert"), when: editable, icon: "pageBreak" },
      );
      // The Delete button of a tab in the navigation pane: it names the tab and its name (for the question).
      ctx.commands.register("tabs.delete", (id) => (typeof id === "string" ? !!e.exec({ op: "tab", action: "delete", id }) : false), { title: t("Delete tab"), category: t("Insert"), when: editable, palette: false });
      ctx.commands.register("view.nextTab", () => step(1), { title: t("Next tab"), category: t("View"), when: several, keybinding: "Mod+Alt+PageDown" });
      ctx.commands.register("view.previousTab", () => step(-1), { title: t("Previous tab"), category: t("View"), when: several, keybinding: "Mod+Alt+PageUp" });
      ctx.commands.register("file.printAllTabs", () => e.printAllTabs(), { title: t("Print all tabs…"), category: t("File"), when: several, icon: "print" });
      ctx.menus.register({ menu: "insert", group: "3_page", command: "insert.tab" });
      ctx.menus.register({ menu: "file", group: "7_print", command: "file.printAllTabs" });
    },
  });
}
