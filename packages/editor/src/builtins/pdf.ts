// PDF, as an extension: File, Download, PDF document. Docxtor writes the file itself from the pages as the
// editor has laid them out (see src/pdf), so it matches the screen: the same lines, fonts, pictures, tables
// and page sizes, with text that can be selected and searched, links, and bookmarks from the headings.
// The file is tagged for screen readers by default (a structure tree of headings, paragraphs, lists, tables, figures and
// links), and "PDF/A archive" makes a PDF/A-2b file for long-term keeping.
import { defineExtension, msg, t, tn, type Extension } from "@docxtor/extensions";
import { exportPdf, type PdfOptions } from "../pdf/export";
import { overlayLayer, toast } from "../ui/dom";
import type { Internals } from "./index";

export function pdf({ editor }: Internals): Extension {
  return defineExtension({
    id: "docxtor.pdf",
    name: msg("PDF"),
    description: msg("Download the document as a PDF that looks like the pages on screen, tagged for screen readers, or as a PDF/A archive file."),
    activate(ctx) {
      let busy = false;
      const options = { tagged: ctx.state.get("tagged", true), pdfa: ctx.state.get("pdfa", false) };

      // Make the file, with a progress notice when it takes more than a moment.
      const make = async (opts: Pick<PdfOptions, "tagged" | "pdfa">): Promise<Uint8Array> => {
        if (busy) throw new Error(t("A PDF is already being made"));
        busy = true;
        // Short documents are done before anyone could read a notice.
        const started = performance.now();
        let last = -1;
        const progress = (f: number) => {
          const percent = Math.floor(f * 10) * 10;
          if (performance.now() - started < 700 || percent === last) return;
          last = percent;
          toast(t("Making the PDF, {percent}%", { percent }), 60000);
        };
        try {
          const result = await editor.asPages(() => exportPdf(editor, { ...opts, progress }));
          overlayLayer().querySelector(".toast")?.remove();
          // One notice says everything there is to say (a second would replace the first).
          const notes: string[] = [];
          if (opts.pdfa && !result.pdfa) notes.push(t("This file is a PDF but not PDF/A: some text is in a font the file does not hold. Turn on the fonts the document uses and try again."));
          else if (result.substituted.length) {
            const fonts = result.substituted.join(", ");
            notes.push(tn(result.substituted.length, "A font wasn't available, so a similar one was used: {fonts}.", "{n} fonts weren't available, so similar ones were used: {fonts}.", { fonts }));
          }
          if (result.unreadablePictures) {
            notes.push(tn(result.unreadablePictures, "A linked picture's website wouldn't let Docxtor copy it, so the PDF has an empty box in its place.", "{n} linked pictures' websites wouldn't let Docxtor copy them, so the PDF has empty boxes in their place."));
          }
          if (notes.length) toast(notes.join(" "), 9000);
          return result.bytes;
        } catch (e) {
          overlayLayer().querySelector(".toast")?.remove();
          throw e;
        } finally {
          busy = false;
        }
      };

      ctx.exporters.register({ id: "pdf", label: t("PDF document"), extension: "pdf", mime: "application/pdf", export: () => make(options) });
      ctx.exporters.register({ id: "pdfa", label: t("PDF/A archive"), extension: "pdf", mime: "application/pdf", export: () => make({ pdfa: true }) });

      const group = t("PDF files");
      ctx.settings.register({
        id: "docxtor.pdf.tagged",
        group,
        label: t("Tag PDFs for screen readers"),
        help: t("A tagged PDF has the structure of the document (headings, lists, tables, picture descriptions and links), so screen readers and other tools can read it in order."),
        kind: "toggle",
        get: () => options.tagged,
        set: (v) => {
          options.tagged = !!v;
          ctx.state.set("tagged", options.tagged);
        },
      });
      ctx.settings.register({
        id: "docxtor.pdf.pdfa",
        group,
        label: t("Make PDF documents PDF/A archive files"),
        help: t("PDF/A is for keeping documents for years: every font is held in the file, with a colour profile and the metadata the standard asks for. The PDF/A archive item in File, Download does it for one file."),
        kind: "toggle",
        get: () => options.pdfa,
        set: (v) => {
          options.pdfa = !!v;
          ctx.state.set("pdfa", options.pdfa);
        },
      });
    },
  });
}
