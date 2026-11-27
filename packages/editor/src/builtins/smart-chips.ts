// Smart chips and the @ menu, as in Google Docs: person, date, file, place and drop-down chips (content controls Word
// shows as their text), the @ menu that inserts them and other things, Docs-style building blocks, and the card a link
// shows. Everything a chip or a line of the menu comes from is a contribution point (`ctx.atMenu`, `ctx.people`,
// `ctx.files`, `ctx.dropdowns`, `ctx.buildingBlocks`, `ctx.linkPreviews`); Docxtor's own providers below are registered
// through them like anyone else's, and work with no account and no network.
import { CHIP_TAG, atMenuMatches, defineExtension, msg, t, type AtMenuItem, type DropdownPreset, type Person } from "@docxtor/extensions";
import { insertDateChip, insertDropdownChip, insertTextChip } from "../editor/chips";
import { AtMenu } from "../ui/at-menu";
import { LinkCard } from "../ui/link-card";
import { dateCard, dropdownCard, fileCard, personCard, placeCard, type CardServices } from "../ui/chip-cards";
import { openAddPerson, openDropdownDialog } from "../ui/chip-dialogs";
import { CHIP_ICONS } from "../ui/chip-icons";
import { toast } from "../ui/dom";
import type { Internals } from "./index";
import type { Editor } from "../editor/editor";

const STATUS: DropdownPreset = {
  id: "docxtor.status",
  label: "Status",
  options: [
    { label: "Not started", color: "gray" },
    { label: "In progress", color: "blue" },
    { label: "Blocked", color: "red" },
    { label: "Done", color: "green" },
  ],
};
const PROJECT_STATUS: DropdownPreset = {
  id: "docxtor.projectStatus",
  label: "Project status",
  options: [
    { label: "Planning", color: "purple" },
    { label: "On track", color: "green" },
    { label: "At risk", color: "yellow" },
    { label: "Off track", color: "red" },
    { label: "Done", color: "blue" },
  ],
};

/** Names of Docxtor's presets and building blocks, for translating. */
const NAMES: Record<string, string> = {
  Status: msg("Status"),
  "Project status": msg("Project status"),
  "Not started": msg("Not started"),
  "In progress": msg("In progress"),
  Blocked: msg("Blocked"),
  Done: msg("Done"),
  Planning: msg("Planning"),
  "On track": msg("On track"),
  "At risk": msg("At risk"),
  "Off track": msg("Off track"),
};
const tr = (s: string) => (NAMES[s] ? t(NAMES[s]) : s);

/** Docs-style building blocks: Markdown with chip tokens (see `DocBuildingBlock`). */
function blocks() {
  const table = (head: string[], rows: string[][]) => [`| ${head.join(" | ")} |`, `| ${head.map(() => "---").join(" | ")} |`, ...rows.map((r) => `| ${r.join(" | ")} |`)].join("\n");
  const status = "{{status:Not started}}";
  return [
    {
      id: "docxtor.meetingNotes",
      label: t("Meeting notes"),
      description: t("The date, who came, notes and action items with their owners."),
      markdown: () => [`## {{date}} | ${t("Meeting title")}`, `**${t("Attendees")}:** {{person}}`, `### ${t("Notes")}`, `- ${t("Add notes here")}`, `### ${t("Action items")}`, `- [ ] {{person}} ${t("What to do")}`].join("\n\n"),
    },
    {
      id: "docxtor.emailDraft",
      label: t("Email draft"),
      description: t("To, Cc and Subject, then the message."),
      markdown: () => [table([t("To"), "{{person}}"], [[t("Cc"), "{{person}}"], [t("Subject"), ""]]), t("Write your message here.")].join("\n\n"),
    },
    {
      id: "docxtor.productRoadmap",
      label: t("Product roadmap"),
      description: t("Milestones with an owner, a date and a status."),
      markdown: () => table([t("Milestone"), t("Owner"), t("Date"), t("Status")], [[t("First milestone"), "{{person}}", "{{date}}", status], [t("Second milestone"), "{{person}}", "{{date}}", status]]),
    },
    {
      id: "docxtor.projectAssets",
      label: t("Project assets"),
      description: t("Files with their owner and status."),
      markdown: () => table([t("Asset"), t("Owner"), t("Status"), t("Notes")], [["{{chip:file:}}", "{{person}}", status, ""]]),
    },
    {
      id: "docxtor.contentTracker",
      label: t("Content tracker"),
      description: t("Pieces of writing with a writer, a due date and a status."),
      markdown: () => table([t("Title"), t("Writer"), t("Due"), t("Status")], [["", "{{person}}", "{{date}}", status]]),
    },
    {
      id: "docxtor.reviewTracker",
      label: t("Review tracker"),
      description: t("Who reviews what, and where each review is."),
      markdown: () => table([t("Reviewer"), t("Status"), t("Notes")], [["{{person}}", status, ""], ["{{person}}", status, ""]]),
    },
    {
      id: "docxtor.decisionLog",
      label: t("Decision log"),
      description: t("Decisions with the date, who made them and why."),
      markdown: () => table([t("Decision"), t("Date"), t("Owner"), t("Why")], [["", "{{date}}", "{{person}}", ""]]),
    },
    {
      id: "docxtor.launchTracker",
      label: t("Launch content tracker"),
      description: t("What goes out with a launch, by channel, owner and status."),
      markdown: () => table([t("Item"), t("Channel"), t("Owner"), t("Date"), t("Status")], [["", "", "{{person}}", "{{date}}", "{{project-status:Planning}}"]]),
    },
  ];
}

