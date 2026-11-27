// A filterable list near the top of the window: the command palette, and `ctx.ui.pick` for extensions.
import { t, type PickItem } from "@docxtor/extensions";
import { h, overlayLayer } from "./dom";

let seq = 0;

/** Every word typed must appear in the label or the detail; label matches rank first. */
function rank(item: PickItem, words: string[]): number {
  const label = item.label.toLowerCase();
  const all = `${label} ${(item.detail ?? "").toLowerCase()}`;
  if (!words.every((w) => all.includes(w))) return -1;
  if (!words.length) return 0;
  if (label.startsWith(words[0])) return 3;
  if (label.split(/\s+/).some((w) => w.startsWith(words[0]))) return 2;
  return label.includes(words[0]) ? 1 : 0;
}

export function quickPick<T extends PickItem>(items: T[], opts: { placeholder?: string; label?: string } = {}): Promise<T | null> {
  return new Promise((resolve) => {
    const id = `qp-${++seq}`;
    const input = h("input", {
      type: "text",
      class: "qp-input",
      role: "combobox",
      "aria-expanded": "true",
      "aria-controls": `${id}-list`,
      "aria-autocomplete": "list",
      "aria-label": opts.label ?? opts.placeholder ?? t("Search"),
      placeholder: opts.placeholder ?? t("Type to search"),
      spellcheck: "false",
      autocomplete: "off",
    });
    const list = h("div", { class: "qp-list", role: "listbox", id: `${id}-list` });
    const box = h("div", { class: "qp", role: "dialog", "aria-modal": "true", "aria-label": opts.label ?? t("Search") }, input, list);
    const scrim = h("div", { class: "scrim qp-scrim" }, box);
    const before = document.activeElement as HTMLElement | null;
    let shown: T[] = [];
    let active = 0;
    let done = false;

    const finish = (item: T | null) => {
      if (done) return;
      done = true;
      scrim.remove();
      before?.focus?.({ preventScroll: true });
      resolve(item);
    };

    const paint = () => {
      list.querySelectorAll(".qp-item").forEach((el, i) => {
        el.classList.toggle("on", i === active);
        el.setAttribute("aria-selected", String(i === active));
      });
      const cur = list.children[active] as HTMLElement | undefined;
      if (cur?.classList.contains("qp-item")) {
        input.setAttribute("aria-activedescendant", cur.id);
        cur.scrollIntoView({ block: "nearest" });
      } else input.removeAttribute("aria-activedescendant");
    };

    const filter = () => {
      const words = input.value.toLowerCase().split(/\s+/).filter(Boolean);
      shown = items
        .map((item, i) => ({ item, i, r: rank(item, words) }))
        .filter((x) => x.r >= 0)
        .sort((a, b) => b.r - a.r || a.i - b.i)
        .map((x) => x.item);
      active = 0;
      list.replaceChildren(
        ...(shown.length
          ? shown.map((item, i) => {
              const row = h(
                "div",
                { class: "qp-item", role: "option", id: `${id}-${i}` },
                h("span", { class: "qp-label" }, item.label),
                item.detail ? h("span", { class: "qp-detail" }, item.detail) : null,
                item.hint ? h("span", { class: "qp-hint" }, item.hint) : null,
              );
              row.addEventListener("mousemove", () => {
                if (active === i) return;
                active = i;
                paint();
              });
              row.addEventListener("click", () => finish(item));
              return row;
            })
          : [h("div", { class: "qp-empty", role: "option", "aria-disabled": "true", "aria-selected": "false" }, t("Nothing matches"))]),
      );
      paint();
    };

    input.addEventListener("input", filter);
    input.addEventListener("keydown", (e) => {
      const move = (to: number) => {
        e.preventDefault();
        if (!shown.length) return;
        active = (to + shown.length) % shown.length;
        paint();
      };
      if (e.key === "ArrowDown") move(active + 1);
      else if (e.key === "ArrowUp") move(active - 1);
      else if (e.key === "PageDown") move(Math.min(shown.length - 1, active + 8));
      else if (e.key === "PageUp") move(Math.max(0, active - 8));
      else if (e.key === "Enter" && !e.isComposing) {
        e.preventDefault();
        if (shown[active]) finish(shown[active]);
      } else if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        finish(null);
      } else if (e.key === "Tab") e.preventDefault();
    });
    scrim.addEventListener("mousedown", (e) => {
      if (e.target === scrim) finish(null);
    });
    overlayLayer().append(scrim);
    filter();
    input.focus();
  });
}
