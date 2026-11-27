// Table designs, as an extension: Format, Table, Table design opens a gallery of Word's built-in table styles (Table Grid,
// Plain, Grid and List tables, the last two in six accent colours), and the style options (header row, total row, banded
// rows, first and last column, banded columns) switch parts of a design on or off. Each is a command, so they sit in the
// menus, the palette and the table strip, and the assistant has its own tool for them (`set_table_design`).
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import type { RTableLook } from "@docxtor/core";
import { LOOK_OPTIONS } from "../ui/table-design";
import type { Internals } from "./index";

export function tableDesign({ fe, editor: e, tables }: Internals): Extension {
  return defineExtension({
    id: "docxtor.tableDesign",
    name: msg("Table designs"),
    description: msg("Format, Table, Table design gives a table one of Word's built-in designs and switches its header row, total row, banded rows and columns and first and last column on or off. Designs stay editable in Word."),
    activate(ctx) {
      const inTable = () => !!fe.engine && !fe.readOnly && !!e.format()?.inTable;
      ctx.commands.register("table.design", () => tables.openDesigns(), { title: t("Table design…"), category: t("Table"), when: inTable, icon: "tableDesign" });
      // The gallery's pick, for keyboard users, scripts and the assistant's commands: the id of a design (`GridTable4-Accent1`).
      ctx.commands.register("table.design.pick", (id) => typeof id === "string" && tables.pickDesign(id), { title: t("Apply table design"), category: t("Table"), when: inTable, palette: false });
      for (const o of LOOK_OPTIONS) {
        const key: keyof RTableLook = o.key;
        ctx.commands.register(`table.look.${key}`, () => tables.toggleLook(key), {
          title: o.title(),
          category: t("Table"),
          when: inTable,
          ai: true,
          toggled: () => inTable() && !!tables.look()?.[key],
        });
      }
    },
  });
}