const isoToday = (days = 0) => {
  const d = new Date();
  d.setDate(d.getDate() + days);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

export function smartChips(x: Internals) {
  return defineExtension({
    id: "docxtor.smartChips",
    name: msg("Smart chips and the @ menu"),
    description: msg("People, dates, files and drop-downs as chips, the @ menu that inserts them, building blocks like meeting notes, and a card for links."),
    activate(ctx) {
      const { fe, editor: e } = x;
      for (const [id, svg] of Object.entries(CHIP_ICONS)) ctx.icons.register({ id, svg });
      const editable = () => !!fe.engine && !fe.readOnly && !e.story;
      const lang = () => e.pageInfo?.lang || navigator.language || "en-US";
      const dateFormat = () => ({ format: ctx.state.get<string>("dateFormat", "MMM d, yyyy"), lid: lang() });

      // -- people ----------------------------------------------------------------------------
      const added = () => ctx.state.get<Person[]>("people", []);
      ctx.people.register({
        id: "docxtor.people.local",
        label: t("This document and you"),
        search: (q) => {
          const out: Person[] = [];
          const me = fe.fieldOptions.user?.name?.trim();
          if (me) out.push({ name: me, detail: t("You") });
          const props = fe.engine?.docProps();
          if (props?.author) out.push({ name: props.author, detail: t("Author of this document") });
          if (props?.lastModifiedBy) out.push({ name: props.lastModifiedBy, detail: t("Last changed this document") });
          // People already in chips here.
          e.root.querySelectorAll<HTMLElement>('[data-chip="person"]').forEach((el) => {
            const name = el.textContent?.trim();
            if (name) out.push({ name, email: (el.closest("[data-href]") as HTMLElement | null)?.dataset.href?.replace(/^mailto:/, "") });
          });
          out.push(...added());
          return out.filter((p) => atMenuMatches(q, p.name, p.email ? [p.email] : []));
        },
        remember: (p) => ctx.state.set("people", [...added().filter((x) => (x.email || x.name) !== (p.email || p.name)), p]),
      });

      const personChip = (p: Person) => insertTextChip(e, { kind: "person", text: p.name, url: p.email ? `mailto:${p.email}` : undefined, alias: t("Person") });
      const addPerson = () =>
        openAddPerson({
          onAdd: (p) => {
            void fe.extensions.rememberPerson(p);
            personChip(p);
          },
          onClose: () => e.focus(),
        });
      const dateChip = (iso: string) => insertDateChip(e, iso, { ...dateFormat(), format: dateFormat().format, alias: t("Date") });

      // -- drop-downs --------------------------------------------------------------------------
      ctx.dropdowns.register(STATUS);
      ctx.dropdowns.register(PROJECT_STATUS);
      const dropdownChip = (d: DropdownPreset, selected?: string) => {
        const options = d.options.map((o) => ({ ...o, label: tr(o.label) }));
        // The first choice, unless one is named.
        const i = Math.max(0, options.findIndex((o) => o.label === tr(selected ?? "")));
        return insertDropdownChip(e, tr(d.label), options, i);
      };
      const newDropdown = () =>
        openDropdownDialog({
          onSave: (title, options) => insertDropdownChip(e, title, options, 0),
          onClose: () => e.focus(),
        });

      // -- building blocks ---------------------------------------------------------------------
      for (const b of blocks()) ctx.buildingBlocks.register({ ...b, category: t("Building blocks"), icon: "buildingBlocks" });
      const insertBlock = (md: string) => {
        if (!editable()) return;
        const before = new Set(Array.from(e.root.querySelectorAll<HTMLElement>("[data-p]")).map((el) => Number(el.dataset.p)));
        e.insertMarkdownAtCaret(md);
        const fresh = Array.from(e.root.querySelectorAll<HTMLElement>("[data-p]"))
          .map((el) => Number(el.dataset.p))
          .filter((id) => !before.has(id));
        fillTokens(e, fresh, { person: (at) => insertTextChip(e, { kind: "person", text: t("Person"), alias: t("Person") }, { at, merge: true }), date: (at) => insertDateChip(e, isoToday(), { ...dateFormat(), at, merge: true, alias: t("Date") }), dropdown: (at, id, option) => {
          const d = fe.extensions.dropdownPresets.list().find((p) => p.id === id);
          if (!d) return null;
          const options = d.options.map((o) => ({ ...o, label: tr(o.label) }));
          return insertDropdownChip(e, tr(d.label), options, Math.max(0, options.findIndex((o) => o.label === tr(option))), { at, merge: true });
        }, chip: (at, kind, text) => insertTextChip(e, { kind, text: text || t("File"), alias: kind }, { at, merge: true }) });
      };

      // -- the @ menu ------------------------------------------------------------------------------
      const line = (id: string, label: string, run: () => unknown, o: Partial<AtMenuItem> = {}): AtMenuItem => ({ id, label, run: () => void run(), ...o });
      ctx.atMenu.register({
        id: "docxtor.at.people",
        label: t("People"),
        order: 10,
        items: async (q) => {
          const people = (await fe.extensions.findPeople(q)).slice(0, 5);
          const out = people.map((p, i) => line(`p${i}`, p.name, () => personChip(p), { detail: p.email ?? p.detail, icon: "chipPerson" }));
          if (atMenuMatches(q, t("Add a person"), [t("People"), "person"])) out.push(line("add", t("Add a person…"), addPerson, { icon: "chipPerson" }));
          return out;
        },
      });
      ctx.atMenu.register({
        id: "docxtor.at.dates",
        label: t("Dates"),
        order: 20,
        items: (q) =>
          [
            line("today", t("Today"), () => dateChip(isoToday()), { icon: "dateTime", keywords: [t("Date"), "date"] }),
            line("tomorrow", t("Tomorrow"), () => dateChip(isoToday(1)), { icon: "dateTime", keywords: [t("Date"), "date"] }),
            line("yesterday", t("Yesterday"), () => dateChip(isoToday(-1)), { icon: "dateTime", keywords: [t("Date"), "date"] }),
            line("pick", t("Date"), () => dateChip(isoToday()), { icon: "dateTime", detail: t("Pick a day in the chip"), keywords: ["date", t("Calendar")] }),
          ].filter((i) => atMenuMatches(q, i.label, i.keywords)),
      });
      ctx.atMenu.register({
        id: "docxtor.at.files",
        label: t("Files"),
        order: 30,
        items: async (q) => (await fe.extensions.findFiles(q)).slice(0, 5).map((f, i) => line(`f${i}`, f.title, () => insertTextChip(e, { kind: "file", text: f.title, url: f.url, alias: t("File") }), { detail: f.detail, icon: "chipFile" })),
      });
      ctx.atMenu.register({
        id: "docxtor.at.dropdowns",
        label: t("Drop-downs"),
        order: 40,
        items: (q) => [
          ...fe.extensions.dropdownPresets
            .list()
            .filter((d) => atMenuMatches(q, tr(d.label), [t("Drop-down"), "dropdown"]))
            .map((d) => line(d.id, tr(d.label), () => dropdownChip(d), { icon: "chipDropdown", detail: d.options.map((o) => tr(o.label)).join(", ") })),
          ...(atMenuMatches(q, t("New drop-down"), [t("Drop-down"), "dropdown"]) ? [line("new", t("New drop-down…"), newDropdown, { icon: "chipDropdown" })] : []),
        ],
      });
      ctx.atMenu.register({
        id: "docxtor.at.blocks",
        label: t("Building blocks"),
        order: 50,
        items: (q) =>
          fe.extensions.docBuildingBlocks
            .list()
            .filter((b) => atMenuMatches(q, b.label, [t("Building blocks")]))
            .map((b) => line(b.id, b.label, () => insertBlock(typeof b.markdown === "function" ? b.markdown() : b.markdown), { icon: b.icon ?? "buildingBlocks", detail: b.description })),
      });
      const inserts: [string, string, string][] = [
        ["insert.table", t("Table"), "table"],
        ["format.heading1", t("Heading {n}", { n: 1 }), "heading1"],
        ["format.heading2", t("Heading {n}", { n: 2 }), "heading2"],
        ["format.heading3", t("Heading {n}", { n: 3 }), "heading3"],
        ["format.bullets", t("Bulleted list"), "bulletList"],
        ["format.checklist", t("Checklist"), "checklist"],
        ["insert.horizontalLine", t("Divider"), "horizontalLine"],
        ["insert.pageBreak", t("Page break"), "pageBreak"],
        ["insert.image", t("Image"), "image"],
        ["insert.equation", t("Equation"), "equation"],
        ["insert.toc", t("Table of contents"), "toc"],
        ["insert.pageNumbers", t("Page number"), "hash"],
      ];
      ctx.atMenu.register({
        id: "docxtor.at.insert",
        label: t("Insert"),
        order: 90,
        items: (q) => inserts.filter(([, label]) => atMenuMatches(q, label)).map(([cmd, label, icon]) => line(cmd, label, () => ctx.commands.execute(cmd), { icon })),
      });

      const menu = new AtMenu(e, fe.extensions);
      // The card a link shows when the caret is in it (not while the @ menu is open).
      const linkCard = new LinkCard(e, fe.extensions, () => void ctx.commands.execute("insert.link"));
      ctx.subscriptions.push(
        { dispose: fe.on("change", () => menu.check(true)) },
        { dispose: fe.on("selection", () => (menu.check(false), menu.open ? linkCard.hide() : linkCard.sync())) },
        { dispose: () => (menu.dismiss(), linkCard.hide()) },
      );

      // -- the chips on the page: their cards ---------------------------------------------------------
      const services: CardServices = {
        copy: (text, what) => {
          void navigator.clipboard?.writeText(text).then(
            () => toast(what),
            () => toast(t("Couldn't copy to the clipboard")),
          );
        },
        focus: () => e.focus(),
        // The editor's one way to open an address: the host opens its own (a file chip's document), the rest are checked.
        open: (url) => void e.openAddress(url),
        dateFormat,
        rememberDateFormat: (f) => ctx.state.set("dateFormat", f),
      };
      ctx.contentControls.register({ kind: `${CHIP_TAG}person`, label: t("Person"), edit: (api) => personCard(api, services) });
      ctx.contentControls.register({ kind: `${CHIP_TAG}file`, label: t("File"), edit: (api) => fileCard(api, services) });
      ctx.contentControls.register({ kind: `${CHIP_TAG}place`, label: t("Place"), edit: (api) => placeCard(api, services) });
      ctx.contentControls.register({ kind: `${CHIP_TAG}date`, label: t("Date"), edit: (api) => dateCard(api, services) });
      ctx.contentControls.register({ kind: `${CHIP_TAG}dropdown`, label: t("Drop-down"), edit: (api) => dropdownCard(api, services) });

      // -- commands and the menu ----------------------------------------------------------------------
      const cat = t("Insert");
      const cmd = (id: string, title: string, run: () => unknown, icon: string) => ctx.commands.register(id, () => void run(), { title, category: cat, when: editable, icon });
      cmd("insert.chip.person", t("Person…"), addPerson, "chipPerson");
      cmd("insert.chip.date", t("Date chip"), () => dateChip(isoToday()), "dateTime");
      cmd("insert.chip.status", t("Status"), () => dropdownChip(STATUS), "chipDropdown");
      cmd("insert.chip.projectStatus", t("Project status"), () => dropdownChip(PROJECT_STATUS), "chipDropdown");
      cmd("insert.chip.dropdown", t("New drop-down…"), newDropdown, "chipDropdown");
      cmd("insert.atMenu", t("@ menu"), () => e.exec({ op: "insert_text", at: e.range()![1], text: "@" }), "atSign");
      ctx.menus.register({ menu: "insert", group: "1_objects", submenu: "insert/chips", title: t("Smart chips"), icon: "smartChips" });
      for (const c of ["person", "date", "status", "projectStatus", "dropdown"]) ctx.menus.register({ menu: "insert/chips", group: "1", command: `insert.chip.${c}` });
      ctx.menus.register({ menu: "insert", group: "1_objects", submenu: "insert/docBlocks", title: t("Building blocks"), icon: "buildingBlocks" });
      const shown = new Map<string, { dispose(): void }[]>();
      const syncBlocks = () => {
        for (const [, ds] of shown) ds.forEach((d) => d.dispose());
        shown.clear();
        for (const b of fe.extensions.docBuildingBlocks.list()) {
          const id = `insert.docBlock.${b.id}`;
          shown.set(b.id, [
            ctx.commands.register(id, () => insertBlock(typeof b.markdown === "function" ? b.markdown() : b.markdown), { title: b.label, category: t("Building blocks"), when: editable, icon: b.icon ?? "buildingBlocks" }),
            ctx.menus.register({ menu: "insert/docBlocks", group: b.category ?? "1", command: id }),
          ]);
        }
      };
      syncBlocks();
      ctx.subscriptions.push(fe.extensions.docBuildingBlocks.onChange(syncBlocks), { dispose: () => shown.forEach((ds) => ds.forEach((d) => d.dispose())) });
    },
  });
}

/** Turn the chip tokens of a building block (`{{date}}`, `{{person}}`, `{{status:Done}}`...) into chips, last first. */
function fillTokens(
  e: Editor,
  ids: number[],
  make: {
    person(at: { p: number; o: number }): unknown;
    date(at: { p: number; o: number }): unknown;
    dropdown(at: { p: number; o: number }, id: string, option: string): unknown;
    chip(at: { p: number; o: number }, kind: string, text: string): unknown;
  },
) {
  for (const p of ids) {
    const view = e.paragraphView(p);
    if (!view || !e.engine) continue;
    const text = e.engine.textRange({ p, o: 0 }, { p, o: view.len });
    const found = [...text.matchAll(/\{\{([a-z-]+)(?::([^}]*))?\}\}/g)].reverse();
    for (const m of found) {
      const at = { p, o: m.index! };
      e.exec({ op: "delete", from: at, to: { p, o: m.index! + m[0].length } }, { select: at, merge: true });
      const [, kind, arg = ""] = m;
      if (kind === "person") make.person(at);
      else if (kind === "date") make.date(at);
      else if (kind === "status") make.dropdown(at, "docxtor.status", arg);
      else if (kind === "project-status") make.dropdown(at, "docxtor.projectStatus", arg);
      else if (kind === "dropdown") {
        const [id, option = ""] = arg.split(":");
        make.dropdown(at, id, option);
      } else if (kind === "chip") {
        const [k, rest = ""] = arg.split(":");
        make.chip(at, k, rest);
      }
    }
  }
}
