// Inspect document, as a built-in extension: Docxtor's own inspectors (one for each thing the engine can find) and the
// command that opens the dialog. The dialog lists every inspector that is on, through `ctx.inspectors`, so another
// extension can add a check of its own (a company's confidential markings, say) and it shows up beside these.
import type { InspectCategory } from "@docxtor/core";
import { defineExtension, msg, t, tn, type Extension, type InspectorFinding } from "@docxtor/extensions";
import { openInspect } from "../ui/inspect";
import { REVIEW_ICONS } from "../ui/review-icons";
import type { Internals } from "./index";

const KINDS: { id: Exclude<InspectCategory, "embedded">; label: string; description: string }[] = [
  { id: "properties", label: msg("Document properties and personal information"), description: msg("The author, who saved it last, the company and manager, custom properties and the preview picture.") },
  { id: "authors", label: msg("Names behind comments and tracked changes"), description: msg("Each comment and tracked change carries the name of the person who made it.") },
  { id: "comments", label: msg("Comments"), description: msg("Comments and the notes in the margin.") },
  { id: "revisions", label: msg("Tracked changes"), description: msg("Insertions, deletions and formatting changes. Removing them accepts every change, so deleted text is gone for good.") },
  { id: "hiddenText", label: msg("Hidden text"), description: msg("Text formatted as hidden, which the page doesn't show.") },
  { id: "headersFooters", label: msg("Headers, footers and watermarks"), description: msg("The text and pictures at the top and bottom of every page, and a watermark behind the text.") },
  { id: "customXml", label: msg("Custom data"), description: msg("Data that other programs keep in the file. Your list of sources for citations is not touched.") },
];

/** The engine's finding for a category, in words for the dialog. */
function describe(id: string, f: { count: number; detail: string[] }): InspectorFinding {
  if (id === "properties") {
    const lines = f.detail.map((d) => {
      const [key, ...rest] = d.split(": ");
      const value = rest.join(": ");
      switch (key) {
        case "author":
          return t("Author: {name}", { name: value });
        case "lastModifiedBy":
          return t("Last saved by: {name}", { name: value });
        case "company":
          return t("Company: {name}", { name: value });
        case "manager":
          return t("Manager: {name}", { name: value });
        case "customProperties":
          return tn(Number(value), "{n} custom property", "{n} custom properties");
        case "thumbnail":
          return t("Preview picture");
        default:
          return d;
      }
    });
    return { count: f.count, detail: lines };
  }
  if (id === "authors") return { count: f.count, detail: f.detail };
  return { count: f.count };
}

export function inspectDocument({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.inspect",
    name: msg("Inspect document"),
    description: msg("Finds personal information and hidden data in the file (names, comments, tracked changes, hidden text, custom data) and removes what you choose."),
    activate(ctx) {
      ctx.icons.register({ id: "inspect", svg: REVIEW_ICONS.inspect });
      KINDS.forEach((k, i) => {
        ctx.inspectors.register({
          id: `docxtor.inspect.${k.id}`,
          label: t(k.label),
          description: t(k.description),
          order: 10 * (i + 1),
          inspect: ({ document }) => {
            const f = document.engine?.inspect().find((x) => x.id === k.id);
            return describe(k.id, f ?? { count: 0, detail: [] });
          },
          remove: ({ exec }) => exec({ op: "remove_hidden_data", what: [k.id] }),
        });
      });
      // Embedded objects and macros can be listed but not taken out here.
      ctx.inspectors.register({
        id: "docxtor.inspect.embedded",
        label: t("Embedded objects and macros"),
        description: t("Spreadsheets, documents and other programs kept inside the file. Docxtor keeps them as they are."),
        order: 80,
        inspect: ({ document }) => {
          const f = document.engine?.inspect().find((x) => x.id === "embedded");
          return { count: f?.count ?? 0, detail: f?.detail ?? [] };
        },
      });
      ctx.commands.register("tools.inspectDocument", () => openInspect(e, { inspect: (signal) => fe.extensions.inspect(signal), remove: (ids, signal) => fe.extensions.removeInspected(ids, signal) }), {
        title: t("Inspect document…"),
        category: t("Tools"),
        icon: "inspect",
        when: () => !!fe.engine,
      });
      ctx.menus.register({ menu: "tools", group: "3_review", command: "tools.inspectDocument" });
    },
  });
}
