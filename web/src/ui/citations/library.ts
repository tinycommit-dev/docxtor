// The shared list of sources (Word's master list in the Source Manager): sources every document can use. One column
// is the list kept by a library (a contribution: this browser's by default), the other is this document's list; sources
// are copied between them. A library the document can't reach (an extension that is off) is simply not offered.
import { h, modal, sourceDetail, sourceTitle, t, tn, toast, type Source, type SourceLibrary } from "@docxtor/editor";
import type { CitationsHost } from "../../citations-host";
import { isDuplicate } from "./lookup";

let seq = 0;

const failure = (e: unknown) => String((e as Error)?.message ?? e);

export async function openLibraryDialog(app: CitationsHost, after: () => void) {
  const libraries = app.fe.extensions.sourceLibraries.list();
  if (!libraries.length) {
    toast(t("There is no shared list of sources yet. Turn on a place for it under Extensions."));
    return;
  }
  const ed = app.editor;
  const id = `lib${++seq}`;
  let library: SourceLibrary = libraries[0];
  let shared: Source[] = [];
  const ticked = { shared: new Set<number>(), doc: new Set<number>() };
  const picker = h("select", { id: `${id}-lib`, "aria-label": t("Shared list") }, ...libraries.map((l) => h("option", { value: l.id }, l.label)));
  const note = h("p", { class: "help" }, library.note ?? "");
  const status = h("p", { class: "bm-error", role: "alert", hidden: true });
  const sharedList = h("ul", { class: "src-list lib-list", "aria-label": t("Sources in the shared list") });
  const docList = h("ul", { class: "src-list lib-list", "aria-label": t("Sources in this document") });
  const toDoc = h("button", { class: "btn", type: "button" }, t("Copy to this document"));
  const toShared = h("button", { class: "btn", type: "button" }, t("Copy to the shared list"));
  const forget = h("button", { class: "btn ghost", type: "button" }, t("Remove from the shared list"));
  const done = h("button", { class: "btn primary", type: "button" }, t("Close"));

  const row = (s: Source, i: number, side: "shared" | "doc") => {
    const box = h("input", { type: "checkbox", id: `${id}-${side}-${i}`, "aria-label": sourceTitle(s) });
    box.addEventListener("change", () => {
      box.checked ? ticked[side].add(i) : ticked[side].delete(i);
      buttons();
    });
    const detail = sourceDetail(s);
    return h("li", { class: "src-item" }, h("label", { class: "check", for: `${id}-${side}-${i}` }, box, h("span", {}, h("span", { class: "src-title" }, sourceTitle(s)), detail ? h("span", { class: "src-detail" }, ` ${detail}`) : null)));
  };
  const buttons = () => {
    toDoc.disabled = !ticked.shared.size;
    forget.disabled = !ticked.shared.size;
    toShared.disabled = !ticked.doc.size;
  };
  const fail = (m: string) => {
    status.textContent = m;
    status.hidden = !m;
  };
  const paint = () => {
    const mine = ed.engine?.sources() ?? [];
    sharedList.replaceChildren(...(shared.length ? shared.map((s, i) => row(s, i, "shared")) : [h("li", { class: "empty-note" }, t("The shared list is empty. Copy sources into it from this document."))]));
    docList.replaceChildren(...(mine.length ? mine.map((s, i) => row(s, i, "doc")) : [h("li", { class: "empty-note" }, t("This document has no sources."))]));
    buttons();
  };
  const load = async () => {
    try {
      shared = [...(await library.list())];
      fail("");
    } catch (e) {
      shared = [];
      fail(failure(e));
    }
    ticked.shared.clear();
    ticked.doc.clear();
    paint();
  };
  picker.addEventListener("change", () => {
    library = libraries.find((l) => l.id === picker.value) ?? libraries[0];
    note.textContent = library.note ?? "";
    void load();
  });

  toDoc.addEventListener("click", () => {
    const have = ed.engine?.sources() ?? [];
    const picked = [...ticked.shared].map((i) => shared[i]).filter((s) => !isDuplicate(have, s));
    if (!picked.length) return fail(t("This document already has those sources."));
    // A tag the document uses for something else is made again by the engine.
    const ops = picked.map((s) => ({ op: "add_source", source: have.some((h2) => h2.tag === s.tag) ? { ...s, tag: "" } : s }) as const);
    if (ed.exec(ops, { onError: fail })) {
      toast(tn(picked.length, "Copied {n} source to this document", "Copied {n} sources to this document"));
      after();
      void load();
    }
  });
  toShared.addEventListener("click", async () => {
    const mine = ed.engine?.sources() ?? [];
    const picked = [...ticked.doc].map((i) => mine[i]).filter((s) => !isDuplicate(shared, s) && !(s.authors.length === 0 && (!s.title || s.title === s.tag)));
    if (!picked.length) return fail(t("The shared list already has those sources."));
    try {
      await library.save([...shared, ...picked]);
      toast(tn(picked.length, "Copied {n} source to the shared list", "Copied {n} sources to the shared list"));
      await load();
    } catch (e) {
      fail(failure(e));
    }
  });
  forget.addEventListener("click", async () => {
    if (!(await app.fe.extensions.ask("citations.forgetShared", ticked.shared.size))) return;
    try {
      await library.save(shared.filter((_, i) => !ticked.shared.has(i)));
      await load();
    } catch (e) {
      fail(failure(e));
    }
  });

  const dialog = h(
    "div",
    { class: "dialog lib-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Shared list of sources") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Shared list of sources")), h("p", { class: "sub" }, t("Sources kept apart from any document, to use in all of them. Copy them across in either direction."))),
    h(
      "div",
      { class: "dialog-body" },
      libraries.length > 1 ? h("div", { class: "field" }, h("label", { for: `${id}-lib` }, t("Keep the list")), picker) : null,
      note,
      status,
      h("div", { class: "lib-cols" }, h("div", {}, h("h3", {}, t("Shared list")), sharedList, h("div", { class: "lib-actions" }, toDoc, forget)), h("div", {}, h("h3", {}, t("This document")), docList, h("div", { class: "lib-actions" }, toShared))),
    ),
    h("div", { class: "dialog-actions" }, done),
  );
  const close = modal(dialog, () => after());
  done.addEventListener("click", close);
  await load();
}
