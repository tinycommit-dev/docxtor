// Tiny DOM helpers (no framework: the editing surface needs full control of the DOM).
import { t, textDirection } from "@docxtor/extensions";

type Attrs = Record<string, string | number | boolean | EventListener | undefined | null>;
type Child = Node | string | null | undefined | false;

export function h<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Attrs = {}, ...children: Child[]): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === null || v === false) continue;
    if (k.startsWith("on") && typeof v === "function") el.addEventListener(k.slice(2).toLowerCase(), v as EventListener);
    else if (k === "class") el.className = String(v);
    else if (k === "html") el.innerHTML = String(v);
    else if (v === true) el.setAttribute(k, "");
    else el.setAttribute(k, String(v));
  }
  for (const c of children) if (c !== null && c !== undefined && c !== false) el.append(c);
  return el;
}

/**
 * `run` once, after the current task, however many times this is called before then. Registries change many times in a
 * row (an extension registers hundreds of commands and menu items as it starts): the menu bar and the toolbar draw
 * themselves again once, not once per change.
 */
/**
 * A list item that a click chooses and a double-click acts on (a suggestion, a field, a format). The second click is the double-click (its
 * `detail` is 2): a list that draws itself again when an item is chosen has replaced the element under the pointer by the second click, and
 * Firefox then sends no `dblclick` at all (Chrome and Safari send it to the new element).
 */
export function onChoose(el: HTMLElement, choose: () => void, act: () => void) {
  el.addEventListener("click", (e) => {
    choose();
    if (e.detail === 2) act();
  });
}

/** What takes the keyboard itself, or is dragged, typed in or edited: a mouse-down on it is left to the browser (`keepFocusOnFrame`). */
const TAKES_FOCUS = "input, textarea, select, button, a[href], label, summary, iframe, [contenteditable=''], [contenteditable='true'], [tabindex], [draggable='true'], .flow, .story-edit";

/**
 * Can the text in this element be selected? `user-select: none` set on a panel reaches its children only through `auto`, which Chrome
 * reports as the parent's `none` in the computed style and Firefox and Safari report as `auto`; so the nearest value that is not `auto`
 * decides. (Safari has had the prefixed name longest.)
 */
function selectable(el: Element): boolean {
  for (let e: Element | null = el; e; e = e.parentElement) {
    const cs = getComputedStyle(e) as CSSStyleDeclaration & { webkitUserSelect?: string };
    const v = cs.userSelect || cs.webkitUserSelect || "auto";
    if (v !== "auto") return v !== "none";
  }
  return true;
}

/**
 * A left mouse-down on the interface round the text that is not a control, something to edit or drag, or text to select (the frame,
 * the desk round the pages, the ruler, the status bar, a panel's empty space) keeps the keyboard where it was, as in Word; the browser
 * would move it to the page body. One rule for a whole root (`DocxtorEditor.useRoot`). Labels in the frame that are not for selecting say
 * so with `user-select: none`. Returns a function that removes it.
 */
export function keepFocusOnFrame(root: HTMLElement): () => void {
  const on = (e: MouseEvent) => {
    if (e.button !== 0 || e.defaultPrevented) return;
    const t = e.target;
    if (!(t instanceof Element) || t.closest(TAKES_FOCUS)) return;
    // Text a reader may want to select (a panel's message, a dialog's explanation) is the browser's.
    if (Array.from(t.childNodes).some((n) => n.nodeType === Node.TEXT_NODE && n.textContent!.trim()) && selectable(t)) return;
    e.preventDefault();
  };
  root.addEventListener("mousedown", on);
  return () => root.removeEventListener("mousedown", on);
}

export function coalesce(run: () => void): () => void {
  let queued = false;
  return () => {
    if (queued) return;
    queued = true;
    queueMicrotask(() => {
      queued = false;
      run();
    });
  };
}

export function svg(markup: string): SVGElement {
  const t = document.createElement("template");
  t.innerHTML = markup.trim();
  return t.content.firstElementChild as SVGElement;
}

export type Theme = "light" | "dark" | "system";
let layerEl: HTMLElement | null = null;
let layerTheme: Theme = "system";
let layerTokens: Record<string, string> = {};

/**
 * The overlay layer: popovers, dialogs, toasts and tool strips render here rather than loose in <body>,
 * so they carry Docxtor's tokens and theme on any host page. One layer is shared by every editor.
 */
