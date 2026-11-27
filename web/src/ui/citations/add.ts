// "Add a source" for any source provider: one dialog fitted to what the provider can do. A provider with `search`
// gets a search box and results as the person types; one with `resolve` gets a line (a DOI) or a text box and a
// file (BibTeX). What is found is listed to tick, or, when the provider asks for it, opened in the source form to
// check. Sources the document already has are shown but left out.
import { h, modal, sourceDetail, sourceTitle, t, tn, type Source, type SourceProvider } from "@docxtor/editor";
import { isDuplicate } from "./lookup";

let seq = 0;

export interface AddHost {
  /** The sources the document has now. */
  have: () => Source[];
  /** Add the ticked sources; false keeps the dialog open. */
  add: (sources: Source[]) => boolean;
  /** Open one source in the form to check before it is saved. */
  review: (source: Source) => void;
  onClose?: () => void;
}

const failure = (e: unknown) => String((e as Error)?.message ?? e);

export function openProviderDialog(p: SourceProvider, host: AddHost) {
  const id = `src${++seq}`;
  const title = p.title ?? p.label.replace(/…$/, "");
  const search = !!p.search;
  const lines = !search && !p.multiline;
  const lookup = p.action ?? t("Look up");
  // A provider that finds one source and wants it checked is a one-step lookup; the others list what they find.
  const checks = !search && !!p.review;

  const input: HTMLInputElement | HTMLTextAreaElement = search
    ? h("input", { type: "search", id, autocomplete: "off", spellcheck: "false", placeholder: p.placeholder ?? "" })
    : p.multiline
      ? h("textarea", { id, rows: 7, class: "src-paste", spellcheck: "false", placeholder: p.placeholder ?? "" })
      : h("input", { type: "text", id, autocomplete: "off", spellcheck: "false", placeholder: p.placeholder ?? "" });
  const status = h("p", { class: "bm-error", role: "alert", hidden: true });
  const preview = h("div", { class: "src-preview", "aria-live": "polite" });
  const go = h("button", { class: "btn primary", type: "submit", disabled: !checks }, checks ? lookup : t("Add sources"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const find = h("button", { class: "btn sm", type: "button" }, lookup);

  let file: HTMLInputElement | null = null;
  let choose: HTMLButtonElement | null = null;
  if (p.multiline && p.files) {
    file = h("input", { type: "file", accept: p.files.accept, hidden: true, "aria-label": p.files.label });
    choose = h("button", { class: "btn sm", type: "button" }, p.files.button);
  }
  const settings = p.settings ? h("button", { class: "btn ghost src-settings", type: "button" }, p.settings.label) : null;

  const field = h(
    "div",
    { class: "field" },
    p.inputLabel ? h("label", { for: id }, p.inputLabel) : null,
    input,
    file && choose ? h("div", { class: "src-paste-actions" }, choose, file) : null,
    !checks && lines ? h("div", { class: "src-paste-actions" }, find) : null,
    p.help ? h("span", { class: "help" }, p.help) : null,
    status,
  );
  const form = h(
    "form",
    { class: `dialog${lines && checks ? "" : " src-dialog"}`, role: "dialog", "aria-modal": "true", "aria-label": title, ...(lines && checks ? { style: "width:min(var(--dialog-m),100%)" } : {}), novalidate: true },
    h("div", { class: "dialog-head" }, h("h2", {}, title), p.intro ? h("p", { class: "sub" }, p.intro) : null),
    h("div", { class: "dialog-body" }, field, checks ? null : preview),
    h("div", { class: "dialog-actions" }, settings, cancel, go),
  );
  const abort = { current: new AbortController() };
  const close = modal(form, () => {
    abort.current.abort();
    window.clearTimeout(timer);
    host.onClose?.();
  });
  cancel.addEventListener("click", close);
  settings?.addEventListener("click", async () => {
    await p.settings?.open();
    if (search) void run();
  });

  type Row = { source: Source; dup: boolean; box: HTMLInputElement };
  let rows: Row[] = [];
  let timer = 0;

  const sync = () => {
    if (checks) return;
    const n = rows.filter((r) => r.box.checked).length;
    go.disabled = n === 0;
    go.textContent = n ? tn(n, "Add {n} source", "Add {n} sources") : t("Add sources");
  };

  const show = (found: Source[]) => {
    const known = host.have();
    const seen: Source[] = [];
    rows = found.map((source) => {
      const dup = isDuplicate(known, source) || isDuplicate(seen, source);
      if (!dup) seen.push(source);
      const box = h("input", { type: "checkbox", checked: !dup && !search, disabled: dup });
      box.addEventListener("change", sync);
      return { source, dup, box };
    });
    preview.replaceChildren(
      h("h3", {}, found.length ? tn(found.length, "{n} source found", "{n} sources found") : t("No sources found")),
      ...rows.map((r) =>
        h(
          "label",
          { class: `src-found${r.dup ? " dup" : ""}` },
          r.box,
          h(
            "span",
            { class: "src-found-text" },
            h("span", { class: "cite-src-title" }, sourceTitle(r.source)),
            sourceDetail(r.source) ? h("span", { class: "cite-src-detail" }, sourceDetail(r.source)) : null,
            r.dup ? h("span", { class: "cite-src-detail" }, t("Already in this document, so it is left out.")) : null,
          ),
        ),
      ),
    );
    sync();
  };

  const fail = (message: string) => {
    rows = [];
    preview.replaceChildren();
    status.textContent = message;
    status.hidden = false;
    sync();
  };

  /** Ask the provider about what is in the input. */
  const run = async () => {
    abort.current.abort();
    abort.current = new AbortController();
    const signal = abort.current.signal;
    const text = input.value;
    status.hidden = true;
    // A live box with nothing in it has nothing to read. A search with nothing typed asks for what the provider
    // has to show first (recent items), and a one-line lookup lets the provider say what is wrong.
    if (!text.trim() && p.live) {
      rows = [];
      preview.replaceChildren();
      return sync();
    }
    if (checks) {
      go.disabled = true;
      go.textContent = t("Looking up…");
    } else if (search) {
      preview.replaceChildren(h("p", { class: "src-detail" }, t("Searching…")));
    }
    try {
      if (search) {
        const found = await p.search!(text, signal);
        if (signal.aborted) return;
        show(found.map((r) => r.source));
      } else {
        const found = await p.resolve!(text, signal);
        if (signal.aborted) return;
        if (checks) {
          const first = found[0];
          if (!first) throw new Error(t("No details were found for that."));
          close();
          host.review(first);
        } else {
          show(found);
        }
      }
    } catch (e) {
      if ((e as Error)?.name === "AbortError" || signal.aborted) return;
      fail(failure(e));
    } finally {
      if (checks && !signal.aborted) {
        go.disabled = false;
        go.textContent = lookup;
      }
    }
  };

  const soon = (ms: number) => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void run(), ms);
  };
  input.addEventListener("input", () => {
    if (search) soon(300);
    else if (p.live) soon(200);
    else status.hidden = true;
  });
  find.addEventListener("click", () => void run());
  choose?.addEventListener("click", () => file?.click());
  file?.addEventListener("change", async () => {
    const f = file?.files?.[0];
    if (file) file.value = "";
    if (!f) return;
    input.value = await f.text();
    void run();
  });
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    if (checks) return void run();
    // Enter in a one-line input looks it up; with something found, Enter adds what is ticked.
    if (lines && !rows.length) return void run();
    const picked = rows.filter((r) => r.box.checked).map((r) => r.source);
    if (picked.length && host.add(picked)) close();
  });
  sync();
  input.focus();
  if (search) void run();
}
