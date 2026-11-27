// Table tools, as an extension: Insert table…, Table properties… (size, alignment, rows, columns, cells, alt text),
// Split cells and Split table, Insert and Delete cells with the others shifting, Distribute rows, Sort table, Convert
// text to table and table to text, Formula…, and the direction of the text in cells. Each is a command, so it sits in
// the menus (Format, Table), the right-click menu in a table and the palette. The ones that need no choices can be run
// by the assistant, which also has the ops themselves (`table_format`, `table_rows`, `table_cells`, …).
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { TABLE_ICONS } from "../ui/table-icons";
import { openConvertToTable, openConvertToText, openFormula, openInsertTable, openCells, openSplitCells } from "../ui/table-dialogs";
import { openSort } from "../ui/sort";
import { openTableProperties } from "../ui/table-properties";
import { cellSelection, distributeRows } from "../ui/table-selection";
import type { Internals } from "./index";

/** The direction the text of a cell runs in: Word's "Text direction" button cycles the same three. */
export const TEXT_DIRECTIONS: [id: string, value: string, title: string][] = [
  ["horizontal", "lrTb", msg("Text across")],
  ["down", "tbRl", msg("Text down")],
  ["up", "btLr", msg("Text up")],
];

export function tableTools({ fe, editor: e, tables }: Internals): Extension {
  return defineExtension({
    id: "docxtor.tableTools",
    name: msg("Table tools"),
    description: msg("Insert table, Table properties, Split cells, Split table, Insert and Delete cells, Distribute rows, Sort table, Convert text to table and back, Formula and the direction of text in cells."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(TABLE_ICONS)) ctx.icons.register({ id, svg });
      const open = () => !!fe.engine;
      const editable = () => open() && !fe.readOnly;
      const inTable = () => editable() && !!e.format()?.inTable;
      const cmd = (id: string, title: string, run: () => unknown, when = inTable, extra: { icon?: string; ai?: boolean; palette?: boolean } = {}) =>
        ctx.commands.register(id, run, { title, category: t("Table"), when, ...extra });

      ctx.commands.register("insert.tableDialog", () => openInsertTable(e), { title: t("Insert table…"), category: t("Insert"), when: editable, icon: "table" });
      cmd("table.properties", t("Table properties…"), () => openTableProperties(e), inTable, { icon: "tableProperties" });
      cmd("table.splitCells", t("Split cells…"), () => openSplitCells(e), inTable, { icon: "splitCells" });
      cmd("table.splitTable", t("Split table"), () => {
        const sel = cellSelection(e);
        if (sel) e.exec({ op: "split_table", at: sel.from });
      }, inTable, { icon: "splitTable", ai: true });
      cmd("table.insertCells", t("Insert cells…"), () => openCells(e, "insert"), inTable, { icon: "insertCells" });
      cmd("table.deleteCells", t("Delete cells…"), () => openCells(e, "delete"), inTable, { icon: "deleteCells" });
      cmd("table.distributeRows", t("Distribute rows"), () => distributeRows(e), inTable, { icon: "distributeRows", ai: true });
      cmd("table.sort", t("Sort table…"), () => {
        const sel = cellSelection(e);
        if (!sel) return;
        // Several rows selected sort those; otherwise the whole table.
        const rows = sel.rows[1] > sel.rows[0];
        openSort(e, { from: sel.from, ...(rows ? { to: sel.to } : {}), columns: sel.details.grid.length, header: sel.details.rows[0]?.header || !!tables.look()?.firstRow });
      }, inTable, { icon: "sortTable" });
      cmd("table.convertToTable", t("Convert text to table…"), () => openConvertToTable(e), () => editable() && !e.story && !e.format()?.inTable, { icon: "convertToTable" });
      cmd("table.convertToText", t("Convert table to text…"), () => openConvertToText(e), inTable, { icon: "convertToText" });
      cmd("table.formula", t("Formula…"), () => openFormula(e), inTable, { icon: "formula" });
      for (const [id, value, title] of TEXT_DIRECTIONS) {
        cmd(`table.textDirection.${id}`, t(title), () => {
          const sel = cellSelection(e);
          if (sel) e.exec({ op: "table_cells", from: sel.from, to: sel.to, action: { type: "format", textDirection: value } });
        }, inTable, { icon: "textDirection", ai: true });
      }
    },
  });
}
