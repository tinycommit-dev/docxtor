// Review, Restrict editing: what Word's pane asks. Limit formatting to some styles, allow only one kind of editing
// (nothing, forms, comments, tracked changes), mark parts anyone may edit, then start enforcing with an optional
// password, or stop with it. The password hash is made by the engine; the salt comes from here so the op can be replayed.
import { t } from "@docxtor/extensions";
import type { MPos, ProtectionSettings } from "@docxtor/core";
import type { Editor } from "../editor/editor";
import { h, modal } from "./dom";
import type { ProtectionTool } from "./protection";

const MODES: [string, () => string][] = [
  ["readOnly", () => t("No changes (read only)")],
  ["forms", () => t("Filling in forms")],
  ["comments", () => t("Comments")],
  ["trackedChanges", () => t("Tracked changes")],
];

function salt(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return btoa(String.fromCharCode(...bytes));
}

export function openRestrictEditing(ed: Editor, tool: ProtectionTool) {
  // The selection when the dialog opened: the dialog takes the focus, and "mark as editable" means this.
  const sel = ed.range();
  const hasSel = !!sel && (sel[0].p !== sel[1].p || sel[0].o !== sel[1].o);

  const body = h("div", { class: "dialog-body restrict" });
  const actions = h("div", { class: "dialog-actions" });
  const dialog = h("div", { class: "dialog restrict-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Restrict editing") }, h("div", { class: "dialog-head" }, h("h2", {}, t("Restrict editing"))), body, actions);
  const close = modal(dialog, () => ed.focus());
  const error = h("p", { class: "field-error", role: "alert", hidden: true });
  const fail = (m: string) => {
    error.textContent = m;
    error.hidden = false;
  };

  const draw = () => {
    const cur = ed.engine.protectionInfo();
    body.textContent = "";
    actions.textContent = "";
    error.hidden = true;
    if (cur.markedFinal) {
      body.append(h("p", {}, t("This document is marked as final. Use Edit anyway before changing its protection.")));
      const done = h("button", { class: "btn primary", type: "button" }, t("Close"));
      done.addEventListener("click", close);
      actions.append(done);
      return;
    }
    if (cur.enforced) {
      body.append(h("section", { class: "restrict-sec" }, h("h3", {}, t("Protection is on")), h("p", {}, modeText(cur.edit, cur.formatting))));
      let pw: HTMLInputElement | null = null;
      if (cur.hasPassword) {
        pw = h("input", { id: "re-stop-pw", type: "password", autocomplete: "off", "aria-label": t("Password") });
        body.append(h("div", { class: "field" }, h("label", { for: "re-stop-pw" }, t("Password")), pw));
      }
      body.append(error);
      const stop = h("button", { class: "btn primary", type: "button" }, t("Stop protection"));
      const cancel = h("button", { class: "btn ghost", type: "button" }, t("Close"));
      cancel.addEventListener("click", close);
      const run = async () => {
        const ran = await ed.command("protection.stop", { ...(pw ? { password: pw.value } : {}), onError: (m: string) => fail(/wrong password/.test(m) ? t("That password isn't right. Try again.") : m) });
        if (ran.status === "done" && ran.value) {
          close();
          tool.refresh();
          ed.focus();
        }
      };
      stop.addEventListener("click", () => void run());
      pw?.addEventListener("keydown", (e) => {
        if (e.key === "Enter") void run();
      });
      actions.append(cancel, stop);
      setTimeout(() => pw?.focus());
      return;
    }

    // 1. Formatting.
    const fmtOn = h("input", { id: "re-fmt", type: "checkbox", checked: cur.formatting });
    const styleBox = h("div", { class: "restrict-styles", role: "group", "aria-label": t("Styles people may use") });
    const styles = ed.engine.styles().filter((s) => (s.kind === "paragraph" || s.kind === "character") && !s.hidden);
    const locked = new Set(cur.lockedStyles);
    const checks = styles.map((s) => {
      const c = h("input", { type: "checkbox", id: `re-st-${s.id}`, checked: !locked.has(s.id), "data-id": s.id });
      styleBox.append(h("label", { class: "check", for: `re-st-${s.id}` }, c, h("span", {}, s.name)));
      return c;
    });
    const all = h("button", { class: "btn ghost", type: "button" }, t("Allow all"));
    const none = h("button", { class: "btn ghost", type: "button" }, t("Allow none"));
    all.addEventListener("click", () => checks.forEach((c) => (c.checked = true)));
    none.addEventListener("click", () => checks.forEach((c) => (c.checked = false)));
    const styleWrap = h("div", { class: "restrict-more" }, styleBox, h("div", { class: "restrict-row" }, all, none));
    const sync1 = () => (styleWrap.hidden = !fmtOn.checked);
    fmtOn.addEventListener("change", sync1);
    sync1();
    body.append(h("section", { class: "restrict-sec" }, h("h3", {}, t("1. Formatting restrictions")), h("label", { class: "check", for: "re-fmt" }, fmtOn, h("span", {}, t("Limit formatting to a selection of styles"))), styleWrap));

    // 2. Editing.
    const editOn = h("input", { id: "re-edit", type: "checkbox", checked: cur.edit !== "none" && cur.edit !== "" });
    const mode = h("select", { id: "re-mode", "aria-label": t("Kind of editing allowed") });
    for (const [v, l] of MODES) mode.append(h("option", { value: v }, l()));
    mode.value = MODES.some(([v]) => v === cur.edit) ? cur.edit : "readOnly";
    const exceptions = h("div", { class: "restrict-more" });
    const drawExceptions = () => {
      exceptions.textContent = "";
      const n = ed.engine.protectionInfo().ranges.length;
      const mark = h("button", { class: "btn", type: "button", disabled: !hasSel }, t("Mark the selection as editable"));
      const unmark = h("button", { class: "btn ghost", type: "button", disabled: !sel || !n }, t("Remove the editable range here"));
      mark.addEventListener("click", () => {
        if (sel && ed.exec({ op: "protection", action: "editable_range", from: sel[0] as MPos, to: sel[1] as MPos })) tool.refresh();
        drawExceptions();
      });
      unmark.addEventListener("click", () => {
        if (sel && ed.exec({ op: "protection", action: "editable_range", from: sel[0], to: sel[0], remove: true })) tool.refresh();
        drawExceptions();
      });
      const hl = h("input", { id: "re-hl", type: "checkbox", checked: tool.highlighting });
      hl.addEventListener("change", () => tool.setHighlight(hl.checked));
      exceptions.append(
        h("p", { class: "restrict-note" }, t("Select parts of the document that anyone may edit freely, even though the rest is protected.")),
        h("div", { class: "restrict-row" }, mark, unmark),
        h("p", { class: "restrict-note" }, n === 0 ? t("No editable ranges yet.") : n === 1 ? t("1 editable range.") : t("{n} editable ranges.", { n })),
        h("label", { class: "check", for: "re-hl" }, hl, h("span", {}, t("Highlight the parts I can edit"))),
      );
    };
    const sync2 = () => {
      mode.disabled = !editOn.checked;
      exceptions.hidden = !editOn.checked || !["readOnly", "comments"].includes(mode.value);
    };
    editOn.addEventListener("change", sync2);
    mode.addEventListener("change", sync2);
    drawExceptions();
    sync2();
    const note = h("p", { class: "restrict-note" }, t("Docxtor treats comments as read only for now, and doesn't track changes yet."));
    body.append(
      h("section", { class: "restrict-sec" }, h("h3", {}, t("2. Editing restrictions")), h("label", { class: "check", for: "re-edit" }, editOn, h("span", {}, t("Allow only this type of editing in the document"))), mode, note, exceptions),
    );

    // 3. Start.
    const pw1 = h("input", { id: "re-pw1", type: "password", autocomplete: "new-password" });
    const pw2 = h("input", { id: "re-pw2", type: "password", autocomplete: "new-password" });
    body.append(
      h(
        "section",
        { class: "restrict-sec" },
        h("h3", {}, t("3. Start enforcement")),
        h("p", { class: "restrict-note" }, t("A password is optional. Without one, anyone can stop the protection.")),
        h("div", { class: "field" }, h("label", { for: "re-pw1" }, t("New password (optional)")), pw1),
        h("div", { class: "field" }, h("label", { for: "re-pw2" }, t("Confirm password")), pw2),
        error,
      ),
    );
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const start = h("button", { class: "btn primary", type: "button" }, t("Start enforcing protection"));
    cancel.addEventListener("click", close);
    start.addEventListener("click", () => {
      if (!editOn.checked && !fmtOn.checked) return fail(t("Choose a restriction first."));
      if (pw1.value !== pw2.value) return fail(t("The passwords don't match."));
      const settings: ProtectionSettings = { edit: editOn.checked ? mode.value : "none", formatting: fmtOn.checked, enforce: true };
      if (fmtOn.checked) settings.allowedStyles = checks.filter((c) => c.checked).map((c) => c.dataset.id!);
      if (pw1.value) {
        settings.password = pw1.value;
        settings.salt = salt();
      }
      if (ed.exec({ op: "protection", action: "set", settings }, { onError: fail })) {
        close();
        tool.refresh();
        ed.focus();
      }
    });
    actions.append(cancel, start);
  };
  draw();
}

function modeText(edit: string, formatting: boolean): string {
  const parts: string[] = [];
  if (edit === "readOnly") parts.push(t("The document can't be changed, except in the parts marked as editable."));
  else if (edit === "comments") parts.push(t("The document can't be changed, except in the parts marked as editable. Comments are not supported yet."));
  else if (edit === "forms") parts.push(t("Only form fields and content controls can be filled in."));
  else if (edit === "trackedChanges") parts.push(t("Changes should be tracked."));
  if (formatting) parts.push(t("Formatting is limited to some styles."));
  return parts.join(" ");
}
