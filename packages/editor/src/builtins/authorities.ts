// References, Table of authorities (Mark citation, Insert table of authorities, Update table) and the table of
// contents' Add text (which level a paragraph has in the table: a Heading style, or body text), as Word has them.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { openMarkCitation, openToaDialog, updateToa } from "../ui/toa";
import { toast } from "../ui/dom";
import type { Internals } from "./index";

const ICONS: Record<string, string> = {
  markCitation: '<path d="M6 4h9l3 3v13H6z"/><path d="M9 10h6M9 13h6M9 16h3"/><path d="M15 4v3h3"/>',
  authorities: '<path d="M12 4v16"/><path d="M5 8h14"/><path d="M5 8l-2 6h4zM19 8l-2 6h4z"/><path d="M8 20h8"/>',
  addText: '<path d="M4 6h10M4 11h7M4 16h10"/><path d="M17 9v8M13 13h8"/>',
};
const wrap = (body: string) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;

export function authorities({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.authorities",
    name: msg("Table of authorities"),
    description: msg("Mark the cases, statutes and rules a legal document cites, and list them by category with their page numbers."),
    activate(ctx) {
      for (const [id, body] of Object.entries(ICONS)) ctx.icons.register({ id, svg: wrap(body) });
      const editable = () => !!fe.engine && !fe.readOnly;
      const inBody = () => editable() && !e.story;
      const cat = t("References");
      ctx.commands.register("references.markCitation", () => openMarkCitation(e), { title: t("Mark citation…"), category: cat, when: inBody, icon: "markCitation", keybinding: "Alt+Shift+I" });
      ctx.commands.register("references.insertAuthorities", () => openToaDialog(e), { title: t("Insert table of authorities…"), category: cat, when: inBody, icon: "authorities" });
      ctx.commands.register(
        "references.updateAuthorities",
        () => {
          if (updateToa(e)) toast(t("The table of authorities is up to date."));
        },
        { title: t("Update table of authorities"), category: cat, when: () => inBody() && !!fe.engine?.toaOptions() },
      );

      // Add text: the level the caret's paragraph has in the table of contents.
      const level = () => {
        const f = e.format();
        const id = f?.style ?? "";
        // A Word file in another language names its heading styles by their own ids (`Ttulo1`): the built-in name says it.
        const m = /^Heading([1-9])$/.exec(id) ?? (id ? /^heading ([1-9])$/i.exec(e.engine.styles().find((x) => x.id === id)?.name ?? "") : null);
        return m ? Number(m[1]) : 0;
      };
      const setLevel = (n: number) => {
        const ids = e.selectedParagraphs();
        const r = e.range();
        const targets = ids.length ? ids : r ? [r[0].p] : [];
        if (!targets.length) return;
        e.exec({ op: "set_style", ids: targets, style: n ? `Heading${n}` : "Normal" }, { select: e.sel });
      };
      const levels: [number, string][] = [
        [0, t("Do not show in table of contents")],
        [1, t("Level {n}", { n: 1 })],
        [2, t("Level {n}", { n: 2 })],
        [3, t("Level {n}", { n: 3 })],
      ];
      for (const [n, title] of levels) {
        ctx.commands.register(`references.addText.${n}`, () => setLevel(n), { title, category: t("Add text"), when: inBody, toggled: () => inBody() && level() === n });
      }

      const item = (menu: string, group: string, command: string, title?: string) => ctx.menus.register({ menu, group, command, title });
      ctx.menus.register({ menu: "insert/references", group: "4", submenu: "insert/authorities", title: t("Table of authorities"), icon: "authorities" });
      item("insert/authorities", "1", "references.markCitation");
      item("insert/authorities", "1", "references.insertAuthorities");
      item("insert/authorities", "1", "references.updateAuthorities");
      ctx.menus.register({ menu: "insert/references", group: "1", submenu: "insert/addText", title: t("Add text"), icon: "addText" });
      for (const [n] of levels) item("insert/addText", "1", `references.addText.${n}`);
    },
  });
}
