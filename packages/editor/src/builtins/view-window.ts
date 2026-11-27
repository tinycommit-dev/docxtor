// View: the Draft view, Immersive (Focus, Line focus), the page guides (Gridlines, Text boundaries) and the Window group (Split,
// Remove split). The Draft view is a `ctx.views` contribution like the other layouts. Line focus, Gridlines and Text
// boundaries work on the pane with the keyboard, or on every pane where a setting belongs to the document. New window,
// View side by side and the other commands that need the app's files and tabs are the app's (web/src/builtins.ts).
import { defineExtension, msg, t, type Extension } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { LineFocus, type LineCount } from "../editor/line-focus";
import { VIEW_WINDOW_ICONS } from "../ui/view-window-icons";
import type { Internals } from "./index";

export function viewWindow({ fe, editor }: Internals): Extension {
  return defineExtension({
    id: "docxtor.view-window",
    name: msg("Draft view, line focus and split window"),
    description: msg("The Draft view, Focus and Line focus, gridlines and text boundaries, and splitting the window in two panes of one document."),
    activate(ctx) {
      const open = () => !!fe.engine;
      const e = editor;
      for (const [id, svg] of Object.entries(VIEW_WINDOW_ICONS)) ctx.icons.register({ id, svg });
      const cmd = (id: string, title: string, run: () => void, opts: { keybinding?: string; toggled?: () => boolean; icon?: string; when?: () => boolean; palette?: boolean } = {}) =>
        ctx.commands.register(id, run, { title, category: t("View"), when: opts.when ?? open, keybinding: opts.keybinding, toggled: opts.toggled, icon: opts.icon, palette: opts.palette });

      // -- the Draft view ---------------------------------------------------------------

      // Text without page boxes, headers, footers, floating pictures or columns; page and section breaks are labelled rules.
      ctx.views.register({
        id: "draft",
        title: msg("Draft"),
        icon: "draftView",
        order: 50,
        layout: "continuous",
        width: "window",
        maxWidth: 1200,
        margin: 24,
        marginTop: 16,
        className: "draft",
        description: msg("Just the text: no page boxes, headers, footers, floating pictures or columns, and breaks shown as labelled lines."),
        enter() {
          e.showBreaks(true);
          return () => e.showBreaks(false);
        },
      });

      // -- Focus --------------------------------------------------------------------------

      let focusMode = false;
      const setFocusMode = (on: boolean) => {
        focusMode = on;
        fe.surface.closest(".docxtor")?.classList.toggle("focus-mode", on);
        if (fe.menuBarEl) fe.menuBarEl.hidden = on;
        if (fe.toolbarEl) fe.toolbarEl.hidden = on || fe.readOnly;
        e.focus();
      };
      cmd("view.focusMode", t("Focus"), () => setFocusMode(!focusMode), { icon: "focusMode", toggled: () => focusMode });
      cmd("view.exitFocusMode", t("Leave focus"), () => setFocusMode(false), { keybinding: "Escape", when: () => open() && focusMode, icon: "focusMode", palette: false });
      ctx.subscriptions.push({ dispose: () => focusMode && setFocusMode(false) });

      // -- Line focus -----------------------------------------------------------------------

      const lineFocus = new Map<Editor, { tool: LineFocus; release: () => void }>();
      let lines: LineCount = ctx.state.get<LineCount>("lineFocusLines", 3);
      const exitLineFocus = (pane: Editor) => {
        const on = lineFocus.get(pane);
        if (!on) return;
        on.tool.destroy();
        lineFocus.delete(pane);
        on.release();
      };
      const currentPane = (): Editor => fe.activePane;
      const toggleLineFocus = () => {
        const active = currentPane();
        if (lineFocus.has(active)) return exitLineFocus(active);
        const tool = new LineFocus(active, fe.split.hostOf(active), {
          count: lines,
          onExit: () => exitLineFocus(active),
          onCount: (n) => ((lines = n), ctx.state.set("lineFocusLines", n)),
        });
        // Its own reason on its own pane: ending Line focus lets go of this and nothing another feature holds.
        lineFocus.set(active, { tool, release: active.lock("line focus") });
      };
      cmd("view.lineFocus", t("Line focus"), toggleLineFocus, { icon: "lineFocus", toggled: () => lineFocus.has(currentPane()) });
      for (const n of [1, 3, 5] as const) {
        const label = n === 1 ? t("Line focus, one line") : n === 3 ? t("Line focus, three lines") : t("Line focus, five lines");
        cmd(`view.lineFocus.${n}`, label, () => {
          const pane = currentPane();
          if (!lineFocus.has(pane)) toggleLineFocus();
          lineFocus.get(pane)?.tool.setLines(n);
        }, { icon: "lineFocus", toggled: () => lineFocus.get(currentPane())?.tool.lines === n, when: open });
      }
      // The split came or went: the panes moved, so the dimming starts again.
      ctx.subscriptions.push({ dispose: fe.on("split", () => [...lineFocus.keys()].forEach(exitLineFocus)) });
      ctx.subscriptions.push({ dispose: () => [...lineFocus.keys()].forEach(exitLineFocus) });

      // -- Gridlines and text boundaries ----------------------------------------------------------

      const guide = (id: string, title: string, key: string, icon: string, set: (on: boolean) => void, get: () => boolean) => {
        cmd(id, title, () => {
          set(!get());
          ctx.state.set(key, get());
        }, { icon, toggled: get });
        // The setting the person left last time.
        if (ctx.state.get(key, false)) set(true);
        ctx.subscriptions.push({ dispose: () => set(false) });
      };
      guide("view.gridlines", t("Gridlines"), "gridlines", "gridlines", (on) => e.showGridlines(on), () => e.guides.gridlines);
      guide("view.textBoundaries", t("Text boundaries"), "textBoundaries", "textBoundaries", (on) => e.showBoundaries(on), () => e.guides.boundaries);

      // -- Split ------------------------------------------------------------------------------------

      cmd("view.split", t("Split"), () => (fe.split.on ? fe.split.close() : fe.split.open()), { keybinding: "Mod+Alt+S", icon: "splitWindow", toggled: () => fe.split.on });
      cmd("view.removeSplit", t("Remove split"), () => fe.split.close(), { icon: "removeSplit", when: () => open() && fe.split.on });
      ctx.subscriptions.push({ dispose: () => fe.split.close() });
    },
  });
}
