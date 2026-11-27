// Where the app's sources come from, as extensions that plug into the Citations panel's "Add a source": a DOI
// looked up at doi.org, and BibTeX that is pasted or read from a file. The DOI one is the part that asks another
// server, so it is its own extension and can be turned off under Extensions, Manage extensions.
import { defineExtension, msg, t, type Extension } from "@docxtor/editor";
import { fetchDoi, findDoi, readSources } from "./lookup";

export function doiSource(): Extension {
  return defineExtension({
    id: "docxtor.citations.doi",
    name: msg("Sources from a DOI"),
    description: msg("Add a source by its DOI or doi.org link. The DOI is sent to doi.org; turn this off to keep every request on your own servers."),
    activate(ctx) {
      ctx.citations.sources.register({
        id: "doi",
        label: t("From a DOI or URL…"),
        title: t("Add from a DOI or URL"),
        intro: t("Paste a DOI or a doi.org link. Docxtor asks doi.org for the details, and you check them before they are saved."),
        inputLabel: t("DOI or link"),
        placeholder: "10.1000/xyz123",
        help: t("The DOI is sent to doi.org. Nothing else is."),
        note: t("A DOI lookup sends the DOI to doi.org. Nothing else leaves your device."),
        action: t("Look up"),
        review: true,
        reviewNote: t("This is what doi.org has. Fix anything that looks wrong, then add it."),
        async resolve(input, signal) {
          const doi = findDoi(input);
          if (!doi) throw new Error(t("That doesn't look like a DOI. A DOI starts with 10. and has a slash, like 10.1000/xyz123."));
          return [await fetchDoi(doi, signal)];
        },
      });
    },
  });
}

export function bibtexSource(): Extension {
  return defineExtension({
    id: "docxtor.citations.bibtex",
    name: msg("Sources from BibTeX"),
    description: msg("Add sources from BibTeX or CSL-JSON that you paste or choose as a file. Nothing leaves your device."),
    activate(ctx) {
      ctx.citations.sources.register({
        id: "bibtex",
        label: t("Import BibTeX…"),
        title: t("Import BibTeX"),
        intro: t("Paste BibTeX from Google Scholar, Zotero or a .bib file, then choose the sources to add."),
        inputLabel: t("BibTeX"),
        placeholder: "@article{key,\n  author = {Smith, Jane},\n  title = {…},\n  year = {2020}\n}",
        multiline: true,
        live: true,
        files: { accept: ".bib,.bibtex,.txt,.json", button: t("Choose a .bib file…"), label: t("BibTeX file") },
        resolve: (input) => readSources(input),
      });
    },
  });
}
