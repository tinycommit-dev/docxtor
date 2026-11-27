// The Arrange menu of the strip over a selected shape, picture or chart: where it sits in the stack of objects,
// behind or in front of the text, and group and ungroup for shapes. The same lines are commands in the menu bar
// (see builtins/arrange.ts).
import { t } from "@docxtor/extensions";
import { ARRANGE_ICONS } from "./arrange-icons";
import { h, popover, svg } from "./dom";

/**
 * The object behind the text at a click, if the click is on page where there is no text, picture or table to take
 * it (Word selects an object behind the text that way). Alt always picks the one behind. The topmost wins.
 */
export function behindAt(root: HTMLElement, e: MouseEvent): HTMLElement | null {
  if (!root.querySelector(".atom.wrap-behind")) return null;
  let content = false;
  for (const el of document.elementsFromPoint(e.clientX, e.clientY)) {
    if (!root.contains(el)) continue;
    const atom = el.closest<HTMLElement>(".atom.shape, .atom.img");
    if (atom?.classList.contains("wrap-behind")) return content && !e.altKey ? null : atom;
    // Text, pictures and tables over the object get the click; the empty page between them lets it through.
    if (el.matches(".s, .lbl, .atom, td, th, img")) content = true;
  }
  return null;
}

export type StackOrder = "front" | "back" | "forward" | "backward";

export interface ArrangeActions {
  /** The object floats over the text; one in line with the text has no place in the stack. */
  floating: boolean;
  /** The object is behind the text now. */
  behind: boolean;
  order(how: StackOrder): void;
  setBehind(behind: boolean): void;
  /** Present when the selection can be grouped or taken apart. */
  group?: () => void;
  ungroup?: () => void;
}

/** Open the Arrange menu at a button. */
export function openArrangeMenu(anchor: HTMLElement, a: ArrangeActions): () => void {
  const list = h("div", { class: "arrange-menu" });
  let close = () => {};
  const item = (label: string, icon: string, run: () => void, disabled = false, on = false) => {
    const b = h("button", { class: "menu-item", type: "button", role: "menuitem", disabled }, h("span", { class: `menu-icon${on ? " on" : ""}`, "aria-hidden": "true" }, svg(icon)), h("span", { class: "menu-label" }, label));
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", () => {
      close();
      run();
    });
    list.append(b);
  };
  const sep = () => list.append(h("div", { class: "menu-sep", role: "separator" }));
  const stack = a.floating;
  item(t("Bring to front"), ARRANGE_ICONS.bringToFront, () => a.order("front"), !stack);
  item(t("Bring forward"), ARRANGE_ICONS.bringForward, () => a.order("forward"), !stack);
  item(t("Send backward"), ARRANGE_ICONS.sendBackward, () => a.order("backward"), !stack);
  item(t("Send to back"), ARRANGE_ICONS.sendToBack, () => a.order("back"), !stack);
  sep();
  item(t("Behind text"), ARRANGE_ICONS.behindText, () => a.setBehind(true), false, stack && a.behind);
  item(t("In front of text"), ARRANGE_ICONS.inFrontOfText, () => a.setBehind(false), false, stack && !a.behind);
  if (a.group || a.ungroup) {
    sep();
    if (a.group) item(t("Group"), ARRANGE_ICONS.group, a.group);
    if (a.ungroup) item(t("Ungroup"), ARRANGE_ICONS.ungroup, a.ungroup);
  }
  close = popover(anchor, list);
  return close;
}
