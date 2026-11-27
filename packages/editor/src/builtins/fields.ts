// Fields, as an extension: Insert, Date and time and Insert, Field, the document properties, View, Field codes (Alt+F9,
// and Shift+F9 for one field), and the settings for updating fields before printing and the person's own details.
// Docxtor's field types are registered through the same `ctx.fields` an extension uses. The engine works out their results.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import { toast } from "../ui/dom";
import { openDateTime } from "../ui/date-time";
import { openFieldDialog } from "../ui/field-dialog";
import { openUserInfo } from "../ui/user-info";
import { FIELD_TYPES } from "./field-types";
import { COMMAND_ICONS } from "./icons";
import type { Internals } from "./index";

/** The properties Insert, Document property offers, and the name DOCPROPERTY knows each by. */
export const PROPERTIES: [id: string, title: string, name: string][] = [
  ["author", msg("Author"), "Author"],
  ["title", msg("Title"), "Title"],
  ["subject", msg("Subject"), "Subject"],
  ["company", msg("Company"), "Company"],
  ["manager", msg("Manager"), "Manager"],
  ["category", msg("Category"), "Category"],
  ["comments", msg("Comments"), "Comments"],
  ["keywords", msg("Keywords"), "Keywords"],
  ["status", msg("Status"), "Status"],
];

export function fields({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.fields",
    name: msg("Fields"),
    description: msg("Dates, document properties, page numbers, formulas and other fields: Insert, Date and time, Insert, Field, View, Field codes."),
    activate(ctx) {
      const editable = () => !!fe.engine && !fe.readOnly;
      for (const f of FIELD_TYPES) ctx.fields.register(f);

      // The settings are kept between sessions and handed to the editor.
      fe.fieldOptions.updateBeforeOutput = ctx.state.get("updateBeforeOutput", true);
      fe.fieldOptions.user = ctx.state.get("user", {});

      ctx.commands.register("insert.dateTime", () => openDateTime(e), { title: t("Date and time…"), category: t("Insert"), when: editable, icon: COMMAND_ICONS["insert.dateTime"] });
      ctx.commands.register("insert.field", () => openFieldDialog(e, fe.extensions.fieldTypeList()), { title: t("Field…"), category: t("Insert"), when: editable, icon: COMMAND_ICONS["insert.field"] });

      for (const [id, title, name] of PROPERTIES) {
        ctx.commands.register(
          `insert.property.${id}`,
          () => {
            const before = e.engine.fieldPreview(`DOCPROPERTY ${name}`, {});
            if (e.insertField(`DOCPROPERTY ${name} \\* MERGEFORMAT`) && !before) toast(t("{property} is empty. Fill it in under File, Document details, then update fields.", { property: t(title) }));
          },
          { title: t(title), category: t("Insert"), when: editable, ai: true, icon: COMMAND_ICONS["insert.field"] },
        );
      }

      // Alt+F9 as in Word: every field shows its code instead of its result. Shift+F9 does it for the field at the caret.
      ctx.commands.register("view.fieldCodes", () => e.showFieldCodes(!e.fieldCodesShown), {
        title: t("Field codes"),
        category: t("View"),
        keybinding: "Alt+F9",
        when: () => !!fe.engine,
        toggled: () => e.fieldCodesShown,
        icon: COMMAND_ICONS["view.fieldCodes"],
      });
      ctx.commands.register("field.toggleCode", () => e.toggleFieldCodeAtCaret(), { title: t("Show or hide this field's code"), category: t("View"), keybinding: "Shift+F9", when: () => !!fe.engine && !!e.fieldAtCaret() });

      ctx.commands.register(
        "tools.updateFieldsBeforePrinting",
        () => {
          fe.fieldOptions.updateBeforeOutput = !fe.fieldOptions.updateBeforeOutput;
          ctx.state.set("updateBeforeOutput", fe.fieldOptions.updateBeforeOutput);
        },
        { title: t("Update fields before printing"), category: t("Tools"), when: () => !!fe.engine, toggled: () => fe.fieldOptions.updateBeforeOutput },
      );
      ctx.commands.register(
        "tools.userInformation",
        () =>
          openUserInfo({
            value: fe.fieldOptions.user,
            save: (user) => {
              fe.fieldOptions.user = user;
              ctx.state.set("user", user);
            },
            onClose: () => e.focus(),
          }),
        { title: t("User information…"), category: t("Tools"), when: () => !!fe.engine },
      );
    },
  });
}