export function overlayLayer(): HTMLElement {
  if (!layerEl || !layerEl.isConnected) {
    layerEl = h("div", { class: "docxtor docxtor-layer", dir: textDirection() });
    applyTheme(layerEl, layerTheme);
    applyTokens(layerEl, layerTokens, {});
    document.body.append(layerEl);
  }
  return layerEl;
}

/** Set data-theme on a Docxtor root ("system" removes it so the OS decides). */
export function applyTheme(el: HTMLElement, theme: Theme) {
  if (theme === "system") el.removeAttribute("data-theme");
  else el.dataset.theme = theme;
}

/** Theme for the shared overlay layer. */
export function setOverlayTheme(theme: Theme) {
  layerTheme = theme;
  if (layerEl) applyTheme(layerEl, theme);
}

/** Set CSS custom properties on `el`, removing the ones in `previous` that `tokens` doesn't set. */
export function applyTokens(el: HTMLElement, tokens: Record<string, string>, previous: Record<string, string>) {
  for (const k of Object.keys(previous)) if (!(k in tokens)) el.style.removeProperty(k);
  for (const [k, v] of Object.entries(tokens)) if (k.startsWith("--")) el.style.setProperty(k, v);
}

/** Token overrides (from a theme) for the shared overlay layer. */
export function setOverlayTokens(tokens: Record<string, string>) {
  if (layerEl) applyTokens(layerEl, tokens, layerTokens);
  layerTokens = { ...tokens };
}

let toastTimer = 0;
export function toast(msg: string, ms = 2800, action?: { label: string; run: () => void }) {
  overlayLayer().querySelector(".toast")?.remove();
  const t = h("div", { class: "toast", role: "status" }, msg);
  if (action) {
    // An action (Undo) stays longer, long enough to reach it.
    const b = h("button", { class: "toast-action", type: "button" }, action.label);
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => {
      t.remove();
      action.run();
    });
    t.append(b);
    ms = Math.max(ms, 8000);
  }
  overlayLayer().append(t);
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => t.remove(), ms);
}

/** Show a popover below `anchor` (an element, or a point such as the mouse). Returns a close function. */
export function popover(anchor: HTMLElement | { x: number; y: number }, content: HTMLElement, opts: { align?: "left" | "right"; onClose?: () => void } = {}): () => void {
  overlayLayer().querySelectorAll(".pop").forEach((p) => p.dispatchEvent(new Event("close")));
  const pop = h("div", { class: "pop", role: "menu" }, content);
  overlayLayer().append(pop);
  const el = anchor instanceof HTMLElement ? anchor : null;
  // A popover is a menu when it holds menu items; a palette, a form or a card is a small dialog that does not hold the keyboard, named
  // after the key that opened it. The content may be drawn after this call, so the role follows what is in it.
  const role = () => {
    const menu = content.dataset.popRole ? content.dataset.popRole === "menu" : !!pop.querySelector('[role^="menuitem"]');
    pop.setAttribute("role", menu ? "menu" : "dialog");
    if (menu) {
      if (pop.getAttribute("data-named") === "1") pop.removeAttribute("aria-label");
      pop.removeAttribute("data-named");
    } else if (!pop.hasAttribute("aria-label")) {
      pop.setAttribute("aria-label", content.getAttribute("aria-label") ?? el?.getAttribute("aria-label") ?? el?.title ?? t("Options"));
      pop.setAttribute("data-named", "1");
    }
  };
  role();
  const watch = new MutationObserver(role);
  watch.observe(pop, { childList: true, subtree: true });
  const r = el ? el.getBoundingClientRect() : { left: (anchor as { x: number }).x, right: (anchor as { x: number }).x, top: (anchor as { y: number }).y, bottom: (anchor as { y: number }).y };
  const gap = el ? 6 : 2;
  const pw = pop.offsetWidth;
  let left = opts.align === "right" ? r.right - pw : r.left;
  left = Math.max(8, Math.min(left, window.innerWidth - pw - 8));
  let top = r.bottom + gap;
  if (top + pop.offsetHeight > window.innerHeight - 8) top = Math.max(8, r.top - pop.offsetHeight - gap);
  pop.style.left = `${left}px`;
  pop.style.top = `${top}px`;
  // Where the keyboard was: it goes back there when the popover closes and took the focus with it (Escape on a card whose
  // button had the focus), as `modal` does. Rule U6: never on the page body.
  const opener = document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : null;
  const picked = window.getSelection();
  const range = picked && picked.rangeCount ? picked.getRangeAt(0).cloneRange() : null;
  const close = () => {
    if (!pop.isConnected) return;
    pop.remove();
    watch.disconnect();
    document.removeEventListener("mousedown", outside, true);
    document.removeEventListener("keydown", esc, true);
    opts.onClose?.();
    const now = document.activeElement;
    if (opener?.isConnected && !pop.contains(opener) && (!now || now === document.body)) {
      opener.focus({ preventScroll: true });
      if (range?.startContainer.isConnected && range.endContainer.isConnected && opener.contains(range.commonAncestorContainer)) {
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
      }
    }
  };
  const outside = (e: Event) => {
    if (!pop.contains(e.target as Node) && !el?.contains(e.target as Node)) close();
  };
  const esc = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      close();
    }
  };
  pop.addEventListener("close", close);
  document.addEventListener("keydown", esc, true);
  // After the click that opened it, so that click doesn't close it again.
  setTimeout(() => {
    document.addEventListener("mousedown", outside, true);
  });
  return close;
}

