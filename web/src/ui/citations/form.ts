// The form for one source: pick what kind of source it is and fill in the fields that kind uses. The same form
// adds a source, edits one, and shows what a DOI lookup found for checking before it is saved.
import { currentLocale, h, I, modal, svg, t, type Source, type SourceInput, type SourceName, type SourceType } from "@docxtor/editor";

type Field = "containerTitle" | "publisher" | "place" | "edition" | "volume" | "issue" | "pages" | "url" | "doi" | "genre";

interface TypeSpec {
  /** Name of the kind, as the type menu shows it. */
  label: () => string;
  /** What the title is called for this kind ("Journal article" has an article title). */
  title: () => string;
  /** The fields this kind uses, in order, with the name each goes by. */
  fields: [Field, () => string, string?][];
  /** How much of the date it takes. */
  date: "year" | "month" | "day";
  /** Websites say when they were read. */
  accessed?: boolean;
}

const SPECS: Record<SourceType, TypeSpec> = {
  book: {
    label: () => t("Book"),
    title: () => t("Title"),
    date: "year",
    fields: [
      ["edition", () => t("Edition"), "2nd"],
      ["volume", () => t("Volume")],
      ["publisher", () => t("Publisher")],
      ["place", () => t("Place of publication")],
      ["doi", () => t("DOI")],
      ["url", () => t("Web address")],
    ],
  },
  article: {
    label: () => t("Journal article"),
    title: () => t("Article title"),
    date: "month",
    fields: [
      ["containerTitle", () => t("Journal")],
      ["volume", () => t("Volume")],
      ["issue", () => t("Issue")],
      ["pages", () => t("Pages"), "45-67"],
      ["doi", () => t("DOI")],
      ["url", () => t("Web address")],
    ],
  },
  chapter: {
    label: () => t("Chapter"),
    title: () => t("Chapter title"),
    date: "year",
    fields: [
      ["containerTitle", () => t("Book title")],
      ["edition", () => t("Edition"), "2nd"],
      ["publisher", () => t("Publisher")],
      ["place", () => t("Place of publication")],
      ["pages", () => t("Pages"), "45-67"],
      ["doi", () => t("DOI")],
      ["url", () => t("Web address")],
    ],
  },
  conference: {
    label: () => t("Conference paper"),
    title: () => t("Paper title"),
    date: "month",
    fields: [
      ["containerTitle", () => t("Conference or proceedings")],
      ["place", () => t("Place of the conference")],
      ["publisher", () => t("Publisher")],
      ["pages", () => t("Pages"), "45-67"],
      ["doi", () => t("DOI")],
      ["url", () => t("Web address")],
    ],
  },
  report: {
    label: () => t("Report"),
    title: () => t("Report title"),
    date: "day",
    fields: [
      ["publisher", () => t("Organisation")],
      ["place", () => t("Place of publication")],
      ["genre", () => t("Kind of report"), "Technical report"],
      ["doi", () => t("DOI")],
      ["url", () => t("Web address")],
    ],
  },
  website: {
    label: () => t("Website"),
    title: () => t("Page title"),
    date: "day",
    accessed: true,
    fields: [
      ["containerTitle", () => t("Website name")],
      ["publisher", () => t("Publisher")],
      ["url", () => t("Web address")],
    ],
  },
  thesis: {
    label: () => t("Thesis"),
    title: () => t("Title"),
    date: "year",
    fields: [
      ["genre", () => t("Kind of thesis"), "Doctoral dissertation"],
      ["publisher", () => t("University")],
      ["place", () => t("Place")],
      ["doi", () => t("DOI")],
      ["url", () => t("Web address")],
    ],
  },
  misc: {
    label: () => t("Other"),
    title: () => t("Title"),
    date: "day",
    fields: [
      ["containerTitle", () => t("Published in")],
      ["genre", () => t("Kind of source")],
      ["publisher", () => t("Publisher")],
      ["place", () => t("Place of publication")],
      ["doi", () => t("DOI")],
      ["url", () => t("Web address")],
    ],
  },
};

const ORDER: SourceType[] = ["book", "article", "chapter", "conference", "report", "website", "thesis", "misc"];

/** The names of the kinds of source, for menus and messages elsewhere. */
export const typeLabel = (type: SourceType): string => (SPECS[type] ?? SPECS.misc).label();

interface Contributor {
  role: "author" | "editor";
  kind: "person" | "organisation";
  family: string;
  given: string;
  literal: string;
}

const blank = (role: Contributor["role"] = "author"): Contributor => ({ role, kind: "person", family: "", given: "", literal: "" });

const contributorOf = (role: Contributor["role"], n: SourceName): Contributor =>
  "literal" in n ? { role, kind: "organisation", family: "", given: "", literal: n.literal } : { role, kind: "person", family: n.family, given: n.given ?? "", literal: "" };

