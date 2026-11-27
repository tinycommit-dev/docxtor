// Content controls, form fields and document protection, as an extension. Insert, Form controls puts a check box,
// drop-down list, combo box, date picker, plain or rich text, picture or repeating section into the text (and Word's
// older form fields: a text blank, a check box and a drop-down); a control has Properties and Remove; Tools, Restrict
// editing… restricts what can be changed; File, Mark as final marks the document final. Controls and fields are filled
// in on the page itself (ui/controls.ts); extensions can add kinds with `ctx.contentControls.register`.
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import type { Internals } from "./index";

export function contentControls({ fe, editor: e, controls, protection }: Internals): Extension {
  return defineExtension({
    id: "docxtor.controls",
    name: msg("Content controls and forms"),
    description: msg("Check boxes, drop-down lists, date pickers, text and picture controls, repeating sections and Word's older form fields, and restricting what can be edited."),
    activate(ctx) {
      const can = () => !!fe.engine && !fe.readOnly;
      const kinds: [string, string, string, string][] = [
        ["checkbox", t("Check box"), "controlCheckbox", "checkbox"],
        ["dropDown", t("Drop-down list"), "controlDropDown", "dropDown"],
        ["comboBox", t("Combo box"), "controlCombo", "comboBox"],
        ["date", t("Date picker"), "controlDate", "date"],
        ["text", t("Plain text"), "controlText", "text"],
        ["richText", t("Rich text"), "controlRichText", "richText"],
        ["picture", t("Picture"), "controlPicture", "picture"],
        ["repeating", t("Repeating section"), "controlRepeating", "repeating"],
      ];
      for (const [id, title, icon, kind] of kinds) {
        ctx.commands.register(`insert.control.${id}`, () => controls.insert(kind), { title, category: t("Insert"), when: can, icon });
        ctx.menus.register({ menu: "insert/forms", group: "1", command: `insert.control.${id}` });
      }
      const legacy: [string, string, "text" | "checkbox" | "dropdown"][] = [
        ["text", t("Text field"), "text"],
        ["checkbox", t("Check box field"), "checkbox"],
        ["dropdown", t("Drop-down field"), "dropdown"],
      ];
      for (const [id, title, kind] of legacy) {
        ctx.commands.register(`insert.form.${id}`, () => controls.insertForm(kind), { title, category: t("Insert"), when: can, icon: "formField" });
        ctx.menus.register({ menu: "insert/forms-legacy", group: "1", command: `insert.form.${id}` });
      }
      ctx.menus.register({ menu: "insert", group: "1_objects", submenu: "insert/forms", title: t("Form controls"), icon: "formControls" });
      ctx.menus.register({ menu: "insert/forms", group: "2", submenu: "insert/forms-legacy", title: t("Legacy form fields"), icon: "formField" });

      ctx.commands.register("control.properties", () => controls.properties(), { title: t("Content control properties…"), category: t("Insert"), when: () => can() && controls.has(), icon: "controlProperties" });
      ctx.commands.register("control.remove", () => controls.remove(), { title: t("Remove content control"), category: t("Insert"), when: () => can() && controls.has(), icon: "close" });
      ctx.commands.register("control.addItem", () => controls.addItem(), { title: t("Add repeating section item"), category: t("Insert"), when: () => can() && controls.inRepeating(), icon: "controlRepeating" });
      ctx.commands.register("form.properties", () => controls.formProperties(), { title: t("Form field properties…"), category: t("Insert"), when: () => can() && !!controls.formAtCaret(), icon: "formField" });
      ctx.menus.register({ menu: "context", group: "6_controls", command: "control.properties" });
      ctx.menus.register({ menu: "context", group: "6_controls", command: "control.remove" });
      ctx.menus.register({ menu: "context", group: "6_controls", command: "form.properties" });

      ctx.commands.register("tools.restrictEditing", () => protection.open(), { title: t("Restrict editing…"), category: t("Tools"), when: () => !!fe.engine, icon: "restrictEditing" });
      ctx.menus.register({ menu: "tools", group: "3_review", command: "tools.restrictEditing" });
      // The Stop protection button of the Restrict editing pane: it passes the password and takes the engine's refusal back.
      ctx.commands.register(
        "protection.stop",
        (opts) => {
          const { password, onError } = (opts ?? {}) as { password?: string; onError?: (message: string) => void };
          return !!e.exec({ op: "protection", action: "stop", ...(password ? { password } : {}) }, { onError });
        },
        { title: t("Stop protection"), category: t("Tools"), when: () => can() && !!protection.info?.enforced && !protection.info.markedFinal, palette: false },
      );
      ctx.commands.register(
        "file.markAsFinal",
        () => {
          const was = !!protection.info?.markedFinal;
          if (was) protection.editAnyway();
          else if (protection.markFinal(true)) ctx.ui.notify(t("Marked as final. The document is read-only until you choose Edit anyway."));
        },
        // While the window is read-only for another reason (Reading, Viewing, pages side by side) it can't be marked, only unmarked.
        { title: t("Mark as final"), category: t("File"), when: () => !!fe.engine && (!!protection.info?.markedFinal || !e.readOnly), icon: "markFinal", toggled: () => !!protection.info?.markedFinal },
      );
      ctx.menus.register({ menu: "file", group: "6_details", command: "file.markAsFinal" });
      // The page itself: the strip of the controls, the strip of protection.
      fe.surface.prepend(protection.el);
      ctx.subscriptions.push({ dispose: () => protection.el.remove() });
      ctx.subscriptions.push(ctx.document.onDidOpen(() => protection.reset()));
      ctx.subscriptions.push(ctx.document.onDidChange(() => protection.schedule()));
      if (fe.engine) protection.reset();
    },
  });
}
