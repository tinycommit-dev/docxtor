// Docxtor's own sources of recipients: a CSV file and an Excel workbook. They are plain registrations, like any extension's. The
// engine reads the files (Engine.readRecipients); the list is kept in the document afterwards.
import { Engine } from "@docxtor/core";
import { msg, t, type MailMergeDataSource, type RecipientTable } from "@docxtor/extensions";

/** Ask for a file with a chooser. Resolves null when the person closes the chooser. */
function chooseFile(accept: string): Promise<File | null> {
  return new Promise((resolve) => {
    const picker = document.createElement("input");
    picker.type = "file";
    picker.accept = accept;
    picker.addEventListener("change", () => resolve(picker.files?.[0] ?? null));
    picker.addEventListener("cancel", () => resolve(null));
    picker.click();
  });
}

export function csvSource(): MailMergeDataSource {
  return {
    id: "docxtor.csv",
    label: msg("From a CSV file…"),
    icon: "mmRecipients",
    async pick({ ui }) {
      const f = await chooseFile(".csv,.tsv,.txt,text/csv,text/plain");
      if (!f) return null;
      const list = Engine.readRecipients(new Uint8Array(await f.arrayBuffer()), f.name);
      if (list.truncated) ui.notify(t("Only the first 100,000 recipients were read."));
      return { columns: list.columns, rows: list.rows, name: f.name, kind: "csv", delimiter: list.source.delimiter };
    },
  };
}

export function excelSource(): MailMergeDataSource {
  return {
    id: "docxtor.xlsx",
    label: msg("From an Excel file…"),
    icon: "mmRecipients",
    async pick({ ui }) {
      const f = await chooseFile(".xlsx,.xlsm,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
      if (!f) return null;
      const bytes = new Uint8Array(await f.arrayBuffer());
      let list = Engine.readRecipients(bytes, f.name);
      // The first sheet, with its first row as the column names. A workbook with several sheets asks which one.
      if (list.sheets.length > 1) {
        const pick = await ui.pick(
          list.sheets.map((label) => ({ label })),
          { placeholder: t("Which sheet holds the recipients?") },
        );
        if (!pick) return null;
        if (pick.label !== list.source.table) list = Engine.readRecipients(bytes, f.name, pick.label);
      }
      if (list.truncated) ui.notify(t("Only the first 100,000 recipients were read."));
      const table: RecipientTable = { columns: list.columns, rows: list.rows, name: f.name, kind: "xlsx", table: list.source.table };
      return table;
    },
  };
}
