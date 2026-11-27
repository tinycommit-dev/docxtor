// Pictures, mail merge, email and label sheets
import type { UiApi } from "./api.ts";

/** A picture a source brings in: the file's bytes and what to call it. */
export interface PictureResult {
  bytes: Uint8Array;
  /** The file name, with its extension (`logo.png`); it also becomes the picture's alt text unless `alt` is given. */
  name: string;
  /** The media type (`image/png`), where the source knows it. */
  type?: string;
  /** A description for screen readers. */
  alt?: string;
}

/** What a picture source is given when the person picks it. */
export interface PictureSourceContext {
  ui: UiApi;
  /** Aborted when the person closes the dialog or starts something else. */
  signal: AbortSignal;
}

/**
 * A place pictures come from: Insert, Image lists each source ("From your computer", "From a URL", a stock photo
 * site…). `pick` runs when the person chooses it, shows whatever it needs (a file chooser, a dialog with a search
 * box) and resolves with the picture or pictures to insert, or null when the person gave up. Throw an `Error` with a
 * message fit to show when something goes wrong. A source that sends anything off the device (a search, a download
 * from another site) says so in `note`.
 */
export interface PictureSource {
  id: string;
  /** The menu entry, sentence case, ending with an ellipsis when it asks for more: "From a URL…". */
  label: string;
  /** An icon id, for the menu and the command palette. */
  icon?: string;
  /** One plain sentence about what leaves the device. */
  note?: string;
  pick(ctx: PictureSourceContext): Promise<PictureResult | PictureResult[] | null> | PictureResult | PictureResult[] | null;
}

// ---------------------------------------------------------------------------
// Mail merge
// ---------------------------------------------------------------------------

/** A list of recipients a data source brings in: a header row of column names and a record for each recipient. */
export interface RecipientTable {
  /** The column names, one for each value of a record. Empty ones and repeated ones are made usable by the engine. */
  columns: string[];
  /** The records, each with one text value for every column (a short record is padded). */
  rows: string[][];
  /** What to call the list in the interface and in the file (`people.csv`, `Members sheet`). Default: the source's label. */
  name?: string;
  /** The sheet or table the list came from, when there is more than one. */
  table?: string;
  /**
   * `csv` or `xlsx` when the list is a file of that kind: the document then links to it the way Word does (`w:mailMerge` names the file).
   * Anything else (the default is the source's id) is kept in the document only.
   */
  kind?: string;
  /** The separator of a `csv` file. */
  delimiter?: string;
}

/** What a data source is given when the person picks it. */
export interface MailMergeSourceContext {
  ui: UiApi;
  /** Aborted when the person closes the dialog or starts something else. */
  signal: AbortSignal;
}

/**
 * A place a list of recipients comes from. Mailings, Select recipients lists each source ("From a CSV file…", "From an Excel file…", and
 * whatever extensions add: a Google Sheet, a CRM, a mailing list). `pick` runs when the person chooses it, shows whatever it needs (a
 * file chooser, a dialog with a sheet's address) and resolves with the list, or null when the person gave up. Throw an `Error` with a
 * message fit to show when something goes wrong. The list is kept in the document, so the merge works without the source afterwards. A
 * source that sends anything off the device says so in `note`.
 */
export interface MailMergeDataSource {
  id: string;
  /** The menu entry, sentence case, ending with an ellipsis when it asks for more: "From a Google Sheet…". */
  label: string;
  /** An icon id, for the menu and the command palette. */
  icon?: string;
  /** One plain sentence about what leaves the device. */
  note?: string;
  pick(ctx: MailMergeSourceContext): Promise<RecipientTable | null> | RecipientTable | null;
}

/** A file sent with an email. */
export interface EmailAttachment {
  /** The file name the receiver sees, with its extension. */
  name: string;
  /** The media type: `application/pdf`. */
  type: string;
  bytes: Uint8Array;
}

/** How the body of each merged message is made: the merged document as a web page, as plain text, or sent as a file. */
export type EmailFormat = "html" | "text" | "docx" | "pdf";

/** One merged message, ready to send. Every field is plain data, so a sender can pass it on to a mail service as it is. */
export interface EmailMessage {
  /** The recipient's address. */
  to: string;
  /** The subject, merge fields filled in. */
  subject: string;
  /** The body as a web page (the merged document), when the format is HTML. */
  html?: string;
  /** The body as plain text. With HTML both are given, for mail apps that show text only. */
  text?: string;
  /** The merged document as a file, when the format is an attachment. */
  attachments?: EmailAttachment[];
  /** The position of the recipient's record in the merge, from 0. */
  record: number;
}

/** What one message came to. */
export type EmailOutcome = { ok: true } | { ok: false; error: string };

/** What a sender is given besides the messages. */
export interface EmailSendContext {
  ui: UiApi;
  /** Aborted when the person stops the sending. */
  signal: AbortSignal;
  /** The document's name without its extension, for a sender that makes a file. */
  name: string;
  /** Say how message `index` (its place in the list given to `send`) went. Call it once for each message as it is done. */
  progress(index: number, outcome: EmailOutcome): void;
}

/**
 * A way to send the merged messages. Mailings, Finish and merge, Send email messages lists each sender. `send` gets every message at once and
 * reports each one with `ctx.progress` as it goes: a message that was not reported when `send` resolves counts as sent, and when `send` throws
 * the unreported ones count as failed with the error's message. Docxtor's own senders need no server: one saves a file for every message
 * (a .zip of .eml files any mail app opens) and one opens the person's mail app, a message at a time. A sender that goes through a mail
 * service (SMTP, Gmail, Outlook) is its own extension that says what it sends in `note`.
 */
export interface EmailSender {
  id: string;
  /** The line in the sender list, sentence case: "Save as email files". */
  label: string;
  /** An icon id. */
  icon?: string;
  /** One plain sentence about where the messages go. */
  note?: string;
  /** The formats it can send; default all. A format it can't send is changed to the first one it can. */
  formats?: EmailFormat[];
  /** Above this many messages the dialog warns first, with `limitNote`. */
  warnAbove?: number;
  /** What the dialog says above `warnAbove` messages. */
  limitNote?: string;
  /** The sentence shown when every message went through, given how many did ("12 email files saved"). Default: "{n} messages sent." */
  summary?(sent: number): string;
  send(messages: EmailMessage[], ctx: EmailSendContext): Promise<void> | void;
}

/**
 * A kind of label sheet: where the labels are on the page. The Labels dialog lists products by vendor. Sizes are in `unit`
 * (inches by default), measured as the sheet's maker gives them.
 */
export interface LabelProduct {
  id: string;
  /** The maker, as the vendor list shows it: "Avery US Letter", "Herma". */
  vendor: string;
  /** The product's name: "5160 Address labels". */
  name: string;
  /** `in` (default), `mm` or `pt`. */
  unit?: "in" | "mm" | "pt";
  pageWidth: number;
  pageHeight: number;
  /** From the top of the page to the top of the first label. */
  top: number;
  /** From the left of the page to the left of the first label. */
  left: number;
  labelWidth: number;
  labelHeight: number;
  /** From the left of one label to the left of the next (the label's width plus the gap). */
  pitchH: number;
  /** From the top of one label to the top of the next. */
  pitchV: number;
  /** Labels across and down. */
  across: number;
  down: number;
}
