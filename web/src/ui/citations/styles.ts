// Citation styles written in CSL, from the Zotero style repository (CC BY-SA 3.0; see THIRD_PARTY_NOTICES.md).
// Docxtor's own processor formats them. Each style is a separate chunk that loads when someone picks it or opens
// a document that uses it. Another extension can add a style the same way: `ctx.citations.styles.register`.
import { defineExtension, msg, t, type Extension } from "@docxtor/editor";

export function cslStyles(): Extension {
  return defineExtension({
    id: "docxtor.citations.styles",
    name: msg("Citation styles from CSL"),
    description: msg("Chicago notes and bibliography, Vancouver, Nature and AMA, formatted from CSL styles."),
    activate(ctx) {
      const reg = ctx.citations.styles;
      reg.register({ id: "chicago-nb-16", label: t("Chicago 16th edition (notes and bibliography)"), group: t("Notes"), csl: () => import("../../csl/chicago-notes-bibliography-16th-edition.csl?raw").then((m) => m.default) });
      reg.register({ id: "chicago-nb-18", label: t("Chicago 18th edition (notes and bibliography)"), group: t("Notes"), csl: () => import("../../csl/chicago-notes-bibliography.csl?raw").then((m) => m.default) });
      reg.register({ id: "vancouver", label: t("Vancouver (Elsevier)"), group: t("Numbered"), csl: () => import("../../csl/elsevier-vancouver.csl?raw").then((m) => m.default) });
      reg.register({ id: "nature", label: t("Nature"), group: t("Numbered"), csl: () => import("../../csl/nature.csl?raw").then((m) => m.default) });
      reg.register({ id: "ama", label: t("AMA 11th edition"), group: t("Numbered"), csl: () => import("../../csl/american-medical-association.csl?raw").then((m) => m.default) });
    },
  });
}
