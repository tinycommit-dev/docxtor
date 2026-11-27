// Tools, Keyboard shortcuts: every command with its shortcut, and a way to give any command a shortcut of your own.
// Choosing a key that another command uses asks first, and takes it from that command. The choices are kept by the
// extension that opens this dialog (`save`), and the host (`ExtensionHost.setShortcuts`) makes them work.
import { specFromEvent, formatChord, t, tn, type Command, type ExtensionHost, type TakenKey } from "@docxtor/extensions";
import { h, modal } from "./dom";

export function openShortcuts(opts: { host: ExtensionHost; save: (overrides: Record<string, string[]>, taken: TakenKey[]) => void; onClose?: () => void }) {
  const { host } = opts;
  const mac = host.mac;
  const search = h("input", { type: "search", class: "kb-search", "aria-label": t("Search commands"), placeholder: t("Search commands"), autocomplete: "off", spellcheck: "false" });
  const list = h("div", { class: "kb-list", role: "list" });
  const note = h("p", { class: "kb-note" });
  /** The command waiting for a key, and a key another command has that the person has yet to confirm. */
  let capturing: string | null = null;
  let clash: { id: string; spec: string; users: Command[] } | null = null;

  const write = (id: string, specs: string[] | null) => {
    host.setShortcuts(id, specs);
    opts.save(host.shortcutOverrides, host.shortcutTaken);
  };
  const keys = (id: string) => host.shortcutSpecs(id).map((k) => formatChord(k, mac));

  const row = (cmd: Command): HTMLElement => {
    const own = cmd.id in host.shortcutOverrides;
    const shown = keys(cmd.id);
    const chips = shown.length ? shown.map((k) => h("kbd", { class: "kbd" }, k)) : [h("span", { class: "kb-none" }, t("None"))];
    const change = h("button", { class: "btn ghost", type: "button", "aria-label": t("Change the shortcut of {command}", { command: cmd.title }) }, t("Change"));
    change.addEventListener("click", () => {
      capturing = cmd.id;
      clash = null;
      draw();
    });
    const actions: HTMLElement[] = [change];
    if (shown.length) {
      const remove = h("button", { class: "btn ghost", type: "button", "aria-label": t("Remove the shortcut of {command}", { command: cmd.title }) }, t("Remove"));
      remove.addEventListener("click", () => {
        write(cmd.id, []);
        draw();
      });
      actions.push(remove);
    }
    if (own) {
      const reset = h("button", { class: "btn ghost", type: "button", "aria-label": t("Reset the shortcut of {command}", { command: cmd.title }) }, t("Reset"));
      reset.addEventListener("click", () => {
        write(cmd.id, null);
        draw();
      });
      actions.push(reset);
    }
    const head = h("div", { class: "kb-name" }, h("span", {}, cmd.title), cmd.category ? h("span", { class: "kb-cat" }, cmd.category) : null);
    const r = h("div", { class: `kb-row${capturing === cmd.id ? " capturing" : ""}`, role: "listitem" }, head, h("div", { class: "kb-keys" }, ...chips), h("div", { class: "kb-actions" }, ...actions));
    if (capturing === cmd.id) {
      const say = h("p", { class: "kb-capture", role: "status" });
      if (clash) {
        const who = clash.users.map((c) => c.title).join(", ");
        say.textContent = t("{keys} is used by {command}. Use it here instead?", { keys: formatChord(clash.spec, mac), command: who });
        const yes = h("button", { class: "btn primary", type: "button" }, t("Use it here"));
        const no = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
        yes.addEventListener("click", () => {
          if (!clash) return;
          write(cmd.id, [clash.spec]);
          capturing = null;
          clash = null;
          draw();
        });
        no.addEventListener("click", () => {
          capturing = null;
          clash = null;
          draw();
        });
        r.append(say, h("div", { class: "kb-actions" }, yes, no));
      } else {
        say.textContent = t("Press the keys you want to use, or Escape to leave it.");
        r.append(say);
      }
    }
    return r;
  };

  const draw = () => {
    const q = search.value.trim().toLowerCase();
    const all = host.paletteCommands();
    const hits = (q ? all.filter((c) => `${c.title} ${c.category ?? ""}`.toLowerCase().includes(q)) : all.filter((c) => host.shortcutSpecs(c.id).length || c.id in host.shortcutOverrides || c.id === capturing)).slice(0, 120);
    list.replaceChildren(...hits.map(row));
    note.textContent = q ? (hits.length ? tn(hits.length, "{n} command", "{n} commands") : t("No command matches.")) : t("Search for any command to give it a shortcut.");
    list.querySelector<HTMLElement>(".kb-row.capturing .btn.primary")?.focus();
  };

  const done = h("button", { class: "btn primary", type: "button" }, t("Done"));
  const resetAll = h("button", { class: "btn ghost", type: "button" }, t("Reset all"));
  const dialog = h(
    "div",
    { class: "dialog kb-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Keyboard shortcuts") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Keyboard shortcuts")), h("p", { class: "sub" }, t("Every command can have a shortcut. Yours replace the ones a command came with."))),
    h("div", { class: "dialog-body" }, search, note, list),
    h("div", { class: "dialog-actions" }, resetAll, h("span", { style: "flex:1" }), done),
  );
  // Listening before the dialog does, so Escape while a command waits for keys ends the wait and not the dialog.
  document.addEventListener("keydown", capture, true);
  const close = modal(dialog, () => {
    document.removeEventListener("keydown", capture, true);
    opts.onClose?.();
  });

  // While a command waits for a key, the keys go to the dialog and not to the commands.
  function capture(e: KeyboardEvent) {
    if (!capturing || clash) return;
    if (e.key === "Escape" && !e.metaKey && !e.ctrlKey && !e.altKey) {
      e.preventDefault();
      e.stopImmediatePropagation();
      capturing = null;
      draw();
      return;
    }
    const spec = specFromEvent(e, mac);
    if (!spec) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    const users = host.shortcutUsers(spec, capturing);
    if (users.length) {
      clash = { id: capturing, spec, users };
    } else {
      write(capturing, [spec]);
      capturing = null;
    }
    draw();
  }
  search.addEventListener("input", draw);
  done.addEventListener("click", close);
  resetAll.addEventListener("click", async () => {
    if (!(await host.ask("shortcuts.resetAll", Object.keys(host.shortcutOverrides).length))) return;
    host.loadShortcutOverrides({});
    opts.save({}, []);
    capturing = null;
    clash = null;
    draw();
  });
  draw();
  search.focus();
}
