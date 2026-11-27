// Mail merge, as an extension: Mailings, with Start mail merge (letters, labels, envelopes, directory), Select recipients (any
// data source: Docxtor's own are a CSV file and an Excel file), Edit recipient list, merge fields, Address block, Greeting line,
// the rules, Preview results, Finish and merge; and Envelopes and Labels on their own. Everything is stored in the .docx the way
// Word stores it (`w:mailMerge`, complex fields); the recipients are kept in the file too.
import { defineExtension, msg, t, type Disposable, type Extension, type MailMergeDataSource } from "@docxtor/extensions";
import { MAIL_ICONS } from "../mailmerge/icons";
import { openEnvelope, openLabels, builtinProducts } from "../mailmerge/envelope-labels";
import { openSendEmail } from "../mailmerge/email";
import { openFinish } from "../mailmerge/finish";
import { openPrintMerged } from "../mailmerge/print-merged";
import { emlSender, mailtoSender } from "../mailmerge/senders";
import { findDuplicatesOfList } from "../mailmerge/duplicates-dialog";
import { openAddressBlock, openGreetingLine, openInsertField, openMatchFields, openRule } from "../mailmerge/field-dialogs";
import { openRecipients } from "../mailmerge/recipients";
import { MergeSession } from "../mailmerge/session";
import { csvSource, excelSource } from "../mailmerge/sources";
import { openFindRecipient } from "../mailmerge/find";
import type { Internals } from "./index";

