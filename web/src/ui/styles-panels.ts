// The Styles panel and Reveal formatting, as tool panels in the navigator. The lists they draw live in the editor
// package (`StylesList`, `RevealPanel`) so an embedding page can mount them too; this is where the app puts them,
// gives them their commands (Format, Paragraph styles) and keeps them in step with the selection.
import { defineExtension, msg, RevealPanel, StylesList, t, type Extension } from "@docxtor/editor";
import type { App } from "../main";

export function stylesPanels(app: App): Extension {
  return defineExtension({
    id: "docxtor.stylesPanels",
    name: msg("Styles and formatting panels"),
    description: msg("The Styles panel, with every style drawn as it looks, and Reveal formatting, which says where the formatting of the selected text comes from."),
    activate(ctx) {
      const editor = app.fe.editor;
      const styles = new StylesList(editor, app.fe.extensions, "panel");
      const reveal = new RevealPanel(editor);
      let showing: "styles" | "reveal" | null = null;
      let pending = 0;
      const refresh = () => {
        if (pending) return;
        pending = requestAnimationFrame(() => {
          pending = 0;
          if (showing === "styles") styles.update();
          if (showing === "reveal") reveal.update();
        });
      };
      ctx.panels.register({
        id: "styles",
        title: t("Styles"),
        order: 50,
        tool: true,
        render: (el) => void el.append(styles.el),
        onShow: () => {
          showing = "styles";
          styles.update();
        },
      });
      ctx.panels.register({
        id: "reveal",
        title: t("Reveal formatting"),
        order: 55,
        tool: true,
        render: (el) => void el.append(reveal.el),
        onShow: () => {
          showing = "reveal";
          reveal.update();
        },
      });
      ctx.subscriptions.push(ctx.document.onDidChangeSelection(refresh), ctx.document.onDidChange(refresh));
      const doc = () => !!app.engine;
      ctx.commands.register("format.stylesPane", () => app.showPanel("styles"), { title: t("Styles panel"), category: t("Format"), keybinding: "Mod+Alt+Shift+S", icon: "styles", when: doc });
      ctx.commands.register("format.revealFormatting", () => app.showPanel("reveal"), { title: t("Reveal formatting"), category: t("Format"), keybinding: "Shift+F1", icon: "reveal", when: doc });
      ctx.menus.register({ menu: "format/styles", group: "5", command: "format.stylesPane" });
      ctx.menus.register({ menu: "format/styles", group: "5", command: "format.revealFormatting" });
    },
  });
}