export function modal(content: HTMLElement, onClose?: () => void): () => void {
  const scrim = h("div", { class: "scrim" }, content);
  // Where the keyboard was: it goes back there when the dialog closes, unless `onClose` put it somewhere (rule U6: never
  // on the page body). One place for every dialog, so a dialog that forgets `onClose` does not lose the caret.
  const opener = document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : null;
  // Focusing a text area puts its caret at the start: the selection goes back too.
  const picked = window.getSelection();
  const range = picked && picked.rangeCount ? picked.getRangeAt(0).cloneRange() : null;
  // A dialog with tabs or a search box keeps the tallest height it has had: choosing a shorter tab, or a search that
  // finds less, doesn't make it jump.
  let tallest = 0;
  const steady = content.querySelector('[role="tablist"], input[type="search"]')
    ? new ResizeObserver(() => {
        const h = content.getBoundingClientRect().height;
        if (h <= tallest) return;
        tallest = h;
        // Never taller than the window allows, should it get smaller.
        content.style.minHeight = `min(${h}px, calc(100dvh - 48px))`;
      })
    : null;
  steady?.observe(content);
  const close = () => {
    steady?.disconnect();
    scrim.remove();
    document.removeEventListener("keydown", esc, true);
    onClose?.();
    const now = document.activeElement;
    if (opener?.isConnected && (!now || now === document.body)) {
      opener.focus({ preventScroll: true });
      if (range?.startContainer.isConnected && range.endContainer.isConnected && opener.contains(range.commonAncestorContainer)) {
        const now = window.getSelection();
        now?.removeAllRanges();
        now?.addRange(range);
      }
    }
  };
  // Escape closes the dialog on top (a picker opened from another dialog leaves that one open).
  const esc = (e: KeyboardEvent) => {
    if (e.key === "Tab") trapTab(e, scrim);
    // A menu opened from the dialog (a colour palette) takes the first Escape.
    if (e.key === "Escape" && !overlayLayer().querySelector(".pop") && Array.from(overlayLayer().querySelectorAll(":scope > .scrim")).pop() === scrim) close();
  };
  scrim.addEventListener("mousedown", (e) => {
    if (e.target === scrim) close();
  });
  document.addEventListener("keydown", esc, true);
  overlayLayer().append(scrim);
  const focusFirst = () => (content.querySelector("input:not([type=hidden]),select,textarea,button") as HTMLElement | null)?.focus();
  focusFirst();
  // A dialog that fills itself after this call (its first control is drawn a moment later) still starts with the focus inside it, so Tab
  // and a screen reader begin there and not on the page behind. With no control at all, the dialog itself holds the focus.
  queueMicrotask(() => {
    if (!scrim.isConnected || scrim.contains(document.activeElement)) return;
    focusFirst();
    if (!scrim.contains(document.activeElement)) {
      content.tabIndex = -1;
      content.focus({ preventScroll: true });
    }
  });
  return close;
}

/** What Tab can reach inside a box: visible, enabled controls and anything given a tab stop. */
const TABBABLE = 'a[href], button, input:not([type="hidden"]), select, textarea, summary, [contenteditable=""], [contenteditable="true"], [tabindex]';
function tabbable(box: HTMLElement): HTMLElement[] {
  return Array.from(box.querySelectorAll<HTMLElement>(TABBABLE)).filter((el) => {
    if (el.matches(":disabled") || el.tabIndex < 0 || el.closest("[hidden], [inert]")) return false;
    // A group of radio buttons is one stop: the chosen one, or the first when none is.
    if (el instanceof HTMLInputElement && el.type === "radio" && el.name) {
      const group = Array.from(box.querySelectorAll<HTMLInputElement>(`input[type="radio"]`)).filter((r) => r.name === el.name && r.form === el.form);
      if ((group.find((r) => r.checked) ?? group[0]) !== el) return false;
    }
    return el.getClientRects().length > 0 && getComputedStyle(el).visibility !== "hidden";
  });
}

