// Insert, Pages and Insert, Header and Footer as Word has them: the cover page gallery (Docxtor's designs and the ones the
// person saved), Remove current cover page, Save selection to cover page gallery, Blank page, and the header and footer
// galleries with Edit and Remove. Cover pages others make plug in as snippets in the "Cover pages" gallery of any
// snippet store.
import type { Op } from "@docxtor/core";
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { openPageGallery, galleryLabels } from "../ui/page-gallery";
import { openSaveQuickPart } from "../ui/quick-parts";
import { h, toast } from "../ui/dom";
import type { Internals } from "./index";

const ICONS: Record<string, string> = {
  coverPage: '<rect x="5" y="3" width="14" height="18" rx="1.5"/><path d="M5 9h14"/><path d="M8.5 13h7M8.5 16h4"/>',
  blankPage: '<path d="M13 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V9z"/><path d="M13 3v6h6"/>',
  headerGallery: '<rect x="4" y="3" width="16" height="18" rx="1.5"/><path d="M4 8h16"/><path d="M7 5.5h6"/>',
  footerGallery: '<rect x="4" y="3" width="16" height="18" rx="1.5"/><path d="M4 16h16"/><path d="M7 18.5h6"/>',
};
const wrap = (body: string) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;

export function pageParts(x: Internals): Extension {
  return defineExtension({
    id: "docxtor.pageParts",
    name: msg("Cover pages, headers and footers"),
    description: msg("Insert, Cover page and Blank page, and ready-made headers and footers, in the document's colours and fonts."),
    activate(ctx) {
      const { fe, editor: e } = x;
      for (const [id, body] of Object.entries(ICONS)) ctx.icons.register({ id, svg: wrap(body) });
      const editable = () => !!e.engine && !e.readOnly;
      const inBody = () => editable() && !e.story;
      const cat = t("Insert");
      const cmd = (id: string, title: string, run: (...args: unknown[]) => unknown, o: { when?: () => boolean; icon?: string; keybinding?: string } = {}) =>
        ctx.commands.register(id, run, { title, category: cat, when: o.when ?? inBody, icon: o.icon, keybinding: o.keybinding });

      // The caret goes to the cover's first box, which brings the first page into view.
      const insertCover = (op: Op) => {
        if (e.exec(op)) e.focus();
      };

      const removeButton = (label: string, run: () => void) => {
        const b = h("button", { class: "btn", type: "button" }, label);
        b.addEventListener("click", run);
        return b;
      };

      cmd("insert.coverPage", t("Cover page…"), () => {
        const extra = e.engine?.hasCoverPage() ? [removeButton(t("Remove current cover page"), () => void ctx.commands.execute("insert.removeCoverPage"))] : [];
        void openPageGallery(
          fe,
          fe.extensions,
          "cover",
          (c) => {
            const labels = galleryLabels(e);
            if (c.design) insertCover({ op: "insert_cover_page", design: c.design, labels });
            else if (c.part) {
              const fragment = c.part.snippet ? c.part.snippet.fragment : c.part.block !== undefined ? e.engine?.buildingBlockFragment(c.part.block) : null;
              if (fragment) insertCover({ op: "insert_cover_page", fragment, labels });
            }
          },
          extra,
        );
      }, { icon: "coverPage" });
      cmd("insert.removeCoverPage", t("Remove current cover page"), () => {
        if (e.exec({ op: "remove_cover_page" })) toast(t("The cover page was removed."));
      }, { when: () => inBody() && !!e.engine?.hasCoverPage() });
      cmd("insert.saveCoverPage", t("Save selection to cover page gallery…"), () => openSaveQuickPart(e, fe.extensions, "Cover pages"), { when: () => inBody() && !e.collapsed() });
      // An empty paragraph on a page of its own, as Word's Blank page: the text after the caret starts the page after it.
      // Breaks are paragraphs that start a page, as Page break makes them; one undo step.
      cmd("insert.blankPage", t("Blank page"), () => {
        const r = e.range();
        if (!r) return;
        const ops: Op[] = e.collapsed() ? [] : [{ op: "delete", from: r[0], to: r[1] }];
        ops.push({ op: "split_paragraph", at: r[0] });
        if (!e.exec(ops)) return;
        const blank = e.sel?.focus.p;
        if (blank === undefined) return;
        e.exec({ op: "split_paragraph", at: { p: blank, o: 0 } }, { merge: true });
        const after = e.sel?.focus.p;
        if (after === undefined) return;
        e.exec({ op: "format_paragraphs", ids: [blank, after], format: { pageBreakBefore: true } }, { merge: true, select: { p: blank, o: 0 } });
      }, { icon: "blankPage" });

      // Whether the caret's section has a header (or footer) of its own to remove: not one it shows from the section
      // before. In the strip over a header being edited, there is one.
      const hasOwn = (kind: "header" | "footer") => {
        const s = e.caretSectionInfo() as { headers?: Record<string, unknown>; footers?: Record<string, unknown>; linkedHeaders?: string[]; linkedFooters?: string[] };
        const parts = kind === "header" ? s.headers : s.footers;
        const linked = (kind === "header" ? s.linkedHeaders : s.linkedFooters) ?? [];
        return !!parts?.default && !linked.includes("default");
      };
      for (const kind of ["header", "footer"] as const) {
        const Kind = kind === "header" ? "Header" : "Footer";
        const section = () => e.caretParagraph();
        cmd(`insert.${kind}Gallery`, kind === "header" ? t("Header…") : t("Footer…"), () => {
          const extra = [
            removeButton(kind === "header" ? t("Edit header") : t("Edit footer"), () => void ctx.commands.execute(`insert.${kind}`)),
            removeButton(kind === "header" ? t("Remove header") : t("Remove footer"), () => void ctx.commands.execute(`insert.remove${Kind}`)),
          ];
          void openPageGallery(fe, fe.extensions, kind, (c) => {
            if (!c.design) return;
            const sec = section();
            e.exec({ op: "insert_page_part", kind, design: c.design, labels: galleryLabels(e), ...(sec !== undefined ? { section: sec } : {}) });
            e.focus();
          }, extra);
        }, { icon: kind === "header" ? "headerGallery" : "footerGallery" });
        // The strip over a header or footer being edited names the one to remove (its variant and the section that owns it) and leaves the editing first.
        cmd(`insert.remove${Kind}`, kind === "header" ? t("Remove header") : t("Remove footer"), (where) => {
          const at = (where ?? {}) as { variant?: string; section?: number; leaveStory?: boolean };
          if (at.leaveStory) e.exitStory(false);
          const sec = where ? at.section : section();
          return !!e.exec({ op: "remove_header_footer", kind, ...(at.variant ? { variant: at.variant } : {}), ...(sec !== undefined ? { section: sec } : {}) }, at.leaveStory ? { select: null } : {});
        }, { when: () => editable() && (!!e.story || hasOwn(kind)) });
      }

      // -- the menu ------------------------------------------------------------------------------
      const item = (menu: string, group: string, command: string, title?: string) => ctx.menus.register({ menu, group, command, title });
      ctx.menus.register({ menu: "insert", group: "3_page", submenu: "insert/pages", title: t("Pages"), icon: "coverPage" });
      for (const c of ["coverPage", "removeCoverPage", "saveCoverPage"]) item("insert/pages", "1", `insert.${c}`);
      item("insert/pages", "2", "insert.blankPage");
      item("insert/pages", "2", "insert.pageBreak");
      item("insert/page", "0", "insert.headerGallery");
      item("insert/page", "0", "insert.removeHeader");
      item("insert/page", "0", "insert.footerGallery");
      item("insert/page", "0", "insert.removeFooter");
    },
  });
}