export function mailMerge(x: Internals): Extension {
  return defineExtension({
    id: "docxtor.mailMerge",
    name: msg("Mail merge"),
    description: msg("Mailings: letters, labels, envelopes and directories filled from a list of recipients, with merge fields, rules, a preview and Finish and merge. Envelopes and Labels on their own."),
    activate(ctx) {
      const fe = x.fe;
      for (const [id, svg] of Object.entries(MAIL_ICONS)) ctx.icons.register({ id, svg });
      const s = new MergeSession(x, ctx.ui);
      const cat = t("Mailings");

      // -- where recipients and label sheets come from ----------------------------------------
      ctx.mailMerge.dataSources.register(csvSource());
      ctx.mailMerge.dataSources.register(excelSource());
      // The ways to send the messages of an email merge. Both work without a server; a mail service is another extension.
      ctx.mailMerge.mailSenders.register(emlSender());
      ctx.mailMerge.mailSenders.register(mailtoSender());
      let products = false;
      const loadProducts = () => {
        if (products) return;
        try {
          for (const p of builtinProducts()) ctx.mailMerge.labelProducts.register(p);
          products = true;
        } catch {
          // The engine isn't loaded yet; the next document that opens tries again.
        }
      };
      loadProducts();
      ctx.subscriptions.push({ dispose: fe.on("open", loadProducts) } as Disposable);

      // -- commands ---------------------------------------------------------------------------
      const hasEngine = () => !!fe.engine;
      const editable = () => s.editable;
      const active = () => hasEngine() && !!s.info()?.active;
      const recipients = () => hasEngine() && !!s.info()?.hasRecipients;
      const writable = () => active() && editable();
      const cmd = (id: string, title: string, run: () => void, o: { when?: () => boolean; icon?: string; toggled?: () => boolean; keybinding?: string } = {}) =>
        ctx.commands.register(`mailings.${id}`, run, { title, category: cat, when: o.when ?? hasEngine, icon: o.icon, toggled: o.toggled, keybinding: o.keybinding });

      cmd("start.letters", t("Letters"), () => void s.start("formLetters"), { when: editable, icon: "mmEnvelope", toggled: () => s.info()?.active === true && s.info()?.mainType === "formLetters" });
      cmd("start.email", t("Email messages"), () => void s.start("email"), { when: editable, icon: "mmSend", toggled: () => s.info()?.active === true && s.info()?.mainType === "email" });
      cmd("start.labels", t("Labels…"), () => {
        loadProducts();
        openLabels(s, ctx, "merge");
      }, { when: editable, icon: "mmLabels", toggled: () => s.info()?.active === true && s.info()?.mainType === "mailingLabels" });
      cmd("start.envelopes", t("Envelopes…"), () => openEnvelope(s, ctx, "merge"), { when: editable, icon: "mmEnvelope", toggled: () => s.info()?.active === true && s.info()?.mainType === "envelopes" });
      cmd("start.directory", t("Directory"), () => void s.start("catalog"), { when: editable, icon: "mmRecipients", toggled: () => s.info()?.active === true && s.info()?.mainType === "catalog" });
      cmd("stop", t("Normal document (stop the mail merge)"), () => void s.stop(), { when: () => active() && editable() });

      cmd("recipients.edit", t("Edit recipient list…"), () => openRecipients(s), { when: recipients, icon: "mmRecipients", keybinding: "Alt+Shift+E" });
      cmd("recipients.duplicates", t("Find duplicates…"), () => void findDuplicatesOfList(s), { when: recipients, icon: "mmDuplicates" });
      cmd("recipients.match", t("Match fields…"), () => openMatchFields(s), { when: recipients, icon: "mmMatch" });

      cmd("highlight", t("Highlight merge fields"), () => s.setHighlight(!s.highlight), { icon: "highlight", toggled: () => s.highlight });
      cmd("insertField", t("Insert merge field…"), () => openInsertField(s), { when: () => recipients() && editable(), icon: "mmField", keybinding: "Alt+Shift+F" });
      cmd("addressBlock", t("Address block…"), () => openAddressBlock(s), { when: () => recipients() && editable(), icon: "mmAddress" });
      cmd("greetingLine", t("Greeting line…"), () => openGreetingLine(s), { when: () => recipients() && editable(), icon: "mmGreeting" });
      cmd("rule.if", t("If…Then…Else…"), () => void openRule(s, "if"), { when: () => recipients() && editable() });
      cmd("rule.ask", t("Ask…"), () => void openRule(s, "ask"), { when: writable });
      cmd("rule.fillIn", t("Fill-in…"), () => void openRule(s, "fillIn"), { when: writable });
      cmd("rule.mergeRecord", t("Merge record #"), () => void s.insert({ kind: "merge_rec" }), { when: writable });
      cmd("rule.mergeSequence", t("Merge sequence #"), () => void s.insert({ kind: "merge_seq" }), { when: writable });
      cmd("rule.nextRecord", t("Next record"), () => void s.insert({ kind: "next" }), { when: writable });
      cmd("rule.nextIf", t("Next record if…"), () => void openRule(s, "nextIf"), { when: () => recipients() && editable() });
      cmd("rule.set", t("Set bookmark…"), () => void openRule(s, "set"), { when: writable });
      cmd("rule.skipIf", t("Skip record if…"), () => void openRule(s, "skipIf"), { when: () => recipients() && editable() });
      cmd("updateLabels", t("Update labels"), () => void s.run({ op: "update_labels" }), { when: () => writable() && s.info()?.mainType === "mailingLabels", icon: "mmRefresh" });

      cmd("preview", t("Preview results"), () => (s.previewing === null ? void s.preview(0) : s.stopPreview()), { when: recipients, icon: "mmPreview", toggled: () => s.previewing !== null });
      cmd("first", t("First record"), () => s.go("first"), { when: recipients, icon: "mmFirst" });
      cmd("previous", t("Previous record"), () => s.go("previous"), { when: recipients, icon: "mmPrevious" });
      cmd("next", t("Next record"), () => s.go("next"), { when: recipients, icon: "mmNext" });
      cmd("last", t("Last record"), () => s.go("last"), { when: recipients, icon: "mmLast" });
      cmd("find", t("Find recipient…"), () => openFindRecipient(s), { when: recipients, icon: "search" });

      cmd("finish", t("Finish and merge…"), () => void openFinish(s), { when: recipients, icon: "mmFinish", keybinding: "Alt+Shift+N" });
      cmd("finish.print", t("Print merged documents…"), () => void openPrintMerged(s), { when: recipients, icon: "print" });
      cmd("finish.email", t("Send email messages…"), () => void openSendEmail(s), { when: recipients, icon: "mmSend" });

      cmd("envelopes", t("Envelopes…"), () => openEnvelope(s, ctx, "document"), { when: editable, icon: "mmEnvelope" });
      cmd("labels", t("Labels…"), () => {
        loadProducts();
        openLabels(s, ctx, "document");
      }, { when: hasEngine, icon: "mmLabels" });

      // The sources of recipients, one line each under Select recipients.
      const shown = new Map<string, Disposable[]>();
      const sync = () => {
        const list = fe.extensions.mailMergeSources.list();
        for (const [id, ds] of shown) {
          if (list.some((src) => src.id === id)) continue;
          ds.forEach((d) => d.dispose());
          shown.delete(id);
        }
        for (const src of list as MailMergeDataSource[]) {
          if (shown.has(src.id)) continue;
          const command = `mailings.recipients.from.${src.id}`;
          shown.set(src.id, [
            ctx.commands.register(command, () => void s.choose(src), { title: t("Select recipients: {source}", { source: t(src.label) }), category: cat, when: editable, icon: src.icon }),
            ctx.menus.register({ menu: "mailings/recipients", group: "1", command, title: t(src.label) }),
          ] as unknown as Disposable[]);
        }
      };
      sync();
      ctx.subscriptions.push(fe.extensions.mailMergeSources.onChange(sync));

      // -- the menu ------------------------------------------------------------------------------
      const item = (menu: string, group: string, command: string, title?: string) => ctx.menus.register({ menu, group, command: `mailings.${command}`, title });
      const sub = (menu: string, group: string, submenu: string, title: string, icon?: string) => ctx.menus.register({ menu, group, submenu, title, icon });
      item("mailings", "1_create", "envelopes");
      item("mailings", "1_create", "labels");
      sub("mailings", "2_start", "mailings/start", t("Start mail merge"), "mmEnvelope");
      for (const c of ["letters", "email", "labels", "envelopes", "directory"]) item("mailings/start", "1", `start.${c}`);
      item("mailings/start", "2", "stop");
      sub("mailings", "2_start", "mailings/recipients", t("Select recipients"), "mmRecipients");
      item("mailings", "2_start", "recipients.edit");
      item("mailings", "2_start", "recipients.duplicates");
      item("mailings", "2_start", "recipients.match");
      item("mailings", "3_write", "highlight");
      item("mailings", "3_write", "insertField");
      item("mailings", "3_write", "addressBlock");
      item("mailings", "3_write", "greetingLine");
      sub("mailings", "3_write", "mailings/rules", t("Rules"), "mmRules");
      for (const c of ["if", "ask", "fillIn", "mergeRecord", "mergeSequence", "nextRecord", "nextIf", "set", "skipIf"]) item("mailings/rules", "1", `rule.${c}`);
      item("mailings", "3_write", "updateLabels");
      item("mailings", "4_preview", "preview");
      for (const c of ["first", "previous", "next", "last", "find"]) item("mailings", "4_preview", c);
      item("mailings", "5_finish", "finish");
      item("mailings", "5_finish", "finish.print");
      item("mailings", "5_finish", "finish.email");
    },
  });
}
