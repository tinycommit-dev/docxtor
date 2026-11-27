// Insert, Date and time: the formats Word offers in the document's language, each shown with today's date or time. "Update
// automatically" inserts a field with the format as its picture (a time-only format makes a TIME field, the rest a DATE
// field); without it the text goes in as ordinary text.
import { t } from "@docxtor/extensions";
import type { Editor } from "../editor/editor";
import { localIso } from "../editor/fields";
import { h, modal, onChoose } from "./dom";

let seq = 0;

/** Whether a picture shows no day, month or year: then it is a time. Quoted text in the picture doesn't count. */
export function isTimePicture(picture: string): boolean {
  return !/[dMy]/.test(picture.replace(/'[^']*'/g, ""));
}

/** The instruction of the field a format makes. */
export function dateTimeCode(picture: string): string {
  return `${isTimePicture(picture) ? "TIME" : "DATE"} \\@ "${picture.replace(/"/g, "'")}"`;
}

export function openDateTime(ed: Editor) {
  const id = `dt-${++seq}`;
  const formats = ed.engine.dateFormats(localIso());
  let active = 0;
  const lang = ed.engine.pageSetup().lang;

  const list = h("div", { class: "xr-list dt-list", role: "listbox", id: `${id}-list`, "aria-label": t("Available formats"), tabindex: "0" });
  const auto = h("input", { type: "checkbox", id: `${id}-auto` });
  const insert = h("button", { class: "btn primary", type: "button" }, t("Insert"));
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));

  const paint = () => {
    list.replaceChildren(
      ...formats.map(([, sample], i) => {
        const b = h("button", { type: "button", role: "option", class: `xr-item${i === active ? " on" : ""}`, "aria-selected": String(i === active), tabindex: "-1" }, h("span", { class: "xr-title" }, sample));
        b.addEventListener("mousedown", (e) => e.preventDefault());
        onChoose(
          b,
          () => {
            active = i;
            select();
          },
          go,
        );
        return b;
      }),
    );
  };
  const select = () => {
    list.querySelectorAll<HTMLElement>(".xr-item").forEach((el, i) => {
      el.classList.toggle("on", i === active);
      el.setAttribute("aria-selected", String(i === active));
    });
    list.querySelector<HTMLElement>(".xr-item.on")?.scrollIntoView({ block: "nearest" });
  };
  list.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      active = Math.min(formats.length - 1, Math.max(0, active + (e.key === "ArrowDown" ? 1 : -1)));
      select();
    } else if (e.key === "Enter") {
      e.preventDefault();
      go();
    }
  });

  const dialog = h(
    "div",
    { class: "dialog dt-dialog", role: "dialog", "aria-modal": "true", "aria-label": t("Date and time") },
    h("div", { class: "dialog-head" }, h("h2", {}, t("Date and time")), h("p", { class: "sub" }, lang ? t("Formats for the document's language, {lang}.", { lang }) : t("Formats for the document's language."))),
    h("div", { class: "dialog-body" }, h("div", { class: "field" }, h("label", {}, t("Available formats")), list), h("label", { class: "check", for: `${id}-auto` }, auto, h("span", {}, t("Update automatically")))),
    h("div", { class: "dialog-actions" }, cancel, insert),
  );
  const dismiss = modal(dialog, () => ed.focus());
  const go = () => {
    const f = formats[active];
    if (!f) return;
    const [picture, sample] = f;
    dismiss();
    if (auto.checked) ed.insertField(dateTimeCode(picture));
    else ed.insertText(sample);
  };
  cancel.addEventListener("click", dismiss);
  insert.addEventListener("click", go);
  paint();
  list.focus();
}