/** A month given as a number or as a name (English or in the reader's language), as its number from 1; "" when unknown. */
function monthNumber(month: string | undefined): string {
  const m = (month ?? "").trim();
  if (!m) return "";
  if (/^\d{1,2}$/.test(m) && +m >= 1 && +m <= 12) return String(+m);
  const names = (locale?: string) => Array.from({ length: 12 }, (_, i) => new Date(2000, i, 1).toLocaleDateString(locale, { month: "long" }).toLowerCase());
  for (const list of [names("en"), names(currentLocale())]) {
    const i = list.findIndex((n) => n === m.toLowerCase() || n.slice(0, 3) === m.toLowerCase().slice(0, 3));
    if (i >= 0) return String(i + 1);
  }
  return "";
}

let seq = 0;

export interface SourceFormOptions {
  /** The source to edit or to check (a DOI lookup's result); omitted for a blank form. */
  source?: Source;
  /** Heading and primary button. */
  title: string;
  ok: string;
  /** A sentence under the heading. */
  sub?: string;
  /**
   * Save the source. Resolves true when it went in; call `showError` with the engine's message when it didn't, and
   * the form stays open for the person to fix.
   */
  save: (source: SourceInput, showError: (message: string) => void) => boolean | Promise<boolean>;
  onClose?: () => void;
}

