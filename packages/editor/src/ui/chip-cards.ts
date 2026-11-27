// The card that opens when a smart chip is clicked (or Enter is pressed in it): who or what it is, what can be done with
// it (copy, send, open), a calendar or a list of choices, and Convert to text or Remove. Each card is a small dialog in
// a popover. The chip is a content control, so every change is an op through the control the extension was handed.
import { parseChoice, t, type ContentControlEditApi } from "@docxtor/extensions";
import { calendarElement, DATE_FORMATS, formatDate, parseIso } from "./control-dates";
import { openDropdownDialog } from "./chip-dialogs";
import { CHIP_ICONS } from "./chip-icons";
import { h } from "./dom";
import { I } from "./icons";

/** What the cards need from the app around the editor. */
export interface CardServices {
  /** Put text on the clipboard and say so. */
  copy(text: string, what: string): void;
  /** Give the focus back to the page. */
  focus(): void;
  /** Open an address: a document in the app, or a web address. */
  open(url: string): void;
  /** The date format and language new date chips use; the card remembers a change. */
  dateFormat(): { format: string; lid: string };
  rememberDateFormat(format: string): void;
}

/** The formats a date chip offers: Docs-like first, then Word's own. */
export const CHIP_DATE_FORMATS = ["MMM d, yyyy", ...DATE_FORMATS];

interface Built {
  root: HTMLElement;
  close: () => void;
}

function button(label: string, icon: string | null, run: () => void, primary = false): HTMLButtonElement {
  const b = h("button", { class: primary ? "btn primary" : "btn", type: "button" }, icon ? h("span", { class: "at-icon", html: icon, style: "width:14px;height:14px" }) : null, label);
  b.addEventListener("click", run);
  return b;
}

/** Open a card under the chip. `body` gets the function that closes it. */
function openCard(api: ContentControlEditApi, services: CardServices, label: string, wide: boolean, body: (close: () => void) => HTMLElement[]): Built {
  const root = h("div", { class: wide ? "chip-card wide" : "chip-card", "aria-label": label });
  let close = () => {};
  const done = () => {
    close();
    services.focus();
  };
  root.append(...body(done));
  close = api.popover(root);
  const pop = root.parentElement;
  pop?.setAttribute("role", "dialog");
  pop?.setAttribute("aria-label", label);
  // The first button takes the focus so the keyboard works at once; Escape closes the card (the popover does that).
  (root.querySelector("button, input, select") as HTMLElement | null)?.focus();
  return { root, close: done };
}

function head(icon: string | HTMLElement, title: string, sub?: string, extra?: HTMLElement | null): HTMLElement {
  const avatar = typeof icon === "string" ? h("span", { class: "card-avatar", html: icon, "aria-hidden": "true" }) : icon;
  return h("div", { class: "card-head" }, avatar, h("div", { class: "card-text" }, h("div", { class: "card-title" }, title), sub ? h("div", { class: "card-sub" }, sub) : null, extra));
}

/** The two ways to take a chip away. */
function leave(api: ContentControlEditApi, close: () => void, linked: boolean): HTMLElement {
  const toText = button(t("Convert to text"), null, () => {
    close();
    const ops = [{ op: "control", action: "remove", target: api.ref }, ...(linked ? [{ op: "set_link", from: api.range.from, to: api.range.to, url: null }] : [])];
    api.exec(ops);
  });
  const remove = button(t("Remove"), I.trash, () => {
    close();
    api.remove(true);
  });
  return h("div", { class: "card-actions" }, toText, remove);
}

const initials = (name: string) =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => Array.from(w)[0]?.toUpperCase() ?? "")
    .join("") || "?";

/**
 * The email address of a `mailto:` link, without `?subject=` and the rest; empty when what is left isn't one plain
 * address. Decoded first, then checked: a crafted `%3Fbcc%3D…` doesn't become a hidden recipient.
 */
