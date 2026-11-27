// Dates in content controls: Word's date formats (`M/d/yyyy`, `dddd, MMMM d, yyyy`) and the calendar that picks one.
// The engine formats the text it writes into the document; this is for previews and the calendar.
import { t } from "@docxtor/extensions";
import { h, popover } from "./dom";
import { I } from "./icons";

/** Word's own list of date formats, offered in the Properties dialog. */
export const DATE_FORMATS = ["M/d/yyyy", "dddd, MMMM d, yyyy", "MMMM d, yyyy", "M/d/yy", "yyyy-MM-dd", "d-MMM-yy", "M.d.yyyy", "MMM. d, yy", "d MMMM yyyy", "MMMM yy", "MMM-yy", "dd/MM/yyyy"];

/** The languages the dialog offers for a date, as Word writes them (`w:lid`). */
export const DATE_LANGUAGES: [string, () => string][] = [
  ["en-US", () => t("English (United States)")],
  ["en-GB", () => t("English (United Kingdom)")],
  ["es-ES", () => t("Spanish (Spain)")],
  ["fr-FR", () => t("French (France)")],
  ["de-DE", () => t("German (Germany)")],
  ["it-IT", () => t("Italian (Italy)")],
  ["pt-BR", () => t("Portuguese (Brazil)")],
];

function locale(lid: string | undefined): string {
  try {
    return new Intl.DateTimeFormat(lid || "en-US").resolvedOptions().locale;
  } catch {
    return "en-US";
  }
}

/** A date in a Word date format. Text in single quotes is kept as it is. */
export function formatDate(fmt: string, lid: string | undefined, d: Date): string {
  const loc = locale(lid);
  const name = (o: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat(loc, { ...o, timeZone: "UTC" }).format(d);
  let out = "";
  for (let i = 0; i < fmt.length; ) {
    const c = fmt[i];
    if (c === "'") {
      const end = fmt.indexOf("'", i + 1);
      if (fmt[i + 1] === "'") {
        out += "'";
        i += 2;
      } else {
        out += fmt.slice(i + 1, end < 0 ? undefined : end);
        i = end < 0 ? fmt.length : end + 1;
      }
      continue;
    }
    let run = 1;
    while (fmt[i + run] === c) run++;
    const day = d.getUTCDate();
    const month = d.getUTCMonth() + 1;
    const year = d.getUTCFullYear();
    if (c === "d") out += run === 1 ? String(day) : run === 2 ? String(day).padStart(2, "0") : name({ weekday: run === 3 ? "short" : "long" });
    else if (c === "M") out += run === 1 ? String(month) : run === 2 ? String(month).padStart(2, "0") : name({ month: run === 3 ? "short" : "long" });
    else if (c === "y") out += run <= 2 ? String(year % 100).padStart(2, "0") : String(year).padStart(4, "0");
    else out += c.repeat(run);
    i += run;
  }
  return out;
}

/** `2026-10-14` as a UTC date, or null. */
export function parseIso(s: string | undefined): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(s ?? "");
  return m ? new Date(Date.UTC(+m[1], +m[2] - 1, +m[3])) : null;
}

export function toIso(d: Date): string {
  return d.toISOString().slice(0, 10);
}

/** Open the calendar under `anchor`. `pick` gets `2026-10-14`, or an empty string for Clear. */
export function openCalendar(anchor: HTMLElement, opts: { lid?: string; current?: string; pick: (iso: string) => void }): () => void {
  let close = () => {};
  const root = calendarElement(opts, () => close());
  close = popover(anchor, root);
  return close;
}

/** The calendar itself, to put in a popover or a card. `close` is called before `pick`, so the host can close what holds it. */
export function calendarElement(opts: { lid?: string; current?: string; pick: (iso: string) => void }, close: () => void): HTMLElement {
  const loc = locale(opts.lid);
  const today = new Date();
  const todayUtc = new Date(Date.UTC(today.getFullYear(), today.getMonth(), today.getDate()));
  const chosen = parseIso(opts.current);
  let view = new Date(Date.UTC((chosen ?? todayUtc).getUTCFullYear(), (chosen ?? todayUtc).getUTCMonth(), 1));
  const root = h("div", { class: "cal", role: "group", "aria-label": t("Choose a date") });
  const draw = () => {
    root.textContent = "";
    const title = new Intl.DateTimeFormat(loc, { month: "long", year: "numeric", timeZone: "UTC" }).format(view);
    const prev = h("button", { class: "ctl-btn", type: "button", "aria-label": t("Previous month"), html: I.chevron, style: "transform:rotate(90deg)" });
    const next = h("button", { class: "ctl-btn", type: "button", "aria-label": t("Next month"), html: I.chevron, style: "transform:rotate(-90deg)" });
    prev.addEventListener("click", () => {
      view = new Date(Date.UTC(view.getUTCFullYear(), view.getUTCMonth() - 1, 1));
      draw();
    });
    next.addEventListener("click", () => {
      view = new Date(Date.UTC(view.getUTCFullYear(), view.getUTCMonth() + 1, 1));
      draw();
    });
    root.append(h("div", { class: "cal-head" }, prev, h("span", { class: "cal-title", "aria-live": "polite" }, title), next));
    const grid = h("div", { class: "cal-grid", role: "grid" });
    // The week starts on Monday for languages that do, Sunday for English (United States).
    const mondayFirst = !/^en-US$|^es-MX$/.test(loc) && !/^en-CA$|^pt-BR$/.test(loc);
    const first = mondayFirst ? 1 : 0;
    for (let k = 0; k < 7; k++) {
      const wd = new Date(Date.UTC(2023, 0, 1 + ((first + k) % 7)));
      grid.append(h("span", { class: "cal-dow", "aria-hidden": "true" }, new Intl.DateTimeFormat(loc, { weekday: "narrow", timeZone: "UTC" }).format(wd)));
    }
    const lead = (view.getUTCDay() - first + 7) % 7;
    const days = new Date(Date.UTC(view.getUTCFullYear(), view.getUTCMonth() + 1, 0)).getUTCDate();
    for (let k = 0; k < lead; k++) grid.append(h("span"));
    for (let day = 1; day <= days; day++) {
      const d = new Date(Date.UTC(view.getUTCFullYear(), view.getUTCMonth(), day));
      const iso = toIso(d);
      const b = h("button", { class: "cal-day", type: "button", "data-date": iso, "aria-label": new Intl.DateTimeFormat(loc, { dateStyle: "full", timeZone: "UTC" }).format(d) }, String(day));
      if (toIso(todayUtc) === iso) b.classList.add("today");
      if (chosen && toIso(chosen) === iso) {
        b.classList.add("on");
        b.setAttribute("aria-pressed", "true");
      }
      b.addEventListener("click", () => {
        close();
        opts.pick(iso);
      });
      grid.append(b);
    }
    root.append(grid);
    const foot = h("div", { class: "cal-foot" });
    const todayBtn = h("button", { class: "btn ghost", type: "button" }, t("Today"));
    const clearBtn = h("button", { class: "btn ghost", type: "button" }, t("Clear"));
    todayBtn.addEventListener("click", () => {
      close();
      opts.pick(toIso(todayUtc));
    });
    clearBtn.addEventListener("click", () => {
      close();
      opts.pick("");
    });
    foot.append(todayBtn, clearBtn);
    root.append(foot);
  };
  draw();
  return root;
}
