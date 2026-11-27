// The start screen: a front panel of actions plus the recent documents.
import type { PreviewLine } from "./files/fs";
import { I, h, svg, t } from "@docxtor/editor";
import type { App } from "./main";
import { DOC_EXT, canPickFiles, canUseFolders, recents, setPinned } from "./files/fs";

// ---------------------------------------------------------------------------
// Start screen: a front panel of actions plus recent documents
// ---------------------------------------------------------------------------

/** First lines of a document from its DocScript, stripped of Markdown, for page thumbnails. */
export function previewLines(script: string): PreviewLine[] {
  const out: PreviewLine[] = [];
  for (const line of script.split("\n").slice(1)) {
    if (line.startsWith("#")) break; // headers, footers and notes follow the body
    const m = /^\s*\[(p|t)\d+([^\]]*)\]\s?(.*)$/.exec(line);
    if (!m || m[1] !== "p") continue;
    const tags = m[2];
    const t = m[3].replace(/!\[[^\]]*\]/g, "").replace(/\[([^\]]*)\]\([^)]*\)/g, "$1").replace(/[*_~`<>]|\/?u>/g, "").replace(/\\n/g, " ").trim();
    if (!t) continue;
    const k: PreviewLine["k"] = /\bTitle\b/.test(tags) ? "title" : /Heading|Subtitle/.test(tags) ? "h" : /list:/.test(tags) ? "li" : "p";
    out.push({ k, t: t.slice(0, 140) });
    if (out.length >= 9) break;
  }
  return out;
}

/** Reopening a file or folder needs a click before the browser grants access again. */
function resumeNotice(r: Resume): HTMLElement {
  const b = h("button", { class: "btn primary", type: "button" }, t("Open “{name}”", { name: r.name.replace(DOC_EXT, "") }));
  b.addEventListener("click", () => void r.open());
  return h(
    "div",
    { class: "resume" },
    h("p", {}, h("b", {}, t("Continue where you left off. ")), "Your browser needs your permission before Docxtor can open this file again."),
    b,
  );
}

let startEl: HTMLElement | null = null;
export function closeStart() {
  startEl?.remove();
  startEl = null;
  // The editor under the start screen is back (it was out of the keyboard's way and the screen reader's while the start screen covered it).
  document.getElementById("app")?.querySelectorAll(":scope > [inert]").forEach((el) => el.removeAttribute("inert"));
  document.removeEventListener("keydown", startKeys);
}
let startKeys: (e: KeyboardEvent) => void = () => undefined;

export interface Resume {
  name: string;
  open: () => Promise<void>;
}

export async function showStart(app: App, resume?: Resume) {
  closeStart();
  const actions: [string, () => void][] = [];
  const kbd = (k: string) => h("kbd", { class: "kbd" }, k);
  const action = (k: string, label: string, title: string, fn: () => void) => {
    actions.push([k.toLowerCase(), fn]);
    const b = h("button", { class: "btn ghost start-action", type: "button", title }, label, kbd(k));
    b.addEventListener("click", fn);
    return b;
  };
  const newDoc = () => void app.createIn(app.files.library, "");
  actions.push(["n", newDoc]);
  const blank = h(
    "button",
    { class: "doc-tile new", type: "button", title: `${t("New document")} (N)` },
    h("span", { class: "page" }, h("span", { class: "plus", "aria-hidden": "true" })),
    h("span", { class: "cap" }, h("b", {}, t("New document"), kbd("N")), h("span", {}, t("Blank, with Word's styles"))),
  );
  blank.addEventListener("click", newDoc);
  const tiles = h("div", { class: "desk-tiles" }, blank);
  startKeys = (e: KeyboardEvent) => {
    if (e.metaKey || e.ctrlKey || e.altKey || (e.target as HTMLElement).tagName === "INPUT") return;
    const hit = actions.find(([k]) => k === e.key.toLowerCase());
    if (hit) {
      e.preventDefault();
      hit[1]();
    }
  };
  document.addEventListener("keydown", startKeys);
  startEl = h(
    "div",
    { class: "start" },
    h(
      "div",
      { class: "start-inner" },
      h("span", { class: "wordmark" }, "docxtor"),
      h("h1", {}, t("Your documents")),
      h(
        "p",
        { class: "lede" },
        t("Open a .docx, edit it on real pages, and save it back the way Word wrote it. Bring your own AI to edit alongside you, with every change shown and reversible."),
      ),
      resume ? resumeNotice(resume) : null,
      tiles,
      h(
        "div",
        { class: "start-actions" },
        action("O", t("Open a file"), canPickFiles ? t("From your computer. Saving writes it back in place.") : t("From your computer. A copy is kept in your Library."), () => void app.openPicker()),
        canUseFolders ? action("F", t("Open a folder"), t("Browse and edit a whole folder of documents"), () => void app.files.connectFolder().then(() => app.sidebar.show("files"))) : null,
        app.fe.extensions.templates.list().length ? action("U", t("Use a template"), t("A letter, meeting notes, a report or a paper"), () => void app.newFromTemplate()) : null,
        action("T", t("Take the tour"), t("A sample document that shows what Docxtor does"), () => void app.sample(false)),
      ),
      h("p", { class: "start-foot" }, t("Free and open source, MIT or Apache-2.0. Runs in your browser; documents stay on your machine.")),
    ),
  );
  // Inside the app root, so the start screen carries Docxtor's tokens and theme.
  document.getElementById("app")!.append(startEl);
  // The start screen covers the editor: Tab and the screen reader stay on the start screen, not on the hidden toolbar behind it.
  for (const el of Array.from(document.getElementById("app")!.children)) if (el !== startEl) el.setAttribute("inert", "");
  const recent = await recents();
  if (!startEl) return;
  // Pinned documents first, then the latest few.
  for (const r of [...recent.filter((x) => x.pinned), ...recent.filter((x) => !x.pinned).slice(0, 7)]) {
    const where = app.recentPlace(r);
    const date = new Date(r.at).toLocaleDateString(undefined, { month: "short", day: "numeric" });
    const page = h("span", { class: "page" });
    for (const l of r.preview ?? []) page.append(h("span", { class: `pl ${l.k}` }, l.t));
    if (r.locked) page.append(svg(I.lock), h("span", { class: "pl lock-note" }, t("Protected with a password")));
    const b = h(
      "button",
      { class: `doc-tile${r.locked ? " locked" : ""}`, type: "button", title: r.path },
      page,
      h("span", { class: "cap" }, h("b", {}, r.name.replace(DOC_EXT, "")), h("span", {}, t("{place}, {date}", { place: where, date }))),
    );
    b.addEventListener("click", () => void app.openRecent(r));
    const name = r.name.replace(DOC_EXT, "");
    const pin = h(
      "button",
      { class: `tile-pin${r.pinned ? " on" : ""}`, type: "button", title: r.pinned ? t("Unpin") : t("Pin to the top"), "aria-label": r.pinned ? t("Unpin {name}", { name }) : t("Pin {name} to the top", { name }), "aria-pressed": String(!!r.pinned) },
      svg(I.bookmark),
    );
    pin.addEventListener("click", async (e) => {
      e.stopPropagation();
      const wasStart = startEl;
      await setPinned(r.key, !r.pinned);
      if (wasStart === startEl) void showStart(app);
    });
    tiles.append(h("div", { class: `tile-wrap${r.pinned ? " pinned" : ""}` }, b, pin));
  }
}


/** Whether the start screen is showing. */
export const startOpen = (): boolean => startEl !== null;

/** The start screen's element while it is showing. */
export const startElement = (): HTMLElement | null => startEl;
