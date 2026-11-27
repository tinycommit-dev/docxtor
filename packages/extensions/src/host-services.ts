// Services the host asks its registries for: the @ menu, people, files, link previews, inspectors, comparers, fields, importers.
import type { ExtensionHost } from "./host.ts";
import type { AtMenuItem, AtMenuSection, FileRef, LinkPreview, Person } from "./smart.ts";
import type { DocumentComparer, FieldCall, FieldType, Importer, Inspector, InspectorReport } from "./types.ts";
import type { Op } from "@docxtor/core";
import { FIELD_CATEGORIES } from "./fields.ts";

/**
 * The lines of the @ menu for what was typed after the @, as sections. Providers are asked in order, a provider that
 * throws is reported and skipped, and lines with the same heading share a section. Empty sections are left out.
 */
export async function atMenuSections(host: ExtensionHost, query: string): Promise<AtMenuSection[]> {
  const entries = [...host.atMenuProviders.all()].sort((a, b) => (a.item.order ?? 100) - (b.item.order ?? 100));
  const found = await Promise.all(
    entries.map(async (entry): Promise<AtMenuItem[]> => {
      try {
        const items = await entry.item.items(query);
        return Array.isArray(items) ? items.filter((i) => i && i.id && i.label && typeof i.run === "function").map((i) => ({ ...i, section: i.section ?? entry.item.label, id: `${entry.item.id}:${i.id}` })) : [];
      } catch (err) {
        host.report(entry.extension, err);
        return [];
      }
    }),
  );
  const sections: AtMenuSection[] = [];
  for (const item of found.flat()) {
    let s = sections.find((x) => x.title === item.section);
    if (!s) sections.push((s = { id: `section:${sections.length}`, title: item.section!, items: [] }));
    s.items.push(item);
  }
  return sections.filter((s) => s.items.length);
}

/** People from every provider whose name or email fits `query`, the first mention of each person kept (people count as the same when their email, or without one their name, is). */
export async function findPeople(host: ExtensionHost, query: string): Promise<Person[]> {
  const out: Person[] = [];
  const seen = new Set<string>();
  for (const entry of host.peopleProviders.all()) {
    try {
      for (const p of (await entry.item.search(query)) ?? []) {
        if (!p?.name) continue;
        const key = (p.email || p.name).trim().toLowerCase();
        if (seen.has(key)) continue;
        seen.add(key);
        out.push(p);
      }
    } catch (err) {
      host.report(entry.extension, err);
    }
  }
  return out;
}

/** Hand a person typed in by hand to the providers that keep people. */
export async function rememberPerson(host: ExtensionHost, person: Person): Promise<void> {
  for (const entry of host.peopleProviders.all()) {
    try {
      await entry.item.remember?.(person);
    } catch (err) {
      host.report(entry.extension, err);
    }
  }
}

/** Files from every provider whose name fits `query`, each address once. */
export async function findFiles(host: ExtensionHost, query: string): Promise<FileRef[]> {
  const out: FileRef[] = [];
  const seen = new Set<string>();
  for (const entry of host.fileProviders.all()) {
    try {
      for (const f of (await entry.item.search(query)) ?? []) {
        if (!f?.title || !f.url || seen.has(f.url)) continue;
        seen.add(f.url);
        out.push(f);
      }
    } catch (err) {
      host.report(entry.extension, err);
    }
  }
  return out;
}

/** What the providers know about a link's page: the latest registered that has an answer wins. Null when none does. */
export async function linkPreview(host: ExtensionHost, url: string, signal?: AbortSignal): Promise<LinkPreview | null> {
  for (const entry of [...host.linkPreviewProviders.all()].reverse()) {
    try {
      if (entry.item.matches && !entry.item.matches(url)) continue;
      const found = await entry.item.preview(url, signal);
      if (found && (found.title || found.description)) return found;
    } catch (err) {
      if (signal?.aborted) return null;
      host.report(entry.extension, err);
    }
  }
  return null;
}

/** The inspectors, from the lowest `order` up (Docxtor's own run from 10 to 80), then in the order they were registered. */
export function inspectorList(host: ExtensionHost): Inspector[] {
  return host.inspectors
    .list()
    .map((item, i) => ({ item, i }))
    .sort((a, b) => (a.item.order ?? 100) - (b.item.order ?? 100) || a.i - b.i)
    .map((x) => x.item);
}

/** Run every inspector. One that fails is reported, and listed with its error and a count of 0. */
export async function inspect(host: ExtensionHost, signal: AbortSignal = new AbortController().signal): Promise<InspectorReport[]> {
  const entries = new Map(host.inspectors.all().map((e) => [e.item, e.extension] as const));
  const ctx = {
    document: host.services.document,
    signal,
    exec: () => {
      throw new Error("inspect() looks and does not change the document; use remove()");
    },
  };
  return Promise.all(
    inspectorList(host).map(async (i): Promise<InspectorReport> => {
      const base = { id: i.id, label: i.label, ...(i.description ? { description: i.description } : {}), removable: typeof i.remove === "function" };
      try {
        const found = await i.inspect(ctx);
        const count = Number.isFinite(found?.count) ? Math.max(0, Math.floor(found.count)) : 0;
        return { ...base, count, detail: Array.isArray(found?.detail) ? found.detail.map(String) : [] };
      } catch (err) {
        host.report(entries.get(i) ?? i.id, err);
        return { ...base, count: 0, detail: [], error: err instanceof Error ? err.message : String(err) };
      }
    }),
  );
}