/**
 * A dialog holds the keyboard: Tab from its last control goes to its first, Shift+Tab from its first goes to its last, so
 * the focus never reaches the page behind it (WCAG 2.4.3, and the ARIA pattern for a modal dialog). Only the dialog on top
 * does this; a menu opened from the dialog (a palette) keeps its own keys, because its controls are not inside the dialog.
 */
export function trapTab(e: KeyboardEvent, scrim: HTMLElement) {
  if (e.defaultPrevented) return;
  const top = Array.from(overlayLayer().querySelectorAll(":scope > .scrim")).pop();
  if (top !== scrim) return;
  const active = document.activeElement as HTMLElement | null;
  if (active && !scrim.contains(active) && active !== document.body && overlayLayer().contains(active)) return;
  const stops = tabbable(scrim);
  if (!stops.length) {
    e.preventDefault();
    return;
  }
  const first = stops[0];
  const last = stops[stops.length - 1];
  if (!active || !scrim.contains(active)) {
    e.preventDefault();
    (e.shiftKey ? last : first).focus();
  } else if (e.shiftKey && active === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && active === last) {
    e.preventDefault();
    first.focus();
  } else {
    // In the middle the browser moves the focus, and it may not agree with `tabbable` on what is a stop: Safari's Tab passes buttons and
    // links unless "Press Tab to highlight each item" is on, so from the last text field it leaves the dialog for the page behind it
    // (and Shift+Tab from the first). Once it has moved, a focus that is outside the dialog is brought back to its first or last stop.
    const back = e.shiftKey ? last : first;
    setTimeout(() => {
      const now = document.activeElement;
      if (!scrim.isConnected || (now && now !== document.body && (scrim.contains(now) || overlayLayer().contains(now)))) return;
      back.focus();
    }, 0);
  }
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** In-app replacement for window.prompt. Resolves null when cancelled. */
export function promptDialog(opts: { title: string; label: string; value?: string; ok?: string }): Promise<string | null> {
  return new Promise((resolve) => {
    let done = false;
    const input = h("input", { type: "text", value: opts.value ?? "", "aria-label": opts.label, spellcheck: "false" });
    const ok = h("button", { class: "btn primary", type: "button" }, opts.ok ?? t("OK"));
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const finish = (v: string | null) => {
      if (done) return;
      done = true;
      close();
      resolve(v);
    };
    const dialog = h(
      "div",
      { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": opts.title, style: "width:min(var(--dialog-s),100%)" },
      h("div", { class: "dialog-head" }, h("h2", {}, opts.title)),
      h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", {}, opts.label), input)),
      h("div", { class: "dialog-actions" }, cancel, ok),
    );
    const close = modal(dialog, () => finish(null));
    ok.addEventListener("click", () => finish(input.value.trim() || null));
    cancel.addEventListener("click", () => finish(null));
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") finish(input.value.trim() || null);
    });
    setTimeout(() => {
      input.focus();
      input.select();
    });
  });
}

/** In-app replacement for window.confirm. */
export function confirmDialog(opts: { title: string; body: string; ok?: string; danger?: boolean }): Promise<boolean> {
  return new Promise((resolve) => {
    let done = false;
    const ok = h("button", { class: `btn ${opts.danger ? "danger" : "primary"}`, type: "button" }, opts.ok ?? t("OK"));
    const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
    const finish = (v: boolean) => {
      if (done) return;
      done = true;
      close();
      resolve(v);
    };
    const dialog = h(
      "div",
      { class: "dialog", role: "alertdialog", "aria-modal": "true", "aria-label": opts.title, style: "width:min(var(--dialog-s),100%)" },
      h("div", { class: "dialog-head" }, h("h2", {}, opts.title), h("p", { class: "sub" }, opts.body)),
      h("div", { class: "dialog-actions" }, cancel, ok),
    );
    const close = modal(dialog, () => finish(false));
    ok.addEventListener("click", () => finish(true));
    cancel.addEventListener("click", () => finish(false));
    setTimeout(() => cancel.focus());
  });
}
