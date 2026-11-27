// Page layout as commands: the page number gallery, drop caps, page borders and colour, the Left and Right column
// presets, odd and even pages, optional and non-breaking hyphens, and hyphenation (options and by hand). Docxtor's own
// hyphenator is registered here too, as the first of the hyphenators other extensions can add to.
import { defineExtension, msg, t, type CommandOptions, type Extension } from "@docxtor/extensions";
import { ruleBreaks } from "../hyphenation/rules";
import { presetWidths, textWidth } from "../ui/columns";
import { dropCapState, openDropCap, setDropCap, type DropKind } from "../ui/drop-cap";
import { NON_BREAKING_HYPHEN, openHyphenationOptions, openManualHyphenation, SOFT_HYPHEN } from "../ui/hyphenation";
import { LAYOUT_ICONS } from "../ui/layout-icons";
import { openPageBorders } from "../ui/page-borders";
import { openPageFill } from "../ui/page-fill";
import type { Internals } from "./index";

type Where = "top" | "bottom";
type Align = "left" | "center" | "right";

const ALIGNS: [Align, string][] = [
  ["left", msg("Left")],
  ["center", msg("Center")],
  ["right", msg("Right")],
];

export function pageLayout({ fe, editor: e }: Internals): Extension {
  return defineExtension({
    id: "docxtor.pageLayout",
    name: msg("Page layout"),
    description: msg("Page numbers, drop caps, page borders and colour, column presets, odd and even pages and hyphenation."),
    activate(ctx) {
      for (const [id, svg] of Object.entries(LAYOUT_ICONS)) ctx.icons.register({ id, svg });
      const open = () => !!fe.engine;
      const editable = () => open() && !fe.readOnly;
      const inBody = () => editable() && !e.story;
      const cmd = (id: string, title: string, category: string, run: () => unknown, icon?: string, when = editable, extra: Partial<CommandOptions> = {}) =>
        ctx.commands.register(id, run, { title, category, when, icon, ...extra });
      const aiCmd = (id: string, title: string, category: string, run: () => unknown, icon?: string, when = editable, extra: Partial<CommandOptions> = {}) =>
        cmd(id, title, category, run, icon, when, { ai: true, ...extra });
      const item = (menu: string, group: string, command: string, title?: string) => ctx.menus.register({ menu, group, command, title });
      const sub = (menu: string, group: string, submenu: string, title: string, icon?: string) => ctx.menus.register({ menu, group, submenu, title, icon });

      // ---- Page numbers: the gallery
      // From inside a header or footer the number goes into that one (the first page's or the even pages'). From the
      // text it goes into the usual one and, when the document has different odd and even pages, into the even one too,
      // so every page is numbered. A different first page is left alone: a title page usually carries no number.
      const numberAt = (spec: { place: string; align?: Align; style?: string }) => () => {
        const inStory = e.story && (e.story.zone === "header" || e.story.zone === "footer") ? (e.story.variant ?? "default") : undefined;
        if (e.story) e.exitStory(false);
        const several = (e.pageInfo.sections?.length ?? 1) > 1;
        const section = several ? e.sectionAnchor(e.caretSection()) : undefined;
        const variants = inStory ? [inStory] : e.pageInfo.evenAndOddHeaders ? ["default", "even"] : ["default"];
        e.exec(
          variants.map((variant) => ({ op: "insert_page_number", ...spec, variant, ...(section !== undefined ? { section } : {}), page_word: t("Page"), of_word: t("of") })),
        );
        e.reloadPageSetup();
      };
      for (const where of ["top", "bottom"] as Where[]) {
        const area = where === "top" ? t("top of page") : t("bottom of page");
        for (const [align, label] of ALIGNS) {
          aiCmd(`insert.pageNumber.${where}.plain.${align}`, t("Page number, {where}, {align}", { where: area, align: t(label).toLowerCase() }), t("Insert"), numberAt({ place: where, align, style: "plain" }), where === "top" ? "pageNumberTop" : "pageNumberBottom");
          aiCmd(`insert.pageNumber.${where}.xofy.${align}`, t("Page X of Y, {where}, {align}", { where: area, align: t(label).toLowerCase() }), t("Insert"), numberAt({ place: where, align, style: "x_of_y" }), where === "top" ? "pageNumberTop" : "pageNumberBottom");
        }
      }
      aiCmd("insert.pageNumber.margin.left", t("Page number in the left margin"), t("Insert"), numberAt({ place: "margin_left" }), "pageNumberMargin");
      aiCmd("insert.pageNumber.margin.right", t("Page number in the right margin"), t("Insert"), numberAt({ place: "margin_right" }), "pageNumberMargin");
      aiCmd("insert.pageNumber.current", t("Page number at the caret"), t("Insert"), () => e.insertField("PAGE"), "pageNumberHere");
      aiCmd("insert.removePageNumbers", t("Remove page numbers"), t("Insert"), () => {
        if (e.story) e.exitStory(false);
        e.exec({ op: "remove_page_numbers" });
        e.reloadPageSetup();
      }, "pageNumberRemove");

      // The gallery sits in Insert itself: Insert, Page numbers, Top of page, Plain number is four deep, as deep as Docxtor's menus go.
      sub("insert", "3_page", "insert/page-numbers", t("Page numbers"), "pageNumber");
      for (const where of ["top", "bottom"] as Where[]) {
        const menu = `insert/pn-${where}`;
        sub("insert/page-numbers", "1", menu, where === "top" ? t("Top of page") : t("Bottom of page"), where === "top" ? "pageNumberTop" : "pageNumberBottom");
        for (const [align, label] of ALIGNS) item(menu, "1", `insert.pageNumber.${where}.plain.${align}`, t("Plain number, {align}", { align: t(label).toLowerCase() }));
        for (const [align, label] of ALIGNS) item(menu, "2", `insert.pageNumber.${where}.xofy.${align}`, t("Page X of Y, {align}", { align: t(label).toLowerCase() }));
      }
      sub("insert/page-numbers", "1", "insert/pn-margin", t("Page margins"), "pageNumberMargin");
      item("insert/pn-margin", "1", "insert.pageNumber.margin.left", t("Left margin"));
      item("insert/pn-margin", "1", "insert.pageNumber.margin.right", t("Right margin"));
      item("insert/page-numbers", "1", "insert.pageNumber.current", t("Current position"));
      item("insert/page-numbers", "2", "insert.removePageNumbers");

      // ---- Odd and even pages
      cmd(
        "insert.differentOddEven",
        t("Different odd and even pages"),
        t("Insert"),
        () => {
          if (e.story) e.exitStory(false);
          e.exec({ op: "set_layout_settings", even_and_odd_headers: !e.pageInfo.evenAndOddHeaders });
          e.reloadPageSetup();
        },
        "oddEven",
        editable,
        { toggled: () => open() && !!e.pageInfo.evenAndOddHeaders, ai: true },
      );
      item("insert/page", "1", "insert.differentOddEven");

      // ---- Drop caps
      const dropOn = () => inBody() && e.caretParagraph() !== undefined;
      for (const [kind, title] of [
        ["none", t("None")],
        ["drop", t("Dropped")],
        ["margin", t("In margin")],
      ] as [DropKind, string][]) {
        aiCmd(`insert.dropCap.${kind}`, kind === "none" ? t("No drop cap") : kind === "drop" ? t("Drop cap, dropped") : t("Drop cap, in margin"), t("Insert"), () => setDropCap(e, kind), "dropCap", dropOn, { toggled: () => dropOn() && dropCapState(e).kind === kind });
        item("insert/dropcap", "1", `insert.dropCap.${kind}`, title);
      }
      cmd("insert.dropCapOptions", t("Drop cap options…"), t("Insert"), () => openDropCap(e), "dropCap", dropOn);
      sub("insert", "1_objects", "insert/dropcap", t("Drop cap"), "dropCap");
      item("insert/dropcap", "2", "insert.dropCapOptions", t("Options…"));

      // ---- Hyphens
      aiCmd("insert.optionalHyphen", t("Optional hyphen"), t("Insert"), () => e.insertText(SOFT_HYPHEN), "optionalHyphen", editable, { keybinding: "Mod+-" });
      aiCmd("insert.nonBreakingHyphen", t("Non-breaking hyphen"), t("Insert"), () => e.insertText(NON_BREAKING_HYPHEN), "nonBreakingHyphen", editable, { keybinding: "Mod+Shift+-" });
      item("insert", "1_objects", "insert.optionalHyphen");
      item("insert", "1_objects", "insert.nonBreakingHyphen");
      cmd("tools.hyphenationOptions", t("Hyphenation options…"), t("Tools"), () => openHyphenationOptions(e), "hyphenation", editable);
      cmd("tools.hyphenateManually", t("Hyphenate manually…"), t("Tools"), () => openManualHyphenation(e, (word, lang) => fe.extensions.hyphenationPoints(word, lang)), "hyphenation", inBody);
      item("tools", "2_layout", "tools.hyphenateManually");
      item("tools", "2_layout", "tools.hyphenationOptions");
      ctx.hyphenation.register({ id: "docxtor.rules", label: msg("Docxtor's own rules"), languages: ["en", "es", "fr", "de", "it", "pt"], hyphenate: (word, lang) => ruleBreaks(word, lang) });

      // ---- Page borders and colour
      cmd("format.pageBorders", t("Page borders…"), t("Format"), () => openPageBorders(e), "pageBorders");
      cmd("format.pageColor", t("Page color…"), t("Format"), () => openPageFill(e), "pageColor");
      item("format", "3_page", "format.pageBorders");
      item("format", "3_page", "format.pageColor");

      // ---- Column presets: a narrow column and a wide one
      const gap = () => {
        const c = e.caretColumns();
        return c.space > 0 ? c.space : 720;
      };
      for (const kind of ["left", "right"] as const) {
        aiCmd(
          `format.columns${kind === "left" ? "Left" : "Right"}`,
          kind === "left" ? t("Left column narrow") : t("Right column narrow"),
          t("Format"),
          () => {
            const space = gap();
            e.setColumns(2, { space, widths: presetWidths(kind, textWidth(e), space), spaces: [space] });
          },
          kind === "left" ? "columnsLeft" : "columnsRight",
          inBody,
          {
            toggled: () => {
              if (!open()) return false;
              const w = e.caretColumns().widths;
              return w.length === 2 && (kind === "left" ? w[0] < w[1] : w[0] > w[1]);
            },
          },
        );
        item("format/columns", "1", `format.columns${kind === "left" ? "Left" : "Right"}`, kind === "left" ? t("Left") : t("Right"));
      }
    },
  });
}
