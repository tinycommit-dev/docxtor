// Citing sources, as an extension: Insert, Citation and Insert, Bibliography, and the keyboard way into the
// popover on a citation. They need only the engine. The Citations panel (adding sources by hand, from a DOI
// or from BibTeX) belongs to the app, because it asks other servers and opens files; when it is installed,
// Insert, Citation opens it, and when it isn't, Insert, Citation offers a plain list of the sources.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { openCitationAtCaret, pickSourceToCite } from "../ui/citations";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

export function references({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.references",
    name: msg("Citations and bibliography"),
    description: msg("Cite sources in the text and list them in a bibliography: Insert, Citation, Insert, Bibliography and the popover on a citation."),
    activate(ctx) {
      const editable = () => !!fe.engine && !fe.readOnly;
      const inBody = () => editable() && !e.story;

      // The five styles the engine formats itself. Other extensions add CSL styles beside them.
      ctx.citations.styles.register({ id: "apa", label: t("APA 7th edition") });
      ctx.citations.styles.register({ id: "mla", label: t("MLA 9th edition") });
      ctx.citations.styles.register({ id: "chicago", label: t("Chicago 17th edition (author-date)") });
      ctx.citations.styles.register({ id: "ieee", label: t("IEEE") });
      ctx.citations.styles.register({ id: "harvard", label: t("Harvard") });

      ctx.commands.register(
        "insert.citation",
        async () => {
          // The app's panel, when it has one, lists the sources with a search box that cites on Enter.
          if (fe.extensions.command("citations.find")) {
            const ran = await fe.extensions.execute("citations.find");
            if (ran.status !== "unavailable") return;
          }
          await pickSourceToCite(e);
        },
        { title: t("Citation…"), category: t("Insert"), when: editable, icon: COMMAND_ICONS["insert.citation"] },
      );
      // The reference list needs no input; the assistant has its own tool for it too.
      ctx.commands.register(
        "insert.bibliography",
        () => void e.insertBibliography(),
        { title: t("Bibliography"), category: t("Insert"), when: inBody, icon: COMMAND_ICONS["insert.bibliography"], ai: true },
      );
      // The Remove button of the popover on a citation: it names the citation.
      ctx.commands.register("citation.remove", (cite) => (cite && typeof cite === "object" ? e.removeCitation(cite as Parameters<typeof e.removeCitation>[0]) : false), { title: t("Remove citation"), category: t("Insert"), when: editable, palette: false });
      // The popover is reached by clicking a citation; this is the way in from the keyboard.
      ctx.commands.register(
        "citation.edit",
        () => void openCitationAtCaret(e, fe.citationPopoverOptions()),
        { title: t("Edit citation…"), category: t("Insert"), when: () => editable() && !!e.citationAtCaret(), icon: COMMAND_ICONS["citation.edit"] },
      );
    },
  });
}
