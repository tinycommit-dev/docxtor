// What an extension is handed when it activates: its context, with every contribution point checked and tracked.
import type { ExtensionHost, Slot } from "./host.ts";
import { Registry } from "./host.ts";
import type { ContributionApi, Disposable, ExtensionContext, IconContribution } from "./types.ts";
import { t } from "./l10n.ts";

function checkIcon(i: IconContribution) {
  if (!i?.id || !i.svg?.trim().startsWith("<svg")) throw new Error("An icon needs an id and SVG markup");
}

export function context(host: ExtensionHost, s: Slot): ExtensionContext {
  const id = s.ext.id;
  const track = (d: Disposable): Disposable => {
    // Registering after deactivation (e.g. from a late promise) takes effect and is undone at once.
    if (!s.active) {
      d.dispose();
      return d;
    }
    s.subscriptions.push(d);
    return d;
  };
  const point = <T>(reg: Registry<T>, check?: (item: T) => void): ContributionApi<T> => ({
    register: (item) => {
      check?.(item);
      return track(reg.add(item, id));
    },
  });
  const needsId = (kind: string) => (item: { id: string }) => {
    if (!item?.id) throw new Error(`A ${kind} needs an id`);
  };
  const store = host.services.state;
  return {
    extension: { id, name: s.ext.name },
    document: host.services.document,
    ui: host.services.ui,
    ask: (cid: string, ...args: unknown[]) => host.ask(cid, ...args),
    state: {
      get: <T>(key: string, fallback: T): T => {
        const v = store ? store.get(id, key) : host.memory.get(`${id}\u0000${key}`);
        return v === undefined ? fallback : (v as T);
      },
      set: (key, value) => (store ? store.set(id, key, value) : host.memory.set(`${id}\u0000${key}`, value)),
    },
    commands: {
      register: (cid, run, opts) => track(host.registerCommand(cid, run, opts, id)),
      execute: (cid, ...args) => host.execute(cid, ...args),
      list: () => host.commands.list().filter((c) => host.enabled(c)).map((c) => ({ id: c.id, title: c.title, category: c.category, ai: !!c.ai })),
    },
    toolbar: point(host.toolbar, (t) => {
      needsId("toolbar item")(t);
      if (!t.icon && !t.label && !host.command(t.command)?.title && !t.title) throw new Error(`Toolbar item ${t.id} needs an icon or a label`);
    }),
    menus: point(host.menus),
    views: point(host.views, (v) => {
      needsId("view")(v);
      if (!v.title) throw new Error(`View ${v.id} needs a title`);
      if (v.enter !== undefined && typeof v.enter !== "function") throw new Error(`View ${v.id} needs enter() to be a function`);
    }),
    panels: point(host.panels, needsId("panel")),
    exporters: point(host.exporters, needsId("exporter")),
    importers: point(host.importers, needsId("importer")),
    templates: {
      ...point(host.templates, needsId("template")),
      stores: point(host.templateStores, (st) => {
        needsId("template store")(st);
        if (!st.label) throw new Error(`Template store ${st.id} needs a label`);
        if (typeof st.list !== "function" || typeof st.read !== "function" || typeof st.save !== "function" || typeof st.remove !== "function") throw new Error(`Template store ${st.id} needs list(), read(), save() and remove()`);
      }),
    },
    settings: point(host.settings, (d) => {
      needsId("setting")(d);
      if (!d.label) throw new Error(`Setting ${d.id} needs a label`);
      if (!["toggle", "choice", "number", "text", "font"].includes(d.kind)) throw new Error(`Setting ${d.id} has an unknown kind`);
      if (d.kind === "choice" && !d.choices?.length) throw new Error(`Setting ${d.id} is a choice and needs choices`);
      if (typeof d.get !== "function" || typeof d.set !== "function") throw new Error(`Setting ${d.id} needs get() and set()`);
    }),
    themes: point(host.themes, needsId("theme")),
    ai: { providers: point(host.aiProviders, needsId("AI provider")), tools: point(host.aiTools, (t) => { if (!t?.name) throw new Error("An AI tool needs a name"); }) },
    storage: point(host.storage, needsId("storage provider")),
    fonts: point(host.fonts, (f) => {
      if (!f?.family) throw new Error("A font needs a family name");
    }),
    icons: point(host.icons, checkIcon),
    iconPacks: point(host.iconPacks, (p) => {
      if (!p?.id || !p.label || !Array.isArray(p.icons)) throw new Error("An icon pack needs an id, a label and a list of icons");
      p.icons.forEach(checkIcon);
    }),
    spelling: {
      dictionaries: point(host.spellingDictionaries, (d) => {
        needsId("spelling dictionary")(d);
        if (typeof d.load !== "function") throw new Error("A spelling dictionary needs a load() function");
      }),
      languageDetectors: point(host.languageDetectors, (d) => {
        needsId("language detector")(d);
        if (typeof d.detect !== "function") throw new Error("A language detector needs a detect() function");
      }),
    },
    fields: point(host.fieldTypes, (f) => {
      if (!f?.name || !/^(=|[A-Za-z0-9]+)$/.test(f.name)) throw new Error("A field type needs a name made of letters and digits");
      if (!f.category) throw new Error(`Field type ${f.name} needs a category`);
      if (f.evaluate !== undefined && typeof f.evaluate !== "function") throw new Error(`Field type ${f.name} needs evaluate() to be a function`);
    }),
    grammar: point(host.grammarCheckers, (c) => {
      needsId("grammar checker")(c);
      if (typeof c.check !== "function") throw new Error("A grammar checker needs a check() function");
    }),
    readings: point(host.readingProviders, (r) => {
      needsId("reading provider")(r);
      if (typeof r.reading !== "function") throw new Error("A reading provider needs a reading() function");
    }),
    hyphenation: point(host.hyphenators, (hy) => {
      needsId("hyphenator")(hy);
      if (typeof hy.hyphenate !== "function") throw new Error("A hyphenator needs a hyphenate() function");
    }),
    thesaurus: point(host.thesauruses, (th) => {
      needsId("thesaurus")(th);
      if (typeof th.lookup !== "function") throw new Error("A thesaurus needs a lookup() function");
    }),
    inspectors: point(host.inspectors, (i) => {
      needsId("inspector")(i);
      if (!i.label) throw new Error(`Inspector ${i.id} needs a label`);
      if (typeof i.inspect !== "function") throw new Error(`Inspector ${i.id} needs an inspect() function`);
      if (i.remove !== undefined && typeof i.remove !== "function") throw new Error(`Inspector ${i.id} needs remove() to be a function`);
    }),
    comparers: point(host.comparers, (c) => {
      needsId("comparer")(c);
      if (typeof c.compare !== "function") throw new Error(`Comparer ${c.id} needs a compare() function`);
    }),
    tableDesigns: point(host.tableDesigns, (d) => {
      if (!d?.id || !d.label) throw new Error("A table design needs an id and a label");
      if (typeof d.xml !== "string" || !d.xml.includes("w:style")) throw new Error(`Table design ${d.id} needs its w:style as XML`);
    }),
    chartPalettes: point(host.chartPalettes, (p) => {
      if (!p?.id || !p.label) throw new Error("A chart palette needs an id and a label");
      if (!Array.isArray(p.colors) || p.colors.length === 0) throw new Error(`Chart palette ${p.id} needs a list of colors`);
      for (const c of p.colors) if (!/^([0-9a-f]{6}|scheme:[a-z0-9]+(:[a-z]+=\d+)*)$/i.test(c)) throw new Error(`Chart palette ${p.id} has a color that is neither RRGGBB nor scheme:name (${c})`);
    }),      contentControls: point(host.contentControls, (c) => {
      if (!c?.kind) throw new Error("A content control kind needs a kind");
      if (typeof c.render !== "function" && typeof c.edit !== "function") throw new Error(`Content control ${c.kind} needs render() or edit()`);
    }),
    atMenu: point(host.atMenuProviders, (p) => {
      needsId("@ menu provider")(p);
      if (!p.label) throw new Error(`@ menu provider ${p.id} needs a label`);
      if (typeof p.items !== "function") throw new Error(`@ menu provider ${p.id} needs an items() function`);
    }),
    people: point(host.peopleProviders, (p) => {
      needsId("people provider")(p);
      if (typeof p.search !== "function") throw new Error(`People provider ${p.id} needs a search() function`);
    }),
    files: point(host.fileProviders, (p) => {
      needsId("file provider")(p);
      if (typeof p.search !== "function") throw new Error(`File provider ${p.id} needs a search() function`);
    }),
    dropdowns: point(host.dropdownPresets, (d) => {
      if (!d?.id || !d.label) throw new Error("A drop-down needs an id and a label");
      if (!Array.isArray(d.options) || d.options.length === 0 || d.options.some((o) => !o?.label)) throw new Error(`Drop-down ${d.id} needs a list of options, each with a label`);
    }),
    buildingBlocks: point(host.docBuildingBlocks, (b) => {
      if (!b?.id || !b.label) throw new Error("A building block needs an id and a label");
      if (typeof b.markdown !== "string" && typeof b.markdown !== "function") throw new Error(`Building block ${b.id} needs its markdown`);
    }),
    cautions: point(host.cautions, (c) => {
      if (!c?.id || !c.title) throw new Error("A caution needs an id and a title");
      if (c.kind !== "ask" && c.kind !== "undo") throw new Error(`Caution ${c.id}: kind is "ask" or "undo"`);
    }),
    linkPreviews: point(host.linkPreviewProviders, (p) => {
      needsId("link preview provider")(p);
      if (typeof p.preview !== "function") throw new Error(`Link preview provider ${p.id} needs a preview() function`);
    }),
    documentThemes: point(host.documentThemes, (d) => {
      if (!d?.id || !d.label) throw new Error("A document theme needs an id and a label");
      if (!d.colors && !d.fonts) throw new Error(`Document theme ${d.id} needs colours, fonts or both`);
      for (const [slot, hex] of Object.entries(d.colors ?? {})) {
        if (!/^[0-9a-fA-F]{6}$/.test(String(hex ?? ""))) throw new Error(`Document theme ${d.id}: ${slot} is not a colour (six hex digits)`);
      }
      if (d.fonts && (!d.fonts.major?.trim() || !d.fonts.minor?.trim())) throw new Error(`Document theme ${d.id} needs a heading font and a body font`);
    }),
    styleSets: point(host.styleSets, (s) => {
      if (!s?.id || !s.label) throw new Error("A style set needs an id and a label");
      if (s.styles !== undefined && !s.styles.includes("w:style")) throw new Error(`Style set ${s.id} needs its w:style elements as XML`);
    }),
    clipboard: {
      converters: point(host.pasteConverters, (c) => {
        needsId("paste converter")(c);
        if (!c.mime) throw new Error(`Paste converter ${c.id} needs a mime type`);
        if (typeof c.convert !== "function") throw new Error(`Paste converter ${c.id} needs a convert() function`);
      }),
    },
    citations: {
      styles: point(host.citationStyles, (c) => {
        if (!c?.id || !c.label) throw new Error("A citation style needs an id and a label");
      }),
      sources: point(host.citationSources, (p) => {
        if (!p?.id || !p.label) throw new Error("A source provider needs an id and a label");
        if (!p.search && !p.resolve) throw new Error(`Source provider ${p.id} needs search() or resolve()`);
      }),
      libraries: point(host.sourceLibraries, (l) => {
        needsId("source library")(l);
        if (!l.label) throw new Error(`Source library ${l.id} needs a label`);
        if (typeof l.list !== "function" || typeof l.save !== "function") throw new Error(`Source library ${l.id} needs list() and save()`);
      }),
    },
    snippets: {
      stores: point(host.snippetStores, (st) => {
        needsId("snippet store")(st);
        if (!st.label) throw new Error(`Snippet store ${st.id} needs a label`);
        if (typeof st.list !== "function" || typeof st.save !== "function") throw new Error(`Snippet store ${st.id} needs list() and save()`);
      }),
    },
    pictures: {
      sources: point(host.pictureSources, (p) => {
        needsId("picture source")(p);
        if (!p.label) throw new Error(`Picture source ${p.id} needs a label`);
        if (typeof p.pick !== "function") throw new Error(`Picture source ${p.id} needs a pick() function`);
      }),
    },
    mailMerge: {
      dataSources: point(host.mailMergeSources, (p) => {
        needsId("mail merge data source")(p);
        if (!p.label) throw new Error(`Mail merge data source ${p.id} needs a label`);
        if (typeof p.pick !== "function") throw new Error(`Mail merge data source ${p.id} needs a pick() function`);
      }),
      mailSenders: point(host.mailSenders, (p) => {
        needsId("mail sender")(p);
        if (!p.label) throw new Error(`Mail sender ${p.id} needs a label`);
        if (typeof p.send !== "function") throw new Error(`Mail sender ${p.id} needs a send() function`);
        if (p.formats && !p.formats.length) throw new Error(`Mail sender ${p.id} lists no formats it can send`);
      }),
      labelProducts: point(host.labelProducts, (p) => {
        needsId("label product")(p);
        if (!p.vendor || !p.name) throw new Error(`Label product ${p.id} needs a vendor and a name`);
        const nums = [p.pageWidth, p.pageHeight, p.labelWidth, p.labelHeight, p.pitchH, p.pitchV];
        if (nums.some((n) => !(n > 0)) || [p.top, p.left].some((n) => !(n >= 0))) throw new Error(`Label product ${p.id} needs sizes greater than zero`);
        if (!Number.isInteger(p.across) || !Number.isInteger(p.down) || p.across < 1 || p.down < 1) throw new Error(`Label product ${p.id} needs a whole number of labels across and down`);
        if (p.pitchH < p.labelWidth || p.pitchV < p.labelHeight) throw new Error(`Label product ${p.id}: the pitch can't be smaller than the label`);
      }),
    },
    subscriptions: s.subscriptions,
  };
}