/**
 * Take what some inspectors found out of the document. Every edit they ask for (`ctx.exec`) is applied together as
 * one undoable step, after all of them have run. Returns the ids of the inspectors that finished; one that fails is
 * reported and skipped, and its edits are not applied.
 */
export async function removeInspected(host: ExtensionHost, ids: string[], signal: AbortSignal = new AbortController().signal): Promise<string[]> {
  const entries = new Map(host.inspectors.all().map((e) => [e.item, e.extension] as const));
  const ops: Op[] = [];
  const done: string[] = [];
  for (const i of inspectorList(host)) {
    if (!ids.includes(i.id) || !i.remove) continue;
    const mine: Op[] = [];
    try {
      await i.remove({ document: host.services.document, signal, exec: (o) => mine.push(...(Array.isArray(o) ? o : [o])) });
      ops.push(...mine);
      done.push(i.id);
    } catch (err) {
      host.report(entries.get(i) ?? i.id, err);
    }
  }
  if (ops.length) host.services.document.exec(ops);
  return done;
}

/** The latest registered comparer that can do the job (`redline` and `combine` are optional parts of a comparer). */
export function comparerFor(host: ExtensionHost, job: "compare" | "redline" | "combine"): DocumentComparer | undefined {
  return [...host.comparers.list()].reverse().find((c) => typeof c[job] === "function");
}

/** The field type with this name (the latest registered wins), or undefined. */
export function fieldType(host: ExtensionHost, name: string): FieldType | undefined {
  const want = name.toUpperCase();
  return [...host.fieldTypes.list()].reverse().find((f) => f.name.toUpperCase() === want);
}

/** Every field type, one per name (the latest registered wins), in the order of the dialog's groups and then of registration. */
export function fieldTypeList(host: ExtensionHost): FieldType[] {
  const byName = new Map<string, FieldType>();
  for (const f of host.fieldTypes.list()) {
    byName.delete(f.name.toUpperCase());
    byName.set(f.name.toUpperCase(), f);
  }
  const order = (c: string) => {
    const i = FIELD_CATEGORIES.findIndex((x) => x.id === c);
    return i < 0 ? FIELD_CATEGORIES.length : i;
  };
  return [...byName.values()].map((f, i) => ({ f, i })).sort((a, b) => order(a.f.category) - order(b.f.category) || a.i - b.i).map((x) => x.f);
}

/**
 * Work out a field with an extension's evaluator. Resolves null when no extension evaluates that field name, when the
 * evaluator has nothing to say, or when it fails (the failure is reported and the result stays as it is).
 */
export async function evaluateField(host: ExtensionHost, call: FieldCall): Promise<string | null> {
  const entry = [...host.fieldTypes.all()].reverse().find((e) => e.item.name.toUpperCase() === call.code && typeof e.item.evaluate === "function");
  if (!entry) return null;
  try {
    const out = await entry.item.evaluate!(call);
    return typeof out === "string" ? out : null;
  } catch (err) {
    host.report(entry.extension, err);
    return null;
  }
}

/** Whether an extension evaluates fields with this name. */
export function evaluatesField(host: ExtensionHost, name: string): boolean {
  const want = name.toUpperCase();
  return host.fieldTypes.list().some((f) => f.name.toUpperCase() === want && typeof f.evaluate === "function");
}

/**
 * The importer for a file, by extension (the latest registered wins). With the file's `bytes`, importers that can
 * `detect` their kind of file are asked: among those that share the extension the first that says yes wins, and when
 * the name matches none (or the bytes say it isn't what the name claims) the first importer that detects the bytes.
 */
export function importerFor(host: ExtensionHost, name: string, bytes?: Uint8Array): Importer | undefined {
  const ext = name.slice(name.lastIndexOf(".") + 1).toLowerCase();
  const latest = [...host.importers.list()].reverse();
  const byName = latest.filter((i) => i.extensions.some((e) => e.toLowerCase().replace(/^\./, "") === ext));
  if (!bytes) return byName[0];
  const fits = (i: Importer) => {
    try {
      return !i.detect || i.detect(bytes);
    } catch {
      return false;
    }
  };
  return byName.find(fits) ?? latest.find((i) => i.detect && fits(i));
}

/** The importer whose `detect` recognises these bytes, whatever the file is called. */
export function importerForBytes(host: ExtensionHost, bytes: Uint8Array): Importer | undefined {
  return [...host.importers.list()].reverse().find((i) => {
    try {
      return !!i.detect?.(bytes);
    } catch {
      return false;
    }
  });
}
