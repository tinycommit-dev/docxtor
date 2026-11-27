// The Citations panel as an extension of its own. Where sources come from (a DOI at doi.org, BibTeX, Zotero) and
// the citation styles are contributions of other extensions (`ctx.citations.sources`, `ctx.citations.styles`), so
// each can be turned off under Extensions, Manage extensions. Citing, editing a citation and the bibliography
// work without the panel (they live in the editor, see packages/editor/src/builtins/citations.ts); with it off,
// Insert, Citation offers a plain list of the document's sources.
import { defineExtension, msg, t, type Extension } from "@docxtor/editor";
import type { CitationsHost } from "../../citations-host";
import { CitationsPanel } from "./panel";

export { cslStyles } from "./styles";
export { bibtexSource, doiSource } from "./sources";
export { userStyles } from "./user-styles";

export function citations(app: CitationsHost): Extension {
  return defineExtension({
    id: "docxtor.citations",
    name: msg("Citations panel"),
    description: msg("Add sources by hand or from the places other extensions offer (a DOI, BibTeX, Zotero), cite them, choose a citation style and insert the bibliography."),
    activate(ctx) {
      const panel = new CitationsPanel(app, (command, ...args) => ctx.commands.execute(command, ...args));
      const doc = () => !!app.fe.engine;
      ctx.panels.register({
        id: "citations",
        title: t("Citations"),
        order: 50,
        tool: true,
        render: (el) => void el.append(panel.el),
        onShow: () => panel.update(),
      });
      ctx.subscriptions.push(ctx.document.onDidChange(() => panel.schedule()), ctx.document.onDidOpen(() => panel.update()));

      ctx.commands.register(
        "tools.citations",
        () => {
          app.showPanel("citations");
          panel.update();
        },
        { title: t("Citations"), category: t("Tools"), icon: "citation", when: doc },
      );
      // Insert, Citation (in the editor) comes here when this panel is installed: the source list, search box first.
      ctx.commands.register(
        "citations.find",
        () => {
          app.showPanel("citations");
          panel.focusSearch();
        },
        { title: t("Find a source to cite"), category: t("Insert"), palette: false, when: doc },
      );
      // The popover on a citation offers "Edit source" when this is installed.
      ctx.commands.register("citations.editSource", (tag) => typeof tag === "string" && panel.edit(tag), { title: t("Edit source"), category: t("Tools"), palette: false, when: doc });
      ctx.menus.register({ menu: "tools", group: "4_references", command: "tools.citations" });
    },
  });
}
