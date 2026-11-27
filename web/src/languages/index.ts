// Language packs that ship with the app. Each is an extension whose `contributes.languages` holds
// the translations (English source text to translation; see locales/source.json for the full list).
// They can be turned off under Extensions like any other.
import { defineExtension, msg, type Extension, type LanguagePack } from "@docxtor/editor";
import es from "./es.json";

const pack = (code: string, data: unknown, name: string): Extension =>
  defineExtension({
    id: `docxtor.language.${code}`,
    name,
    description: msg("Docxtor's interface in this language."),
    contributes: { languages: [data as LanguagePack] },
    activate() {},
  });

export const languages: Extension[] = [pack("es", es, "Español")];
