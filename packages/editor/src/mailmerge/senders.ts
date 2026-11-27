// The two ways to send merged messages that need no server: a .zip of .eml files (any mail app opens one, and most send it), and the
// person's own mail app, one message at a time with a mailto: link. A mail service (SMTP, Gmail, Outlook) is another sender that an
// extension adds with `ctx.mailMerge.mailSenders` (see docs/extensions/index.md).
import { msg, t, tn, type EmailSender } from "@docxtor/extensions";
import { h, modal } from "../ui/dom";
import { downloadBytes } from "./download";
import { emlBytes, safeFileName } from "./eml";
import { mailtoLink } from "./mailto";
import { makeZip } from "./zip";

/** Save as email files: every message becomes an .eml file in one .zip. */
export function emlSender(): EmailSender {
  return {
    id: "docxtor.eml",
    label: msg("Save as email files"),
    icon: "mmEnvelope",
    note: msg("Makes a .zip with one .eml file for every recipient. Open one in your mail app to check it and send it. Nothing leaves your computer."),
    summary: (n) => tn(n, "Saved {n} email file in a .zip.", "Saved {n} email files in a .zip."),
    send(messages, ctx) {
      const digits = String(messages.length).length;
      const entries = messages.map((m, i) => ({
        name: `${String(i + 1).padStart(digits, "0")} ${safeFileName(m.to, "message")}.eml`,
        bytes: emlBytes(m),
      }));
      downloadBytes(makeZip(entries), `${t("{name} emails", { name: ctx.name })}.zip`, "application/zip");
      messages.forEach((_, i) => ctx.progress(i, { ok: true }));
    },
  };
}

/** Open in my mail app: plain text only, a message each time the person chooses Next email (a browser allows one window per click). */
export function mailtoSender(): EmailSender {
  return {
    id: "docxtor.mailto",
    label: msg("Open in my mail app"),
    icon: "mmEnvelope",
    note: msg("Opens a message in your mail app for each recipient, one at a time, as plain text. You send each one there."),
    formats: ["text"],
    warnAbove: 10,
    limitNote: msg("That is a lot of windows to open by hand. Save as email files is better for a long list."),
    summary: (n) => tn(n, "Opened {n} message in your mail app.", "Opened {n} messages in your mail app."),
    send: (messages, ctx) =>
      new Promise<void>((resolve) => {
        let next = 0;
        let cutCount = 0;
        const status = h("p", { class: "help", role: "status", "aria-live": "polite" });
        const go = h("button", { class: "btn primary", type: "button" });
        const stop = h("button", { class: "btn ghost", type: "button" }, t("Stop"));
        const label = () => (go.textContent = t("Next email ({n} of {total})", { n: next + 1, total: messages.length }));
        const dialog = h(
          "div",
          { class: "dialog mm-dialog mm-mailto", role: "dialog", "aria-modal": "true", "aria-label": t("Open in my mail app") },
          h("div", { class: "dialog-head" }, h("h2", {}, t("Open in my mail app")), h("p", { class: "sub" }, t("Choose Next email to open a message for the next recipient. Send it in your mail app, then come back here."))),
          h("div", { class: "dialog-body" }, status),
          h("div", { class: "dialog-actions" }, stop, go),
        );
        let finished = false;
        const finish = () => {
          if (finished) return;
          finished = true;
          close();
          // The ones that were not opened are not done.
          for (let i = next; i < messages.length; i++) ctx.progress(i, { ok: false, error: t("Not opened.") });
          resolve();
        };
        const close = modal(dialog, finish);
        const paint = () => {
          label();
          status.textContent = [next ? tn(next, "Opened {n} message.", "Opened {n} messages.") : "", cutCount ? tn(cutCount, "{n} message was long, so only its start is in the mail app.", "{n} messages were long, so only the start of each is in the mail app.") : ""].filter(Boolean).join(" ");
        };
        go.addEventListener("click", () => {
          const link = mailtoLink(messages[next]);
          if (link.cut) cutCount++;
          const a = document.createElement("a");
          a.href = link.url;
          a.rel = "noopener";
          // A mail app on the web (Gmail as the mailto handler) opens in a new tab instead of taking this one away.
          a.target = "_blank";
          document.body.append(a);
          a.click();
          a.remove();
          ctx.progress(next, { ok: true });
          next++;
          if (next >= messages.length) finish();
          else paint();
        });
        stop.addEventListener("click", finish);
        ctx.signal.addEventListener("abort", finish, { once: true });
        paint();
        go.focus();
      }),
  };
}