/** Open the source form. */
export function openSourceForm(opts: SourceFormOptions): void {
  const start = opts.source;
  const id = `src${++seq}`;
  let type: SourceType = start && SPECS[start.type] ? start.type : "book";
  const values: Partial<Record<Field | "title" | "year" | "month" | "day" | "accessed", string>> = {
    title: start?.title ?? "",
    year: start?.year ?? "",
    month: monthNumber(start?.month),
    day: start?.day ?? "",
    accessed: /^\d{4}-\d{2}-\d{2}$/.test(start?.accessed ?? "") ? start?.accessed : "",
  };
  for (const f of ["containerTitle", "publisher", "place", "edition", "volume", "issue", "pages", "url", "doi", "genre"] as Field[]) values[f] = start?.[f] ?? "";
  const people: Contributor[] = [...(start?.authors ?? []).map((n) => contributorOf("author", n)), ...(start?.editors ?? []).map((n) => contributorOf("editor", n))];
  if (!people.length) people.push(blank());
  let accessedTouched = false;

  const typeSelect = h("select", { id: `${id}-type`, "aria-label": t("Kind of source") }, ...ORDER.map((k) => h("option", { value: k }, SPECS[k].label())));
  typeSelect.value = type;
  const contributors = h("div", { class: "src-people" });
  const titleHost = h("div", {});
  const fields = h("div", { class: "src-grid" });
  const error = h("p", { class: "bm-error", role: "alert", hidden: true });
  const showError = (message: string) => {
    error.textContent = message;
    error.hidden = !message;
  };
  const ok = h("button", { class: "btn primary", type: "submit" }, opts.ok);
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const addPerson = h("button", { class: "btn sm", type: "button" }, t("Add a contributor"));
  const form = h(
    "form",
    { class: "dialog src-dialog", role: "dialog", "aria-modal": "true", "aria-label": opts.title, novalidate: true },
    h("div", { class: "dialog-head" }, h("h2", {}, opts.title), opts.sub ? h("p", { class: "sub" }, opts.sub) : null),
    h(
      "div",
      { class: "dialog-body" },
      h("div", { class: "field" }, h("label", { for: `${id}-type` }, t("Kind of source")), typeSelect),
      titleHost,
      h("div", { class: "src-section" }, h("h3", {}, t("Authors and editors")), contributors, addPerson),
      fields,
      error,
    ),
    h("div", { class: "dialog-actions" }, cancel, ok),
  );

  const text = (key: keyof typeof values, label: string, placeholder = "", wide = false, kind = "text") => {
    const input = h("input", { type: kind, id: `${id}-${key}`, value: values[key] ?? "", placeholder, autocomplete: "off", spellcheck: "false" });
    input.addEventListener("input", () => {
      values[key] = input.value;
      if (key === "accessed") accessedTouched = true;
      if (key === "title") showError("");
    });
    return h("div", { class: `field${wide ? " wide" : ""}` }, h("label", { for: input.id }, label), input);
  };

  const dates = (spec: TypeSpec) => {
    const months = Array.from({ length: 12 }, (_, i) => new Date(2000, i, 1).toLocaleDateString(currentLocale(), { month: "long" }));
    const year = text("year", t("Year"), "2024");
    const out: HTMLElement[] = [year];
    if (spec.date !== "year") {
      const month = h("select", { id: `${id}-month` }, h("option", { value: "" }, t("Month")), ...months.map((m, i) => h("option", { value: String(i + 1) }, m)));
      month.value = values.month ?? "";
      month.addEventListener("change", () => (values.month = month.value));
      out.push(h("div", { class: "field" }, h("label", { for: month.id }, t("Month")), month));
    }
    if (spec.date === "day") out.push(text("day", t("Day"), "", false, "text"));
    return out;
  };

  const paintFields = () => {
    const spec = SPECS[type];
    titleHost.replaceChildren(text("title", spec.title(), ""));
    const parts: HTMLElement[] = [...dates(spec)];
    for (const [key, label, placeholder] of spec.fields) parts.push(text(key, label(), placeholder ?? "", key === "url" || key === "containerTitle"));
    if (spec.accessed) parts.push(text("accessed", t("Date you read it"), "", false, "date"));
    fields.replaceChildren(...parts);
  };

  const paintPeople = () => {
    const rows = people.map((c, i) => {
      const role = h("select", { "aria-label": t("Role") }, h("option", { value: "author" }, t("Author")), h("option", { value: "editor" }, t("Editor")));
      role.value = c.role;
      role.addEventListener("change", () => (c.role = role.value as Contributor["role"]));
      const kind = h("select", { "aria-label": t("Person or organisation") }, h("option", { value: "person" }, t("Person")), h("option", { value: "organisation" }, t("Organisation")));
      kind.value = c.kind;
      kind.addEventListener("change", () => {
        c.kind = kind.value as Contributor["kind"];
        paintPeople();
        contributors.querySelectorAll<HTMLInputElement>(".src-person")[i]?.querySelector<HTMLInputElement>("input")?.focus();
      });
      const input = (key: "family" | "given" | "literal", label: string) => {
        const el = h("input", { type: "text", value: c[key], "aria-label": label, placeholder: label, autocomplete: "off", spellcheck: "false" });
        el.addEventListener("input", () => (c[key] = el.value));
        return el;
      };
      const names = c.kind === "person" ? [input("family", t("Family name")), input("given", t("Given names"))] : [input("literal", t("Organisation name"))];
      const remove = h("button", { class: "tb", type: "button", title: t("Remove this contributor"), "aria-label": t("Remove this contributor") }, svg(I.close));
      remove.addEventListener("click", () => {
        people.splice(i, 1);
        paintPeople();
      });
      return h("div", { class: `src-person ${c.kind}` }, role, kind, ...names, remove);
    });
    contributors.replaceChildren(...rows);
  };

  typeSelect.addEventListener("change", () => {
    type = typeSelect.value as SourceType;
    paintFields();
  });
  addPerson.addEventListener("click", () => {
    people.push(blank());
    paintPeople();
    contributors.querySelector<HTMLElement>(".src-person:last-of-type input")?.focus();
  });

  const collect = (): SourceInput => {
    const spec = SPECS[type];
    const name = (c: Contributor): SourceName | null =>
      c.kind === "organisation" ? (c.literal.trim() ? { literal: c.literal.trim() } : null) : c.family.trim() || c.given.trim() ? { family: (c.family.trim() || c.given.trim()), ...(c.family.trim() && c.given.trim() ? { given: c.given.trim() } : {}) } : null;
    const named = (role: Contributor["role"]) => people.filter((c) => c.role === role).map(name).filter((n): n is SourceName => !!n);
    const out: SourceInput = { type, title: (values.title ?? "").trim(), authors: named("author") };
    const editors = named("editor");
    if (editors.length) out.editors = editors;
    for (const [key] of spec.fields) {
      const v = (values[key] ?? "").trim();
      if (v) out[key] = v;
    }
    const year = (values.year ?? "").trim();
    if (year) out.year = year;
    if (spec.date !== "year" && values.month) out.month = values.month;
    if (spec.date === "day" && (values.day ?? "").trim()) out.day = (values.day ?? "").trim();
    if (spec.accessed) {
      const accessed = accessedTouched ? (values.accessed ?? "") : (values.accessed || start?.accessed || "");
      if (accessed) out.accessed = accessed;
    }
    if (start?.tag) out.tag = start.tag;
    if (start?.guid) out.guid = start.guid;
    return out;
  };

  const close = modal(form, opts.onClose);
  cancel.addEventListener("click", close);
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const source = collect();
    if (!source.title) {
      showError(t("Give the source a title."));
      form.querySelector<HTMLInputElement>(`#${id}-title`)?.focus();
      return;
    }
    showError("");
    ok.disabled = true;
    try {
      if (await opts.save(source, showError)) close();
    } finally {
      ok.disabled = false;
    }
  });
  paintFields();
  paintPeople();
  form.querySelector<HTMLInputElement>(`#${id}-title`)?.focus();
}
