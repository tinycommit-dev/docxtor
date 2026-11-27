// Every warning the app gives before a risky action, in one place (DESIGN.md, Risky actions); the editor's are in
// packages/editor/src/builtins/cautions.ts. The screens ask with `host.ask(id, ...)` (their hosts pass it on from
// the extension host), with the arguments the wording takes.
import { defineExtension, msg, t, tn, type Caution, type Extension } from "@docxtor/editor";

export function appCautions(): Caution[] {
  const name = (v: unknown) => String(v ?? "");
  const count = (v: unknown) => Number(v) || 0;
  return [
    // Files: to the Trash (kept for a while), and out of it for good.
    { id: "files.delete", kind: "ask", title: (n) => t("Delete “{name}”?", { name: name(n) }), impact: (_, days) => t("It moves to the Trash, where it stays for {days} days.", { days: count(days) }), confirm: t("Delete") },
    { id: "files.deleteFolder", kind: "ask", title: t("Delete folder?"), impact: (folder, days) => t("“{folder}” and everything in it moves to the Trash, where it stays for {days} days.", { folder: name(folder), days: count(days) }), confirm: t("Delete") },
    { id: "trash.empty", kind: "ask", title: t("Empty the trash?"), impact: (n) => tn(count(n), "{n} document will be deleted for good.", "{n} documents will be deleted for good."), confirm: t("Empty trash") },
    { id: "trash.deleteForever", kind: "ask", title: (n) => t("Delete “{name}” for good?", { name: name(n) }), impact: t("It can't be put back afterwards."), confirm: t("Delete for good") },
    // The document as a whole.
    { id: "history.restore", kind: "ask", title: t("Restore this version?"), impact: t("The document will be replaced with this version. Your current state is saved as a version first, so you can come back."), confirm: t("Restore"), danger: false },
    { id: "password.remove", kind: "ask", title: t("Remove the password?"), impact: t("Once the document is saved, anyone with the file can open it."), confirm: t("Remove password") },
    { id: "citations.deleteSource", kind: "ask", title: t("Delete this source?"), impact: (why) => name(why), confirm: t("Delete source") },
    // Kept for the person, outside the document.
    { id: "templates.delete", kind: "ask", title: (n) => t("Delete “{template}”?", { template: name(n) }), impact: t("The template is removed. Documents made from it stay as they are."), confirm: t("Delete") },
    { id: "citations.forgetShared", kind: "ask", title: (n) => tn(count(n), "Remove {n} source from the shared list?", "Remove {n} sources from the shared list?"), impact: t("Documents that cite them keep their own copies."), confirm: t("Remove from the shared list"), permanent: true },
    { id: "citations.removeStyle", kind: "ask", title: (n) => t("Remove “{name}”?", { name: name(n) }), impact: t("Documents that use it keep their citations as they are."), confirm: t("Remove"), permanent: true },
  ];
}

export function cautions(): Extension {
  return defineExtension({
    id: "docxtor.app.cautions",
    name: msg("Warnings before risky actions in the app"),
    description: msg("Asks before deleting files, emptying the Trash, restoring a version, removing a password and the like."),
    activate(ctx) {
      for (const c of appCautions()) ctx.cautions.register(c);
    },
  });
}
