// Tools > Citations: the document's sources in the navigator. Add a source by hand or from any source provider
// (a DOI, BibTeX, Zotero, ...); cite one at the caret; edit or delete one; choose the citation style; insert the
// bibliography. The styles and the providers are contributions (`ctx.citations.styles`, `ctx.citations.sources`),
// so the panel lists whatever extensions add.
import { coalesce, h, I, popover, promptDialog, sourceDetail, sourceTitle, svg, t, tn, toast, type Source, type SourceName, type SourceProvider } from "@docxtor/editor";
import type { CitationsHost } from "../../citations-host";
import { openProviderDialog } from "./add";
import { openLibraryDialog } from "./library";
import { openSourceForm } from "./form";

/** Prefix of the picker lines that run a command instead of choosing a style. */
const ACTION = "action:";

const nameText = (n: SourceName) => ("literal" in n ? n.literal : `${n.given ?? ""} ${n.family}`);

/** Everything a search looks through for one source. */
const haystack = (s: Source) => [s.title, s.containerTitle, s.year, s.publisher, ...s.authors.map(nameText), ...(s.editors ?? []).map(nameText)].filter(Boolean).join(" ").toLowerCase();

export class CitationsPanel {
  readonly el: HTMLElement;
  private style: HTMLSelectElement;
  private adding: HTMLElement;
  private search: HTMLInputElement;
  private heading: HTMLElement;
  private list: HTMLElement;
  private bibliography: HTMLButtonElement;
  private exportBibtex: HTMLButtonElement;
  private shared!: HTMLButtonElement;
  private timer = 0;
  private active = 0;
  private shown: Source[] = [];

  constructor(
    private app: CitationsHost,
    private run: (command: string, ...args: unknown[]) => Promise<unknown>,
  ) {
    this.style = h("select", { class: "src-select", id: "src-style", "aria-label": t("Citation style") });
    this.style.addEventListener("change", () => void this.chooseStyle(this.style.value));

    this.adding = h("div", { class: "src-block" });
    const extensions = this.app.fe.extensions;
    // Once for a run of changes: every command an extension registers as it starts is a change, and each drew the whole list of styles.
    const styles = coalesce(() => this.drawStyles());
    extensions.citationStyles.onChange(styles);
    extensions.citationSources.onChange(() => this.drawProviders());
    extensions.commands.onChange(styles);
    this.drawProviders();

    this.heading = h("h3", {}, t("Sources"));
    this.search = h("input", { type: "search", class: "src-search", "aria-label": t("Search sources"), placeholder: t("Search sources"), autocomplete: "off", spellcheck: "false" });
    this.search.addEventListener("input", () => {
      this.active = 0;
      this.update();
    });
    this.search.addEventListener("keydown", (e) => this.onSearchKey(e));
    this.search.addEventListener("focus", () => this.list.classList.add("navigating"));
    this.search.addEventListener("blur", () => this.list.classList.remove("navigating"));
    this.list = h("ul", { class: "src-list" });

    this.bibliography = h("button", { class: "btn primary", type: "button" }, t("Insert bibliography"));
    this.bibliography.addEventListener("click", () => void this.run("insert.bibliography"));
    this.exportBibtex = h("button", { class: "btn", type: "button" }, t("Export BibTeX"));
    this.exportBibtex.addEventListener("click", () => this.export());
    this.shared = h("button", { class: "btn", type: "button" }, t("Shared list…"));
    this.shared.addEventListener("click", () => void openLibraryDialog(this.app, () => this.update()));

    this.el = h(
      "div",
      { class: "cite-panel" },
      h("div", { class: "panel-title" }, h("span", {}, t("Citations"))),
      h("div", { class: "src-block" }, h("label", { for: "src-style" }, t("Citation style")), this.style),
      this.adding,
      h("div", { class: "src-block" }, this.heading, this.search, this.list),
      h("div", { class: "src-foot" }, this.bibliography, this.exportBibtex, this.shared),
    );
    this.drawStyles();
    this.update();
  }

  private get visible(): boolean {
    return !!this.app.sidebar?.open && this.app.sidebar.tab === "citations";
  }

  /** Draw again soon, if the panel is showing (called after edits). */
  schedule() {
    window.clearTimeout(this.timer);
    if (!this.visible) return;
    this.timer = window.setTimeout(() => this.update(), 250);
  }

  /** Put the cursor in the search box, ready to type or to press Enter on the first source. */
  focusSearch() {
    this.search.value = "";
    this.active = 0;
    this.update();
    this.search.focus();
  }

