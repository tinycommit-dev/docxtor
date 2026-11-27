// The Docxtor extension API: what an extension sees. Hosts (the editor, the Docxtor app, a product
// embedding Docxtor) implement it; extensions only ever import these types.
export type { EmbeddedFont } from "@docxtor/core";
export * from "./types/extension.ts";
export * from "./types/api.ts";
export * from "./types/ui.ts";
export * from "./types/files.ts";
export * from "./types/look.ts";
export * from "./types/sources.ts";
export * from "./types/ai.ts";
export * from "./types/storage.ts";
export * from "./types/language.ts";
export * from "./types/inspect.ts";
export * from "./types/fields.ts";
export * from "./types/mail.ts";
