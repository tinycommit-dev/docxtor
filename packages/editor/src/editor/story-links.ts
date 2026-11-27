// "Link to previous" for the header or footer being edited. A section without a header of its own shows the
// one before it, and editing it edits that one, as in Word. Unlinking gives the section a copy of its own.
import type { Editor } from "./editor";

export interface StoryLink {
  kind: "header" | "footer";
  /** The variant being edited (`default`, `first` or `even`). */
  variant: string;
  /** Index of the section whose page is being edited. */
  section: number;
  /** The first section has no section before it to link to. */
  linkable: boolean;
  /** The section shows the previous section's header or footer. */
  linked: boolean;
}

/** The link state of the header or footer being edited; null elsewhere (the text, the notes). */
export function storyLink(ed: Editor): StoryLink | null {
  const st = ed.story;
  if (!st || (st.zone !== "header" && st.zone !== "footer")) return null;
  const info = ed.pageInfo;
  const section = ed.pageBox(st.page).section;
  const sec = info.sections?.[section];
  if (!sec) return null;
  const variant = st.variant ?? "default";
  const linkable = section > 0;
  const list = (st.zone === "header" ? sec.linkedHeaders : sec.linkedFooters) ?? [];
  return { kind: st.zone, variant, section, linkable, linked: linkable && list.includes(variant) };
}

/** Turn "Link to previous" on or off for the header or footer being edited. Returns whether anything changed. */
export function toggleLinkToPrevious(ed: Editor): boolean {
  const link = storyLink(ed);
  const st = ed.story;
  if (!link || !st || !link.linkable || ed.readOnly) return false;
  const section = ed.sectionAnchor(link.section);
  ed.exitStory(false);
  const done = ed.exec({ op: "link_header_footer", kind: link.kind, variant: link.variant, linked: !link.linked, ...(section !== undefined ? { section } : {}) }, { select: null });
  // Back into the same page: the shared header when linked, the section's own copy when not.
  ed.enterStory(st.zone, st.page);
  return !!done;
}
