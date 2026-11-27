// Password dialogs: asking for the password of a protected document, and File > Protect with password.
// The password is held in memory only. It is never put in storage, the address or any state the app saves.
import { h, modal, t, WrongPasswordError } from "@docxtor/editor";

let seq = 0;

/**
 * Ask for the password of a protected document until `attempt` accepts it or the person cancels. `attempt` gets the
 * password and returns what opening produced (it throws a `WrongPasswordError` when the password isn't right: the
 * dialog then says so and stays open). Resolves null when the person cancels.
 */
export function askPassword<T>(name: string, attempt: (password: string) => T | Promise<T>): Promise<{ value: T; password: string } | null> {
  return new Promise((resolve) => {
    const id = `pw${++seq}`;
    let done = false;
    const input = h("input", { type: "password", id: `${id}-input`, autocomplete: "off", spellcheck: "false", "data-pw": "open" });
    const error = h("p", { class: "bm-error", role: "alert", hidden: true });
    const ok = h("button", { class: "btn primary", type: "submit" }, t("Open"));
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const showError = (message: string) => {
      error.textContent = message;
      error.hidden = !message;
      if (message) input.setAttribute("aria-invalid", "true");
      else input.removeAttribute("aria-invalid");
    };
    const finish = (v: { value: T; password: string } | null) => {
      if (done) return;
      done = true;
      close();
      resolve(v);
    };
    const form = h(
      "form",
      { novalidate: "" },
      h(
        "div",
        { class: "dialog-head" },
        h("h2", { id: `${id}-title` }, t("Password needed")),
        h("p", { class: "sub" }, t("This document is protected with a password. Enter it to open it.")),
        h("p", { class: "sub" }, h("b", { "data-pw": "name" }, name)),
      ),
      h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", { for: `${id}-input` }, t("Password")), input, error)),
      h("div", { class: "dialog-actions" }, cancel, ok),
    );
    const dialog = h("div", { class: "dialog", role: "dialog", "aria-modal": "true", "aria-labelledby": `${id}-title`, style: "width:min(var(--dialog-s),100%)" }, form);
    const close = modal(dialog, () => finish(null));
    input.addEventListener("input", () => showError(""));
    cancel.addEventListener("click", () => finish(null));
    form.addEventListener("submit", async (e) => {
      e.preventDefault();
      if (!input.value) {
        input.focus();
        return;
      }
      const password = input.value;
      showError("");
      ok.disabled = true;
      ok.textContent = t("Opening…");
      // Let the button repaint: working out the key takes a moment.
      await new Promise((r) => setTimeout(r, 30));
      try {
        finish({ value: await attempt(password), password });
      } catch (err) {
        ok.disabled = false;
        ok.textContent = t("Open");
        if (err instanceof WrongPasswordError || (err as { code?: string }).code === "wrong_password") {
          showError(t("That password isn't right. Try again."));
          input.focus();
          input.select();
        } else {
          showError((err as Error).message);
        }
      }
    });
    setTimeout(() => input.focus());
  });
}

export interface ProtectOptions {
  /** The document's name, for the dialog's description. */
  name: string;
  /** Whether the document has a password now (the dialog then offers to remove it). */
  hasPassword: boolean;
  /** Ask before a risky action, by its caution id (web/src/cautions.ts). */
  ask(id: string, ...args: unknown[]): Promise<boolean>;
}

/**
 * File > Protect with password: choose a new password (typed twice), or remove the one the document has.
 * Resolves `{ password }` to set or change it, `{ password: null }` to remove it, and null when cancelled.
 */
export function protectDialog(o: ProtectOptions): Promise<{ password: string | null } | null> {
  return new Promise((resolve) => {
    const id = `pw${++seq}`;
    let done = false;
    const first = h("input", { type: "password", id: `${id}-new`, autocomplete: "new-password", spellcheck: "false", "data-pw": "new" });
    const second = h("input", { type: "password", id: `${id}-confirm`, autocomplete: "new-password", spellcheck: "false", "data-pw": "confirm" });
    const error = h("p", { class: "bm-error", role: "alert", hidden: true });
    const ok = h("button", { class: "btn primary", type: "submit" }, o.hasPassword ? t("Change password") : t("Protect"));
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const remove = o.hasPassword ? h("button", { class: "btn", type: "button", "data-pw": "remove" }, t("Remove password")) : null;
    const showError = (message: string, field?: HTMLInputElement) => {
      error.textContent = message;
      error.hidden = !message;
      for (const f of [first, second]) {
        if (message && f === field) f.setAttribute("aria-invalid", "true");
        else f.removeAttribute("aria-invalid");
      }
    };
    const finish = (v: { password: string | null } | null) => {
      if (done) return;
      done = true;
      close();
      resolve(v);
    };
    const form = h(
      "form",
      { novalidate: "" },
      h(
        "div",
        { class: "dialog-head" },
        h("h2", { id: `${id}-title` }, t("Protect with password")),
        h("p", { class: "sub" }, o.hasPassword ? t("This document has a password. Type a new one to change it, or remove it.") : t("Anyone who opens this document will need the password. Docxtor can't recover it for you, so keep it somewhere safe.")),
        h("p", { class: "sub" }, h("b", {}, o.name)),
      ),
      h(
        "div",
        { class: "dialog-body" },
        h("div", { class: "field" }, h("label", { for: `${id}-new` }, t("New password")), first),
        h("div", { class: "field" }, h("label", { for: `${id}-confirm` }, t("Confirm password")), second, error),
      ),
      h("div", { class: "dialog-actions" }, ...(remove ? [remove] : []), cancel, ok),
    );
    const dialog = h("div", { class: "dialog", role: "dialog", "aria-modal": "true", "aria-labelledby": `${id}-title`, style: "width:min(var(--dialog-s),100%)" }, form);
    const close = modal(dialog, () => finish(null));
    for (const f of [first, second]) f.addEventListener("input", () => showError(""));
    cancel.addEventListener("click", () => finish(null));
    remove?.addEventListener("click", async () => {
      if (await o.ask("password.remove")) finish({ password: null });
    });
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      if (!first.value) return showError(t("Type a password."), first), first.focus();
      if (first.value !== second.value) return showError(t("The passwords don't match."), second), second.focus();
      finish({ password: first.value });
    });
    setTimeout(() => first.focus());
  });
}