  /** Draw the panel from the document. */
  update() {
    window.clearTimeout(this.timer);
    const engine = this.app.fe.engine;
    if (!engine) {
      this.shown = [];
      this.list.replaceChildren(h("li", { class: "empty-note" }, t("Open a document to work with its sources.")));
      return;
    }
    let sources: Source[] = [];
    let counts = new Map<string, number>();
    try {
      sources = engine.sources();
      this.drawStyles();
      for (const c of engine.citations()) for (const tag of new Set(c.tags)) counts.set(tag, (counts.get(tag) ?? 0) + 1);
    } catch {
      sources = [];
      counts = new Map();
    }
    this.heading.textContent = sources.length ? tn(sources.length, "{n} source", "{n} sources") : t("Sources");
    const words = this.search.value.toLowerCase().split(/\s+/).filter(Boolean);
    this.shown = sources.filter((s) => words.every((w) => haystack(s).includes(w)));
    this.active = Math.min(this.active, Math.max(0, this.shown.length - 1));
    if (!sources.length) this.list.replaceChildren(h("li", { class: "empty-note" }, t("No sources yet. Add one above, then cite it in your text.")));
    else if (!this.shown.length) this.list.replaceChildren(h("li", { class: "empty-note" }, t("No source matches “{query}”.", { query: this.search.value.trim() })));
    else this.list.replaceChildren(...this.shown.map((s, i) => this.row(s, i, counts.get(s.tag) ?? 0)));
    this.bibliography.textContent = this.app.editor.bibliographyElement() ? t("Go to bibliography") : t("Insert bibliography");
    this.exportBibtex.disabled = !sources.length;
  }

  private row(s: Source, i: number, cited: number): HTMLElement {
    const title = sourceTitle(s);
    const cite = h("button", { class: "btn sm primary", type: "button", "aria-label": t("Cite {title}", { title }) }, t("Cite"));
    cite.addEventListener("click", () => this.cite(s.tag));
    const edit = h("button", { class: "btn sm", type: "button", "aria-label": t("Edit {title}", { title }) }, t("Edit"));
    edit.addEventListener("click", () => this.edit(s.tag));
    const more = h("button", { class: "btn sm src-more", type: "button", "aria-label": t("More actions for {title}", { title }), "aria-haspopup": "menu" }, svg(I.more));
    more.addEventListener("click", () => this.more(s, more));
    const detail = sourceDetail(s);
    return h(
      "li",
      { class: `src-item${i === this.active ? " on" : ""}`, "data-tag": s.tag },
      h("div", { class: "src-title" }, title),
      detail ? h("div", { class: "src-detail" }, detail) : null,
      cited ? h("div", { class: "src-detail" }, tn(cited, "Cited {n} time", "Cited {n} times")) : null,
      h("div", { class: "src-actions" }, cite, edit, more),
    );
  }

  private more(s: Source, anchor: HTMLElement) {
    const item = (label: string, go: () => void) => {
      const b = h("button", { class: "menu-item", type: "button", role: "menuitem" }, label);
      b.addEventListener("click", () => {
        close();
        go();
      });
      return b;
    };
    const close = popover(anchor, h("div", { class: "src-menu" }, item(t("Cite with page…"), () => void this.citeWithPage(s)), item(t("Delete source"), () => void this.delete(s))), { align: "right" });
  }

