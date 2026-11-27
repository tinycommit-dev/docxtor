// Design, as an extension: themes, colour schemes, font schemes, style sets and paragraph spacing. Docxtor's own themes
// and style sets are registered through the same contribution points an extension uses (`ctx.documentThemes`,
// `ctx.styleSets`), so the galleries list Word's and anyone else's together. The engine holds Word's colours, fonts and
// style set definitions (`Engine.themeLibrary()`, `Engine.styleSetInfos()`); this adds them to the points once it is loaded.
import { Engine, type DesignSnapshot } from "@docxtor/core";
import { defineExtension, msg, t, type Extension, type ExtensionContext } from "@docxtor/extensions";
import { designAnchor, openColors, openCustomizeColors, openCustomizeFonts, openFonts, openSpacing, openStyleSets, openThemes, applySpacing, presetName } from "../ui/design";
import { DESIGN_ICONS } from "../ui/design-icons";
import type { Internals } from "./index";

/** The design new documents start with, as the host kept it (null if none or unusable). The editor stores nothing itself: the host's extension state does. */
function readDefaultDesign(ctx: ExtensionContext): DesignSnapshot | null {
  const v = ctx.state.get<DesignSnapshot | null>("defaultDesign", null);
  return v && v.theme && v.styleSet ? v : null;
}

function writeDefaultDesign(ctx: ExtensionContext, d: DesignSnapshot | null) {
  ctx.state.set("defaultDesign", d);
  Engine.defaultDesign = d;
}

/** Word's paragraph spacing presets, in the order of its menu (the engine gives their numbers). */
const SPACING = [
  { id: "default", name: msg("Default") },
  { id: "none", name: msg("No paragraph space") },
  { id: "compact", name: msg("Compact") },
  { id: "tight", name: msg("Tight") },
  { id: "open", name: msg("Open") },
  { id: "relaxed", name: msg("Relaxed") },
  { id: "double", name: msg("Double") },
];

/** The built-in style sets, named and described for the gallery (the engine's own words are English). */
const SETS: Record<string, { name: string; description: string }> = {
  "word-2013": { name: msg("Word 2013+"), description: msg("Light headings in dark blue and a little room after each paragraph.") },
  "word-2010": { name: msg("Word 2010"), description: msg("Bold blue Cambria headings and a rule under the title.") },
  "word-2003": { name: msg("Word 2003"), description: msg("Times New Roman text, Arial headings and single spacing.") },
  "basic-elegant": { name: msg("Basic (Elegant)"), description: msg("Garamond with small capitals in the headings.") },
  "basic-simple": { name: msg("Basic (Simple)"), description: msg("Calibri throughout, headings in bold black.") },
  "basic-stylish": { name: msg("Basic (Stylish)"), description: msg("A centered title between two rules and spaced capitals in the headings.") },
  "bw-classic": { name: msg("Black & White (Classic)"), description: msg("Times New Roman, everything in black.") },
  "bw-modern": { name: msg("Black & White (Modern)"), description: msg("Sans-serif headings in black with a heavy rule under the title.") },
  "bw-word-2003": { name: msg("Black & White (Word 2003)"), description: msg("Word 2003 headings in black.") },
  casual: { name: msg("Casual"), description: msg("Rounded, friendly type in warm colors.") },
  centered: { name: msg("Centered"), description: msg("The title and every heading centered.") },
  "lines-distinctive": { name: msg("Lines (Distinctive)"), description: msg("A heavy rule above and below the title and a rule under each main heading.") },
  "lines-simple": { name: msg("Lines (Simple)"), description: msg("A single rule under the title and under each main heading.") },
  "lines-stylish": { name: msg("Lines (Stylish)"), description: msg("Capitals and a rule above and below the title and above each main heading.") },
  minimalist: { name: msg("Minimalist"), description: msg("Light type, plenty of room and no color but grey.") },
  shaded: { name: msg("Shaded"), description: msg("Main headings and the title in white on a band of the accent color.") },
};

const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, "-");

/** Add Word's themes, colour schemes, font schemes and style sets to the contribution points. */
function offerLibrary(ctx: ExtensionContext): boolean {
  let lib;
  let sets;
  try {
    lib = Engine.themeLibrary();
    sets = Engine.styleSetInfos();
  } catch {
    return false; // the engine is not loaded yet
  }
  for (const th of lib.themes) ctx.documentThemes.register({ id: `docxtor.theme.${th.id}`, label: th.name, colors: th.colors.colors, fonts: { major: th.fonts.major, minor: th.fonts.minor } });
  for (const c of lib.colorSchemes) ctx.documentThemes.register({ id: `docxtor.colors.${slug(c.name)}`, label: c.name, colors: c.colors });
  for (const f of lib.fontSchemes) ctx.documentThemes.register({ id: `docxtor.fonts.${slug(f.name)}`, label: f.name, fonts: { major: f.major, minor: f.minor } });
  for (const s of sets) ctx.styleSets.register({ id: s.id, label: SETS[s.id] ? t(SETS[s.id].name) : s.name, description: SETS[s.id] ? t(SETS[s.id].description) : s.description });
  return true;
}

