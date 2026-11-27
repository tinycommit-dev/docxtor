// View: the layouts (Print layout, Pageless, Reading, Outline), zoom (a factor, the page width, a whole page, several
// pages side by side), folding headings, the Outline view's commands, compact mode and the Navigation pane. The
// layouts are `ctx.views` contributions like any extension's; the rest are commands.
import { defineExtension, msg, t, type Disposable, type Extension, type ViewMode } from "@docxtor/extensions";
import { OutlineOps } from "../editor/outline-view";
import { PAGES_ACROSS, stepZoom, ZOOM_PRESETS } from "../editor/zoom";
import { NavigationPane } from "../ui/navigation";
import { OutlineBar } from "../ui/outline-bar";
import { VIEW_ICONS } from "../ui/view-icons";
import type { Internals } from "./index";

export function views({ fe, editor }: Internals): Extension {
  return defineExtension({
    id: "docxtor.views",
    name: msg("Views"),
    description: msg("Print layout, pageless, reading and outline views, zoom, folding headings, compact mode and the navigation pane."),
    activate(ctx) {
      const open = () => !!fe.engine;
      const e = editor;
      const outline = new OutlineOps(e);
      for (const [id, svg] of Object.entries(VIEW_ICONS)) ctx.icons.register({ id, svg });

      // -- the layouts ------------------------------------------------------------------

      ctx.views.register({ id: "print", title: msg("Print layout"), icon: "printLayout", order: 10, layout: "pages", description: msg("Pages as they print, with headers, footers and page breaks.") });
      ctx.views.register({ id: "pageless", title: msg("Pageless"), icon: "pageless", order: 20, layout: "continuous", width: "page", description: msg("One long sheet with no page breaks, headers or footers.") });
      ctx.views.register({
        id: "reading",
        title: msg("Reading"),
        icon: "reading",
        order: 30,
        layout: "continuous",
        width: "window",
        maxWidth: 720,
        margin: 40,
        marginTop: 40,
        readOnly: true,
        className: "reading",
        description: msg("The text reflows to the window in a comfortable column, to read and not to edit."),
        enter() {
          // A little larger, as a book is set; leaving puts the zoom back.
          const before = { mode: e.zoomer.mode, zoom: e.zoom };
          e.zoomer.set(1.15);
          return () => (before.mode === "custom" ? e.zoomer.set(before.zoom) : e.zoomer.setMode(before.mode));
        },
      });
      // One strip for each pane that is in the Outline view (a split window can have two).
      const bars = new Set<OutlineBar>();
      ctx.views.register({
        id: "outline",
        title: msg("Outline"),
        icon: "outlineView",
        order: 40,
        keybinding: "Mod+Alt+O",
        layout: "continuous",
        width: "window",
        maxWidth: 1000,
        margin: 32,
        marginLeft: 56,
        marginTop: 24,
        className: "outline",
        description: msg("Headings and text at their levels: promote, demote and move whole sections."),
        enter({ root }) {
          e.folds.outline = true;
          e.keyHook = (ev) => outline.key(ev);
          const bar = new OutlineBar(fe.extensions, outline, () => fe.setView("print"));
          bars.add(bar);
          root.prepend(bar.el);
          const sync = fe.on("layout", () => bar.sync());
          e.foldsChanged();
          return () => {
            sync();
            bar.destroy();
            bars.delete(bar);
            e.keyHook = null;
            e.folds.outline = false;
            e.folds.levels = 0;
            e.folds.firstLine = false;
            e.foldsChanged();
          };
        },
      });

      // A command and a menu line for each view, including the ones extensions add later.
      const made = new Map<string, Disposable[]>();
      const sync = () => {
        const present = new Set(fe.views().map((v) => v.id));
        for (const [id, disposables] of made) {
          if (present.has(id)) continue;
          disposables.forEach((d) => d.dispose());
          made.delete(id);
        }
        for (const v of fe.views()) {
          if (made.has(v.id)) continue;
          const id = `view.mode.${v.id}`;
          made.set(v.id, [
            ctx.commands.register(id, () => void fe.setView(v.id), { title: t(v.title), category: t("View"), when: open, icon: v.icon, keybinding: v.keybinding, toggled: () => fe.view === v.id }),
            ...(["print", "pageless", "reading", "outline", "draft"].includes(v.id) ? [] : [ctx.menus.register({ menu: "view/layout", group: "2", command: id })]),
          ]);
        }
      };
      ctx.subscriptions.push(fe.extensions.views.onChange(sync));
      sync();
      ctx.subscriptions.push({ dispose: () => fe.setView("print") });
      ctx.commands.register("view.exitView", () => void fe.setView("print"), { title: t("Back to the print layout"), category: t("View"), keybinding: "Escape", when: () => open() && fe.view === "reading", palette: false });

      // -- zoom -------------------------------------------------------------------------

      const zoom = e.zoomer;
      const cmd = (id: string, title: string, run: () => void, opts: { keybinding?: string; toggled?: () => boolean; icon?: string } = {}) =>
        ctx.commands.register(id, run, { title, category: t("View"), when: open, ...opts });
      cmd("view.zoomIn", t("Zoom in"), () => zoom.set(stepZoom(e.zoom, 1), "centre"), { keybinding: "Mod+Alt+=", icon: "zoomIn" });
      cmd("view.zoomOut", t("Zoom out"), () => zoom.set(stepZoom(e.zoom, -1), "centre"), { keybinding: "Mod+Alt+-", icon: "zoomOut" });
      cmd("view.zoomReset", t("Actual size (100%)"), () => zoom.set(1, "centre"), { icon: "actualSize", toggled: () => zoom.mode === "custom" && e.zoom === 1 });
      cmd("view.fitWidth", t("Page width"), () => zoom.setMode("width"), { icon: "fitWidth", toggled: () => zoom.mode === "width" });
      cmd("view.zoomWholePage", t("Whole page"), () => zoom.setMode("page"), { icon: "zoomPage", toggled: () => zoom.mode === "page" });
      for (const n of PAGES_ACROSS) cmd(`view.zoomPages.${n}`, t("{n} pages across", { n }), () => zoom.setMode("pages", n), { icon: "zoomPages", toggled: () => zoom.mode === "pages" && zoom.across === n });
      for (const z of ZOOM_PRESETS) {
        const pct = Math.round(z * 100);
        cmd(`view.zoomTo.${pct}`, t("{percent}%", { percent: pct }), () => zoom.set(z, "centre"), { toggled: () => zoom.mode === "custom" && Math.abs(e.zoom - z) < 0.005 });
      }
      ctx.commands.register(
        "view.zoomCustom",
        async () => {
          const answer = await ctx.ui.prompt(t("Zoom (a percentage from 10 to 500)"), { value: String(Math.round(e.zoom * 100)), confirm: t("Zoom") });
          const n = Number(String(answer ?? "").replace(/[^0-9.]/g, ""));
          if (answer !== null && n > 0) zoom.set(n / 100, "centre");
        },
        { title: t("Custom zoom…"), category: t("View"), when: open, icon: "zoomIn" },
      );

      // -- headings that fold -----------------------------------------------------------

      const inHeadings = () => open() && !e.story;
      ctx.commands.register(
        "view.toggleHeading",
        () => {
          const r = e.range();
          const block = r && e.topBlockId(r[0].p);
          const entries = outline.entries();
          let i = entries.findIndex((x) => x.id === block);
          while (i > 0 && !entries[i].level) i--;
          const head = entries[i];
          if (!head?.level) return;
          const el = e.flow.querySelector<HTMLElement>(`[data-id="${head.id}"]`);
          if (!el || (!e.folds.isFolded(head.id) && !e.folds.hasContent(el))) return;
          e.toggleFold(head.id);
        },
        { title: t("Collapse or expand heading"), category: t("View"), when: inHeadings, icon: "foldHeadings" },
      );
      ctx.commands.register("view.collapseHeadings", () => (e.folds.foldAll(), e.foldsChanged()), { title: t("Collapse all headings"), category: t("View"), when: inHeadings, icon: "collapse" });
      ctx.commands.register("view.expandHeadings", () => outline.expandAll(), { title: t("Expand all headings"), category: t("View"), when: inHeadings, icon: "expand" });

      // -- the Outline view's commands ----------------------------------------------------

      const inOutline = () => open() && fe.view === "outline" && !e.story;
      const edits = () => inOutline() && !fe.readOnly;
      const oc = (id: string, title: string, run: () => unknown, opts: { keybinding?: string; icon: string; when?: () => boolean; toggled?: () => boolean }) =>
        ctx.commands.register(id, () => void run(), { title, category: t("Outline"), when: opts.when ?? edits, keybinding: opts.keybinding, icon: opts.icon, toggled: opts.toggled });
      oc("outline.promote", t("Promote"), () => outline.promote(), { keybinding: "Alt+Shift+ArrowLeft", icon: "promote" });
      oc("outline.demote", t("Demote"), () => outline.demote(), { keybinding: "Alt+Shift+ArrowRight", icon: "demote" });
      oc("outline.toBody", t("Demote to body text"), () => outline.toBody(), { icon: "demoteBody" });
      oc("outline.toHeading1", t("Make heading 1"), () => outline.toHeading1(), { icon: "promote" });
      oc("outline.moveUp", t("Move up"), () => outline.moveUp(), { keybinding: "Alt+Shift+ArrowUp", icon: "moveUp" });
      oc("outline.moveDown", t("Move down"), () => outline.moveDown(), { keybinding: "Alt+Shift+ArrowDown", icon: "moveDown" });
      oc("outline.expand", t("Expand"), () => outline.expand(), { keybinding: "Alt+Shift+=", icon: "expand", when: inOutline });
      oc("outline.collapse", t("Collapse"), () => outline.collapse(), { keybinding: "Alt+Shift+-", icon: "collapse", when: inOutline });
      oc("outline.showAll", t("Show all levels"), () => outline.showLevel(0), { keybinding: "Alt+Shift+A", icon: "outlineView", when: inOutline, toggled: () => inOutline() && outline.level === 0 });
      for (let n = 1; n <= 9; n++) oc(`outline.showLevel.${n}`, t("Show level {n}", { n }), () => outline.showLevel(n), { keybinding: `Alt+Shift+${n}`, icon: "outlineView", when: inOutline, toggled: () => inOutline() && outline.level === n });
      oc("outline.firstLine", t("Show first line only"), () => (outline.setFirstLineOnly(!outline.firstLineOnly), bars.forEach((b) => b.sync())), { keybinding: "Alt+Shift+L", icon: "firstLine", when: inOutline, toggled: () => inOutline() && outline.firstLineOnly });

      // -- compact mode -------------------------------------------------------------------

      ctx.commands.register(
        "view.compact",
        () => {
          fe.setCompact(!fe.compact);
          ctx.state.set("compact", fe.compact);
        },
        { title: t("Compact mode"), category: t("View"), keybinding: "Ctrl+Shift+F", icon: "compact", toggled: () => fe.compact },
      );
      fe.setCompact(ctx.state.get("compact", false));
      ctx.subscriptions.push({ dispose: () => fe.setCompact(false) });

      // -- the Navigation pane ------------------------------------------------------------

      ctx.panels.register({
        id: "navigation",
        title: t("Navigation"),
        order: 20,
        render: (el) => {
          const pane = new NavigationPane(fe, e, outline);
          el.append(pane.el);
          return () => pane.destroy();
        },
        onShow: () => void 0,
      });
    },
  });
}

export type { ViewMode };