  private onSearchKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (!this.shown.length) return;
      e.preventDefault();
      this.active = (this.active + (e.key === "ArrowDown" ? 1 : -1) + this.shown.length) % this.shown.length;
      this.list.querySelectorAll<HTMLElement>(".src-item").forEach((li, i) => {
        li.classList.toggle("on", i === this.active);
        if (i === this.active) li.scrollIntoView({ block: "nearest" });
      });
    } else if (e.key === "Enter") {
      e.preventDefault();
      const s = this.shown[this.active];
      if (s) this.cite(s.tag);
    }
  }

  /** Cite a source at the caret in the text. */
  cite(tag: string, page?: string) {
    if (this.app.fe.readOnly) return toast(t("Switch to editing to add a citation."));
    this.app.editor.insertCitation([tag], page);
  }

  private async citeWithPage(s: Source) {
    const page = await promptDialog({ title: t("Cite with page"), label: t("Page, for example 23 or 23-25"), ok: t("Cite") });
    if (page) this.cite(s.tag, page);
  }

  /** The form to add a source by hand. */
  addByHand() {
    this.openForm({ title: t("Add a source"), ok: t("Add source") });
  }

  /** The dialog of a source provider. A single source it finds can open in the form to check first. */
  private openProvider(p: SourceProvider) {
    const ed = this.app.editor;
    openProviderDialog(p, {
      have: () => ed.engine?.sources() ?? [],
      add: (sources) => {
        const done = ed.exec(
          sources.map((source) => ({ op: "add_source", source }) as const),
          { onError: (m) => toast(m) },
        );
        if (done) toast(tn(sources.length, "Added {n} source", "Added {n} sources"));
        return !!done;
      },
      review: (source) => this.openForm({ source, title: t("Check the source"), sub: p.reviewNote ?? t("Fix anything that looks wrong, then add it."), ok: t("Add source") }),
      onClose: () => ed.focus(),
    });
  }

  /** The buttons under "Add a source": the form, and one for each source provider that is on. */
  private drawProviders() {
    const button = (label: string, run: () => void) => {
      const b = h("button", { class: "btn src-add", type: "button" }, label);
      b.addEventListener("click", run);
      return b;
    };
    const providers = this.app.fe.extensions.citationSources.list();
    const notes = [...new Set(providers.map((p) => p.note).filter((n): n is string => !!n))];
    this.adding.replaceChildren(
      h("h3", {}, t("Add a source")),
      button(t("Fill in a form…"), () => this.addByHand()),
      button(t("Add a placeholder…"), () => void this.run("insert.citationPlaceholder")),
      ...providers.map((p) => button(p.label, () => this.openProvider(p))),
      ...notes.map((n) => h("p", { class: "src-help" }, n)),
    );
  }

  /** The style picker: Docxtor's styles first, then the other groups. */
  private drawStyles() {
    const styles = this.app.fe.extensions.citationStyleList();
    const option = (id: string, label: string) => h("option", { value: id }, label);
    const groups = new Map<string, HTMLElement[]>();
    const loose: HTMLElement[] = [];
    for (const s of styles) {
      if (!s.group) loose.push(option(s.id, s.label));
      else groups.set(s.group, [...(groups.get(s.group) ?? []), option(s.id, s.label)]);
    }
    const current = this.app.fe.engine?.citationStyle();
    // A document can name a style nobody offers now (an extension that is off): show it rather than another one.
    const missing = current && !styles.some((s) => s.id === current) ? [h("option", { value: current, disabled: true }, t("{id} (not available)", { id: current }))] : [];
    // Extensions that let people bring their own styles add these two lines at the end of the picker.
    const ext = this.app.fe.extensions;
    const action = (command: string) => {
      const c = ext.command(command);
      return c && ext.enabled(c) ? [h("option", { value: `${ACTION}${command}` }, c.title)] : [];
    };
    const actions = [...action("citations.addStyle"), ...action("citations.manageStyles")];
    this.style.replaceChildren(...loose, ...[...groups].map(([label, options]) => h("optgroup", { label }, ...options)), ...missing, ...(actions.length ? [h("optgroup", { label: t("Add or remove styles") }, ...actions)] : []));
    if (current) this.style.value = current;
  }

  /** Choose a style for the document; a CSL style is loaded first. */
  private async chooseStyle(id: string) {
    if (id.startsWith(ACTION)) {
      // "Add a style…" and "Manage styles…" are commands, not styles: run one and show the document's style again.
      this.drawStyles();
      return void this.run(id.slice(ACTION.length));
    }
    if (this.app.fe.readOnly) {
      toast(t("Switch to editing to change the citation style."));
      return this.update();
    }
    const ok = await this.app.fe.setCitationStyle(id);
    if (!ok) toast(t("That citation style isn't available."));
    this.update();
  }

  private openForm(opts: { source?: Source; title: string; sub?: string; ok: string }) {
    openSourceForm({
      ...opts,
      save: (source, showError) => !!this.app.editor.exec({ op: "add_source", source }, { onError: showError }),
      onClose: () => this.app.editor.focus(),
    });
  }

  /** The form for a source that is in the document. */
  edit(tag: string) {
    const ed = this.app.editor;
    const source = ed.engine?.sources().find((s) => s.tag === tag);
    if (!source) return;
    openSourceForm({
      source,
      title: t("Edit source"),
      ok: t("Save changes"),
      save: (input, showError) => !!ed.exec({ op: "update_source", tag, source: input }, { onError: showError }),
      onClose: () => ed.focus(),
    });
  }

  /** Delete a source. While citations use it, the engine says so and the person decides. */
  private async delete(s: Source) {
    const ed = this.app.editor;
    let refusal = "";
    if (ed.exec({ op: "delete_source", tag: s.tag }, { onError: (m) => (refusal = m) }) || !refusal) return;
    if (await this.app.fe.extensions.ask("citations.deleteSource", refusal)) ed.exec({ op: "delete_source", tag: s.tag, force: true });
  }

  private export() {
    const engine = this.app.fe.engine;
    if (!engine) return;
    this.app.download("references.bib", new TextEncoder().encode(engine.sourcesBibtex()), "application/x-bibtex");
  }
}