export function emailOf(href: string | undefined): string {
  if (!href || !/^mailto:/i.test(href)) return "";
  let a: string;
  try {
    a = decodeURIComponent(href.slice(7).split("?")[0]);
  } catch {
    return "";
  }
  return /^[^\s@<>,;"?&=#%/\\]+@[^\s@<>,;"?&=#%/\\]+\.[^\s@<>,;"?&=#%/\\]+$/.test(a) ? a : "";
}

export function personCard(api: ContentControlEditApi, services: CardServices) {
  const name = api.control.text;
  const email = emailOf(api.control.href);
  openCard(api, services, t("Person: {name}", { name }), false, (close) => {
    const out: HTMLElement[] = [head(h("span", { class: "card-avatar", "aria-hidden": "true" }, initials(name)), name, email || undefined)];
    const acts = h("div", { class: "card-actions" });
    if (email) {
      acts.append(button(t("Copy email"), I.copy, () => (close(), services.copy(email, t("Email address copied")))));
      const send = h("a", { class: "btn", href: `mailto:${encodeURIComponent(email).replace("%40", "@")}`, target: "_blank", rel: "noopener" }, h("span", { class: "at-icon", html: CHIP_ICONS.mail, style: "width:14px;height:14px" }), t("Send email"));
      send.addEventListener("click", close);
      acts.append(send);
    }
    out.push(acts, h("div", { class: "card-rule" }), leave(api, close, true));
    return out;
  });
}

export function fileCard(api: ContentControlEditApi, services: CardServices) {
  const title = api.control.text;
  const url = api.control.href ?? "";
  openCard(api, services, t("File: {name}", { name: title }), false, (close) => {
    const out: HTMLElement[] = [head(CHIP_ICONS.chipFile, title, undefined, url ? h("div", { class: "card-url" }, url) : null)];
    const acts = h("div", { class: "card-actions" });
    if (url) {
      acts.append(button(t("Open"), CHIP_ICONS.externalLink, () => (close(), services.open(url)), true));
      acts.append(button(t("Copy link"), I.copy, () => (close(), services.copy(url, t("Link copied")))));
    }
    out.push(acts, h("div", { class: "card-rule" }), leave(api, close, true));
    return out;
  });
}

export function placeCard(api: ContentControlEditApi, services: CardServices) {
  const name = api.control.text;
  const url = api.control.href ?? "";
  openCard(api, services, t("Place: {name}", { name }), false, (close) => {
    const out: HTMLElement[] = [head(CHIP_ICONS.chipPlace, name, undefined, url ? h("div", { class: "card-url" }, url) : null)];
    const acts = h("div", { class: "card-actions" });
    if (url) acts.append(button(t("Open in maps"), CHIP_ICONS.externalLink, () => (close(), services.open(url)), true));
    acts.append(button(t("Copy name"), I.copy, () => (close(), services.copy(name, t("Name copied")))));
    out.push(acts, h("div", { class: "card-rule" }), leave(api, close, true));
    return out;
  });
}

export function dateCard(api: ContentControlEditApi, services: CardServices) {
  const lid = api.control.lid || services.dateFormat().lid;
  const current = api.control.placeholder ? undefined : api.control.fullDate;
  const day = parseIso(current) ?? new Date();
  openCard(api, services, t("Date"), true, (close) => {
    const cal = calendarElement(
      {
        lid,
        current,
        pick: (iso) => {
          close();
          api.set({ date: iso });
        },
      },
      () => undefined,
    );
    const select = h("select", { id: "chip-date-format" });
    const active = api.control.dateFormat ?? services.dateFormat().format;
    const formats = CHIP_DATE_FORMATS.includes(active) ? CHIP_DATE_FORMATS : [active, ...CHIP_DATE_FORMATS];
    for (const f of formats) select.append(h("option", { value: f, ...(f === active ? { selected: true } : {}) }, formatDate(f, lid, day)));
    select.addEventListener("change", () => {
      services.rememberDateFormat(select.value);
      api.props({ dateFormat: select.value });
    });
    return [cal, h("div", { class: "card-field" }, h("label", { for: "chip-date-format" }, t("Format")), select), h("div", { class: "card-rule" }), leave(api, close, false)];
  });
}

export function dropdownCard(api: ContentControlEditApi, services: CardServices) {
  const items = api.control.items ?? [];
  // The first entry is the prompt Word shows when nothing is chosen.
  const prompt = items[0]?.display ?? "";
  const options = items.slice(1);
  openCard(api, services, api.control.alias || t("Drop-down"), false, (close) => {
    const list = h("div", { class: "choice-list", role: "listbox", "aria-label": api.control.alias || t("Drop-down") });
    options.forEach((o, i) => {
      const { color, label } = parseChoice(o.value);
      const on = !api.control.placeholder && o.display === api.control.text;
      const b = h(
        "button",
        { class: "choice", type: "button", role: "option", "aria-selected": String(on) },
        h("span", { class: "choice-check", html: on ? I.check : "" }),
        h("span", { class: "choice-pill", "data-color": color }, label || o.display),
      );
      b.addEventListener("click", () => {
        close();
        api.set({ index: i + 1 });
      });
      list.append(b);
    });
    const clear = h("button", { class: "choice", type: "button", role: "option", "aria-selected": String(!!api.control.placeholder) }, h("span", { class: "choice-check", html: api.control.placeholder ? I.check : "" }), h("span", {}, t("Clear")));
    clear.addEventListener("click", () => {
      close();
      api.set({ index: 0 });
    });
    list.append(clear);
    // Arrow keys move between the choices.
    list.addEventListener("keydown", (e) => {
      const all = Array.from(list.querySelectorAll<HTMLElement>(".choice"));
      const i = all.indexOf(document.activeElement as HTMLElement);
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        all[(i + (e.key === "ArrowDown" ? 1 : -1) + all.length) % all.length]?.focus();
      }
    });
    const edit = button(t("Edit options…"), null, () => {
      close();
      openDropdownDialog({
        edit: true,
        title: api.control.alias || "",
        options: options.map((o) => {
          const c = parseChoice(o.value);
          return { label: c.label || o.display, color: c.color };
        }),
        onSave: (title, opts) => {
          const kept = opts.map((o) => ({ display: o.label, value: `${o.color ?? "gray"}:${o.label}` }));
          api.props({ alias: title, items: [{ display: prompt || title, value: prompt || title }, ...kept] });
        },
        onClose: services.focus,
      });
    });
    return [list, h("div", { class: "card-rule" }), h("div", { class: "card-actions" }, edit), leave(api, close, false)];
  });
}
