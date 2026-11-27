// Content controls, forms and protection
import type { MPos } from "./patches.ts";

/** A content control (`w:sdt`) as the editor needs it. */
export interface RControl {
  /** The control's number among the content controls of its paragraph (document order, outer before inner). */
  n: number;
  /** `text`, `richText`, `picture`, `date`, `dropDown`, `comboBox`, `checkbox`, `repeating`, `repeatingItem`, `group`, `bibliography`, `docPart`, `equation` or `other`. */
  kind: string;
  /** Where the content starts and ends in the paragraph (UTF-16 offsets). Absent offsets (0) for a block-level control. */
  start: number;
  end: number;
  /** The id that addresses a block-level control (or a table row's) in a `control` op. */
  ctl?: number;
  id?: number;
  alias?: string;
  tag?: string;
  color?: string;
  /** `boundingBox` (default), `tags` or `hidden`. */
  appearance?: string;
  /** `sdtLocked` (can't be deleted), `contentLocked` (content can't be edited) or `sdtContentLocked` (both). */
  lock?: string;
  /** The control shows its placeholder text. */
  placeholder?: boolean;
  multiline?: boolean;
  items?: { display: string; value: string }[];
  dateFormat?: string;
  lid?: string;
  calendar?: string;
  fullDate?: string;
  checked?: boolean;
  checkedSymbol?: { val: string; font: string };
  uncheckedSymbol?: { val: string; font: string };
  bound?: boolean;
  temporary?: boolean;
  sectionTitle?: string;
  fixedItems?: boolean;
  gallery?: string;
}

/** What the Properties dialog of a control edits; a field left out stays as it is, an empty alias, tag or colour removes it. */
export interface ControlProps {
  alias?: string;
  tag?: string;
  color?: string;
  appearance?: string;
  lock?: string;
  multiline?: boolean;
  items?: { display: string; value: string }[];
  dateFormat?: string;
  lid?: string;
  calendar?: string;
  checkedSymbol?: { val: string; font: string };
  uncheckedSymbol?: { val: string; font: string };
  placeholder?: string;
  temporary?: boolean;
  sectionTitle?: string;
  fixedItems?: boolean;
}

/** Which content control a `control` op means: control `n` of paragraph `p`, the block-level control `block`, or the row-level control `wrap`. */
export interface ControlRef {
  p?: number;
  n?: number;
  block?: number;
  wrap?: number;
}

/** A legacy form field. */
export interface RForm {
  n: number;
  kind: "text" | "checkbox" | "dropdown";
  /** Where the field starts, and (text) where its result starts and ends. */
  at: number;
  start: number;
  end: number;
  name?: string;
  disabled?: boolean;
  help?: string;
  status?: string;
  textType?: string;
  default?: string;
  maxLength?: number;
  format?: string;
  checked?: boolean;
  /** A check box's size in points; absent when automatic. */
  size?: number;
  defaultChecked?: boolean;
  items?: string[];
  selected?: number;
}

/** What a form field's Properties dialog edits. */
export interface FormProps {
  name?: string;
  disabled?: boolean;
  help?: string;
  status?: string;
  textType?: string;
  default?: string;
  maxLength?: number;
  format?: string;
  size?: number;
  defaultChecked?: boolean;
  items?: string[];
}

/** What `w:documentProtection` and the editable ranges say (see `Engine.protectionInfo`). */
export interface ProtectionInfo {
  /** `none`, `readOnly`, `comments`, `trackedChanges` or `forms`. */
  edit: string;
  enforced: boolean;
  formatting: boolean;
  hasPassword: boolean;
  /** The file only has the old 32-bit hash of the password. */
  legacyHash: boolean;
  markedFinal: boolean;
  /** Styles marked locked: they can't be applied while formatting is limited. */
  lockedStyles: string[];
  ranges: { id: string; group: string; from: MPos; to: MPos }[];
  /** Forms protection: paragraphs in sections that are not protected. */
  openParagraphs: number[];
}

/** What Restrict editing sets. */
export interface ProtectionSettings {
  edit?: string;
  formatting?: boolean;
  enforce?: boolean;
  password?: string;
  /** 16 random bytes in base64, for the password hash. */
  salt?: string;
  allowedStyles?: string[];
}

