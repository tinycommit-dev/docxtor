// Insert, Special characters and Emoji: one dialog with a search box, categories, the characters of the
// category as a grid, and a preview of the one in focus. A character goes in at the caret when it is
// clicked or Enter is pressed, and the dialog stays open so several can be inserted.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { h, modal } from "./dom";
import type { Glyph } from "./special-chars-data";

const RECENT = 16;

export interface RecentStore {
  get(): string[];
  set(list: string[]): void;
}

/** What a space-like character looks like on its button. */
const showable = (c: string) => (/^\s$/.test(c) || c === "‑" ? (c === "‑" ? "‑" : "␣") : c);

/**
 * `pick` makes it a chooser: the character clicked goes to `pick` instead of into the document, and the dialog closes
 * (the bullet dialog uses it).
 */
export async function openSpecialCharacters(ed: Editor, opts: { category?: string; recent: RecentStore; pick?: (c: string) => void }) {
  // The characters are a module of their own: they load when the dialog is first asked for.
  const data = await import("./special-chars-data");
  const names = new Map<string, string>();
  for (const cat of data.CATEGORIES) for (const g of cat.groups) for (const i of g.items) if (!names.has(i.c)) names.set(i.c, i.n);

  let category = data.CATEGORIES.some((c) => c.id === opts.category) ? opts.category! : data.CATEGORIES[0].id;
  let query = "";
  let recent = opts.recent.get().filter((c) => typeof c === "string");

  const search = h("input", {
    type: "text",
    class: "sc-search",
    placeholder: t("Search by name, like arrow or euro"),
    "aria-label": t("Search by name"),
    "aria-controls": "sc-results",
    autocomplete: "off",
    spellcheck: "false",
  });
  const tabs = h("div", { class: "sc-tabs", role: "tablist", "aria-label": t("Categories") });
  const recentRow = h("div", { class: "sc-recent" });
  const results = h("div", { class: "sc-results", id: "sc-results" });
  const preview = h("div", { class: "sc-preview", "aria-live": "polite" });
  const done = h("button", { class: "btn primary", type: "button" }, t("Done"));

  const dialog = h(
    "div",
    { class: "dialog sc-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Special characters") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Special characters"))),
    h("div", { class: "sc-top" }, search, tabs),
    recentRow,
    results,
    preview,
    h("div", { class: "dialog-actions" }, done),
  );

  /** The buttons that hold a character, in reading order (the recent row, then the results). */
  const keys = () => Array.from(dialog.querySelectorAll<HTMLButtonElement>(".sc-key"));

  const describe = (c: string | null) => {
    preview.replaceChildren();
    if (!c) {
      preview.append(h("span", { class: "sc-hint" }, opts.pick ? t("Choose a character to use.") : t("Choose a character to insert it at the caret.")));
      return;
    }
    preview.append(
      h("span", { class: "sc-big", "aria-hidden": "true" }, showable(c)),
      h("span", { class: "sc-info" }, h("span", { class: "sc-name" }, names.get(c) ?? ""), h("span", { class: "sc-code readout" }, data.codePoints(c))),
    );
  };

  const key = (g: Glyph, inRecent = false) => {
    const b = h("button", { type: "button", class: "sc-key", title: g.n, "aria-label": g.n, "data-c": g.c, tabindex: "-1" }, showable(g.c));
    b.addEventListener("focus", () => describe(g.c));
    b.addEventListener("mouseenter", () => describe(g.c));
    b.addEventListener("click", () => insert(g.c, b));
    if (inRecent) b.dataset.recent = "1";
    return b;
  };

  const paintRecent = () => {
    recentRow.replaceChildren(h("span", { class: "sc-label" }, t("Recently used")));
    // The row is always there, so the dialog keeps its size when the first character goes in.
    if (!recent.length) {
      recentRow.append(h("span", { class: "sc-hint" }, t("Characters you insert are listed here.")));
      return;
    }
    recentRow.append(h("div", { class: "sc-keys", role: "group", "aria-label": t("Recently used") }, ...recent.map((c) => key({ c, n: names.get(c) ?? c }, true))));
    recentRow.querySelector<HTMLButtonElement>(".sc-key")!.tabIndex = 0;
  };

  const paintResults = () => {
    results.replaceChildren();
    results.scrollTop = 0;
    if (query.trim()) {
      const found = data.search(query);
      if (!found.length) results.append(h("p", { class: "sc-none", role: "status" }, t("No characters match “{query}”.", { query: query.trim() })));
      else results.append(h("div", { class: "sc-keys", role: "group", "aria-label": t("Search results") }, ...found.map((g) => key(g))));
    } else {
      const cat = data.CATEGORIES.find((c) => c.id === category)!;
      for (const g of cat.groups) {
        if (g.title) results.append(h("h3", { class: "sc-group" }, t(g.title)));
        results.append(h("div", { class: "sc-keys", role: "group", "aria-label": g.title ? t(g.title) : t(cat.title) }, ...g.items.map((i) => key(i))));
      }
    }
    for (const tab of tabs.children) {
      const on = !query.trim() && (tab as HTMLElement).dataset.id === category;
      tab.classList.toggle("on", on);
      tab.setAttribute("aria-selected", String(on));
    }
    // One stop in the grid for the Tab key; the arrows move within it.
    const first = results.querySelector<HTMLButtonElement>(".sc-key");
    if (first) first.tabIndex = 0;
    describe(first?.dataset.c ?? null);
  };

  for (const cat of data.CATEGORIES) {
    const tab = h("button", { type: "button", role: "tab", class: "sc-tab", "data-id": cat.id, "aria-selected": "false" }, t(cat.title));
    tab.addEventListener("click", () => {
      category = cat.id;
      query = "";
      search.value = "";
      paintResults();
    });
    tabs.append(tab);
  }

  const insert = (c: string, from: HTMLElement) => {
    if (opts.pick) {
      opts.pick(c);
      close();
      return;
    }
    if (!ed.insertText(c)) return;
    recent = [c, ...recent.filter((x) => x !== c)].slice(0, RECENT);
    opts.recent.set(recent);
    const focusedRecent = from.dataset.recent === "1";
    paintRecent();
    // Putting the caret back in the document took the focus: return it to the character just inserted.
    const again = keys().find((k) => k.dataset.c === c && (focusedRecent ? k.dataset.recent : !k.dataset.recent));
    (again ?? search).focus();
  };

  search.addEventListener("input", () => {
    query = search.value;
    paintResults();
  });
  search.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") {
      // The dialog's own arrow handling must not see this key again once the focus has moved.
      e.preventDefault();
      e.stopPropagation();
      keys().find((k) => !k.dataset.recent)?.focus();
    } else if (e.key === "Enter") {
      // Enter in the search box inserts the first match.
      e.preventDefault();
      const first = results.querySelector<HTMLButtonElement>(".sc-key");
      if (first) insert(first.dataset.c!, first);
    }
  });
  search.addEventListener("focus", () => describe(results.querySelector<HTMLElement>(".sc-key")?.dataset.c ?? null));

  tabs.addEventListener("keydown", (e) => {
    const list = Array.from(tabs.querySelectorAll<HTMLButtonElement>(".sc-tab"));
    const i = list.indexOf(document.activeElement as HTMLButtonElement);
    if (i < 0 || (e.key !== "ArrowRight" && e.key !== "ArrowLeft")) return;
    e.preventDefault();
    list[(i + (e.key === "ArrowRight" ? 1 : list.length - 1)) % list.length].focus();
  });

  // Arrow keys move through the characters by what is on screen: along a row, or to the nearest button in the row above or below.
  dialog.addEventListener("keydown", (e) => {
    const cur = document.activeElement as HTMLElement | null;
    if (!cur?.classList.contains("sc-key")) return;
    const list = keys();
    const i = list.indexOf(cur as HTMLButtonElement);
    let next: HTMLButtonElement | undefined;
    if (e.key === "ArrowRight") next = list[i + 1];
    else if (e.key === "ArrowLeft") next = list[i - 1];
    else if (e.key === "Home") next = list[0];
    else if (e.key === "End") next = list[list.length - 1];
    else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      const r = cur.getBoundingClientRect();
      const x = r.left + r.width / 2;
      const down = e.key === "ArrowDown";
      // The closest button whose row lies beyond this one.
      let best = Infinity;
      for (const k of list) {
        const kr = k.getBoundingClientRect();
        const dy = down ? kr.top - r.top : r.top - kr.top;
        if (dy < r.height / 2) continue;
        const score = dy * 1000 + Math.abs(kr.left + kr.width / 2 - x);
        if (score < best) {
          best = score;
          next = k;
        }
      }
      if (!next && !down) {
        e.preventDefault();
        search.focus();
        return;
      }
    } else return;
    if (!next) return;
    e.preventDefault();
    for (const k of list) k.tabIndex = -1;
    next.tabIndex = 0;
    next.focus();
    next.scrollIntoView({ block: "nearest" });
  });

  paintRecent();
  paintResults();
  const close = modal(dialog, () => {
    if (!opts.pick) ed.focus();
  });
  done.addEventListener("click", close);
  search.focus();
}
