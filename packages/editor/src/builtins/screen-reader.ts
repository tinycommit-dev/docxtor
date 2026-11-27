// Screen reader support. A long document draws only the pages near the view, to stay fast (`farKeep`): the
// rest is out of the browser's layout, so a screen reader's "read all" stops at the edge of the drawn pages and the browser's own find does
// not see it. With screen reader support on, every page is drawn, as Google Docs' own screen reader mode does (and with its shortcut).
import { defineExtension, msg, parseChord, t, type Extension } from "@docxtor/extensions";
import { toast } from "../ui/dom";
import type { Internals } from "./index";

/** A shortcut as a screen reader should say it: "Ctrl+Alt+Z", "Command+Option+Z" (the symbols ⌘⌥ are read badly or not at all). */
function spoken(spec: string, mac: boolean): string {
  const c = parseChord(spec, mac);
  const key = c.key.length === 1 ? c.key.toUpperCase() : c.key[0].toUpperCase() + c.key.slice(1);
  const mods = mac ? [c.meta && "Command", c.ctrl && "Control", c.alt && "Option", c.shift && "Shift"] : [c.ctrl && "Ctrl", c.meta && "Win", c.alt && "Alt", c.shift && "Shift"];
  return [...mods, key].filter(Boolean).join("+");
}

export function screenReader({ fe, editor }: Internals): Extension {
  return defineExtension({
    id: "docxtor.screenReader",
    name: msg("Screen reader support"),
    description: msg("Draws every page of a long document, so a screen reader can read all of it and the browser's find sees all of it."),
    activate(ctx) {
      // While it is off, the text says how to turn it on, as Google Docs does: a screen reader reads the description when the text gets the
      // keyboard. The shortcut is the one the person has now (they can change it).
      const describe = () => {
        const flow = editor.flow;
        const spec = fe.extensions.shortcutSpecs("tools.screenReader")[0];
        if (editor.drawEveryPage || !spec) flow.removeAttribute("aria-description");
        else flow.setAttribute("aria-description", t("A long document shows only the pages near the view. To have a screen reader read every page, turn on screen reader support with {keys}.", { keys: spoken(spec, fe.extensions.mac) }));
      };
      const set = (on: boolean) => {
        editor.drawEveryPage = on;
        ctx.state.set("on", on);
        describe();
      };
      editor.drawEveryPage = ctx.state.get("on", false);
      describe();
      const onFocus = () => describe();
      editor.flow.addEventListener("focusin", onFocus);
      ctx.subscriptions.push({ dispose: () => editor.flow.removeEventListener("focusin", onFocus) });
      ctx.commands.register(
        "tools.screenReader",
        () => {
          set(!editor.drawEveryPage);
          toast(editor.drawEveryPage ? t("Screen reader support is on: every page is drawn") : t("Screen reader support is off"));
        },
        { title: t("Screen reader support"), category: t("Tools"), keybinding: "Mod+Alt+Z", toggled: () => editor.drawEveryPage },
      );
      ctx.settings.register({
        id: "docxtor.screenReader",
        group: t("Accessibility"),
        label: t("Screen reader support"),
        help: t("A long document draws only the pages near the view, to stay fast. With this on, every page is drawn, so a screen reader can read the whole document and the browser's own find sees all of it. Typing in a long document is slower."),
        kind: "toggle",
        get: () => editor.drawEveryPage,
        set: (v) => set(!!v),
      });
    },
  });
}
