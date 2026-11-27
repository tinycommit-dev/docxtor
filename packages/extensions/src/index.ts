// @docxtor/extensions — the API Docxtor extensions are written against, and the host that runs them.
//
//   import { defineExtension } from "@docxtor/extensions";
//   export default defineExtension({
//     id: "acme.shout",
//     name: "Shout",
//     activate(ctx) {
//       ctx.commands.register("acme.shout", () => {
//         const sel = ctx.document.selection();
//         if (sel?.text) ctx.document.insertMarkdown(sel.text.toUpperCase());
//       }, { title: "Make the selection upper case", keybinding: "Mod+Shift+U" });
//     },
//   });
export * from "./types.ts";
export { DEFAULT_ICON_PACK, ExtensionHost, Registry, type AddOptions, type Entry, type ExtensionInfo, type HostServices, type TakenKey } from "./host.ts";
export { languageMatches, mergeIssues } from "./checkers.ts";
export { buildInstruction, FIELD_CATEGORIES, parseInstruction, quoteWord, readValues, type FieldValues, type ParsedField } from "./fields.ts";
export * from "./smart.ts";
export { columnWidth, sortViews, type ViewHandle, type ViewMode } from "./views.ts";
export { formatChord, matches, parseChord, specFromEvent, type Chord, type KeyLike } from "./keys.ts";
export { timeLimit } from "./net.ts";
export { currentLocale, directionOf, msg, pickLanguage, PSEUDO_LOCALE, PSEUDO_RTL_LOCALE, setLanguage, t, textDirection, tn, type LanguagePack, type Message } from "./l10n.ts";
