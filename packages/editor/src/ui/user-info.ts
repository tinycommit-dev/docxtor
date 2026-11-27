// Tools, User information: the name, initials and address that USERNAME, USERINITIALS and USERADDRESS fields show.
import { t } from "@docxtor/extensions";
import { h, modal } from "./dom";

export interface UserInfo {
  name?: string;
  initials?: string;
  address?: string;
}

export function openUserInfo(opts: { value: UserInfo; save: (value: UserInfo) => void; onClose?: () => void }) {
  const box = (id: string, label: string, value: string | undefined) => {
    const input = h("input", { type: "text", id: `ui-${id}`, value: value ?? "", autocomplete: "off", spellcheck: "false" });
    return { input, field: h("div", { class: "field" }, h("label", { for: `ui-${id}` }, label), input) };
  };
  const name = box("name", t("Name"), opts.value.name);
  const initials = box("initials", t("Initials"), opts.value.initials);
  const address = box("address", t("Address"), opts.value.address);
  const cancel = h("button", { class: "btn ghost", type: "button" }, t("Cancel"));
  const apply = h("button", { class: "btn primary", type: "button" }, t("Apply"));
  const dialog = h(
    "div",
    { class: "dialog", role: "dialog", "aria-modal": "true", "aria-label": t("User information"), style: "width:min(var(--dialog-s),100%)" },
    h("div", { class: "dialog-head" }, h("h2", {}, t("User information")), h("p", { class: "sub" }, t("Fields that show your name, initials or address use these. Update fields to see a change."))),
    h("div", { class: "dialog-body" }, name.field, initials.field, address.field),
    h("div", { class: "dialog-actions" }, cancel, apply),
  );
  const close = modal(dialog, opts.onClose);
  cancel.addEventListener("click", close);
  const go = () => {
    opts.save({ name: name.input.value.trim(), initials: initials.input.value.trim(), address: address.input.value.trim() });
    close();
  };
  apply.addEventListener("click", go);
  for (const i of [name.input, initials.input, address.input]) {
    i.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        go();
      }
    });
  }
}