export function design({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.design",
    name: msg("Design"),
    description: msg("Themes, colors, fonts and style sets for the whole document, and the spacing between paragraphs. Word's themes and style sets are offered through the same points an extension uses to add its own."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(DESIGN_ICONS)) ctx.icons.register({ id, svg });
      const editable = () => !!fe.engine && !fe.readOnly;
      const dctx = () => ({ ed: e, host: fe.extensions });
      Engine.defaultDesign = readDefaultDesign(ctx);
      if (!offerLibrary(ctx)) {
        // The engine loads after the editor is made; offer the library with the first document.
        const sub = ctx.document.onDidOpen(() => {
          if (offerLibrary(ctx)) sub.dispose();
        });
        ctx.subscriptions.push(sub);
      }

      const open = (id: string, title: string, icon: string, run: () => void) => ctx.commands.register(id, run, { title, category: t("Design"), when: editable, icon });
      open("design.themes", t("Themes…"), "designThemes", () => openThemes(dctx(), designAnchor(e)));
      open("design.colors", t("Colors…"), "designColors", () => openColors(dctx(), designAnchor(e)));
      open("design.fonts", t("Fonts…"), "designFonts", () => openFonts(dctx(), designAnchor(e)));
      open("design.styleSet", t("Style set…"), "styleSet", () => openStyleSets(dctx(), designAnchor(e)));
      open("design.spacing", t("Paragraph spacing…"), "paragraphSpacing", () => openSpacing(dctx(), designAnchor(e)));
      open("design.customizeColors", t("Customize colors…"), "designColors", () => openCustomizeColors(dctx()));
      open("design.customizeFonts", t("Customize fonts…"), "designFonts", () => openCustomizeFonts(dctx()));
      // Every preset asks for the Normal style's spacing when the menu opens, in one run of the same task: read it once
      // for that run (the engine walks the document to describe a style) and read it again on the next one.
      let normalSpacing: { after: number; line: number } | null | undefined;
      const normalNow = () => {
        if (normalSpacing === undefined) {
          const normal = fe.engine?.styleDetails("Normal");
          normalSpacing = normal ? { after: normal.para.spaceAfter ?? 0, line: normal.para.line ?? 240 } : null;
          queueMicrotask(() => (normalSpacing = undefined));
        }
        return normalSpacing;
      };
      // The presets as commands of their own, so they sit in a menu and the palette and the assistant can use them.
      for (const p of SPACING) {
        ctx.commands.register(`design.spacing.${p.id}`, () => applySpacing(e, p.id), {
          title: t(p.name),
          category: t("Design"),
          when: editable,
          ai: true,
          toggled: () => {
            if (!fe.engine) return false;
            const preset = Engine.spacingPresets().find((x) => x.id === p.id);
            const normal = normalNow();
            return !!preset && !!normal && normal.after === preset.after && normal.line === preset.line;
          },
        });
      }
      ctx.commands.register(
        "design.setAsDefault",
        async (arg: unknown) => {
          if (!fe.engine) return;
          // Modify style's "New documents based on this template" asks without a second question.
          const quiet = !!(arg as { quiet?: boolean } | undefined)?.quiet;
          if (!quiet) {
            const ok = await ctx.ui.confirm(t("New documents will start with this document's theme, colors, fonts, style set and paragraph spacing. Documents you already have are not changed."), {
              title: t("Set as default"),
              confirm: t("Set as default"),
            });
            if (!ok) return;
          }
          writeDefaultDesign(ctx, fe.engine.designSnapshot());
          ctx.ui.notify(quiet ? t("New documents will start with this style too.") : t("New documents will start with this design."));
        },
        { title: t("Set as default"), category: t("Design"), when: editable, icon: "setDefault" },
      );
      ctx.commands.register(
        "design.resetDefault",
        () => {
          writeDefaultDesign(ctx, null);
          ctx.ui.notify(t("New documents start with Word's own design again."));
        },
        { title: t("Reset the default design"), category: t("Design"), when: () => !!Engine.defaultDesign },
      );

      const item = (menu: string, group: string, command: string, title?: string) => ctx.menus.register({ menu, group, command, title });
      for (const c of ["themes", "colors", "fonts"]) item("design", "1_look", `design.${c}`);
      item("design", "2_style", "design.styleSet");
      ctx.menus.register({ menu: "design", group: "2_style", submenu: "design/spacing", title: t("Paragraph spacing"), icon: "paragraphSpacing" });
      for (const p of SPACING) item("design/spacing", "1", `design.spacing.${p.id}`, presetName(p.name));
      item("design", "3_default", "design.setAsDefault");
      item("design", "3_default", "design.resetDefault");
      // Format, Paragraph styles gets the lines that edit styles.
      for (const c of ["createStyle", "newStyle", "modifyStyle", "updateStyle"]) item("format/styles", "4", `format.${c}`);
      item("format/styles", "5", "format.manageStyles");
    },
  });
}
