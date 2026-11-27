// Docxtor's own field types, the same kind of thing an extension registers with `ctx.fields.register`: each has a name, a
// group, a sentence about it and the form Insert, Field shows. The engine works out their results.
import { msg, type FieldOption, type FieldType } from "@docxtor/extensions";

const choice = (value: string, label: string) => ({ value, label });

/** Word's text formats (`\* Upper`…). */
const textFormat: FieldOption = {
  id: "case",
  label: msg("Text format"),
  kind: "choice",
  write: "format",
  choices: [choice("", msg("None")), choice("Upper", msg("Uppercase")), choice("Lower", msg("Lowercase")), choice("FirstCap", msg("First capital")), choice("Caps", msg("Title case"))],
};

/** Word's number formats (`\* roman`…). */
const numberFormat: FieldOption = {
  id: "numberFormat",
  label: msg("Number format"),
  kind: "choice",
  write: "format",
  choices: [
    choice("", msg("None")),
    choice("Arabic", msg("1, 2, 3")),
    choice("alphabetic", msg("a, b, c")),
    choice("ALPHABETIC", msg("A, B, C")),
    choice("roman", msg("i, ii, iii")),
    choice("ROMAN", msg("I, II, III")),
    choice("CardText", msg("One, two, three")),
    choice("OrdText", msg("First, second, third")),
    choice("Ordinal", msg("1st, 2nd, 3rd")),
  ],
};

const keep: FieldOption = { id: "keep", label: msg("Keep formatting when the field is updated"), kind: "toggle", write: "format", value: "MERGEFORMAT", default: true };

const picture = (id: string, fallback: string): FieldOption => ({ id, label: msg("Date and time format"), kind: "text", write: "date", default: fallback });
const numberPicture: FieldOption = { id: "numberPicture", label: msg("Number picture"), kind: "text", write: "number", placeholder: "#,##0.00", help: msg("0 is a digit that always shows, # one that shows when there is a number there, and the rest is copied.") };
const text = (id: string, label: string, extra: Partial<FieldOption> = {}): FieldOption => ({ id, label, kind: "text", write: "arg", ...extra });
const toggle = (id: string, label: string, sw: string): FieldOption => ({ id, label, kind: "toggle", write: "switch", switch: sw });

const DATE = "Date and time";
const INFO = "Document information";
const NUMBERING = "Numbering";
const LINKS = "Links and references";
const MATH = "Equations and formulas";
const USER = "User information";
const AUTO = "Document automation";

/** The properties DOCPROPERTY knows by name. */
export const PROPERTY_NAMES = ["Author", "Title", "Subject", "Company", "Manager", "Category", "Comments", "Keywords", "Status", "LastSavedBy", "CreateTime", "LastSavedTime", "Template", "Application"];

const info = (name: string, title: string, description: string, argLabel?: string): FieldType => ({
  name,
  title,
  category: INFO,
  description,
  options: [...(argLabel ? [text("value", argLabel, { help: msg("Leave this empty to use the document's own.") })] : []), textFormat, keep],
});

export const FIELD_TYPES: FieldType[] = [
  { name: "DATE", title: msg("Date"), category: DATE, description: msg("Today's date, brought up to date when the document opens."), options: [picture("picture", "MMMM d, yyyy"), keep] },
  { name: "TIME", title: msg("Time"), category: DATE, description: msg("The time now, brought up to date when the document opens."), options: [picture("picture", "h:mm am/pm"), keep] },
  { name: "CREATEDATE", title: msg("Date created"), category: DATE, description: msg("When the document was made."), options: [picture("picture", "MMMM d, yyyy h:mm am/pm"), keep] },
  { name: "SAVEDATE", title: msg("Date saved"), category: DATE, description: msg("When the document was last saved."), options: [picture("picture", "MMMM d, yyyy h:mm am/pm"), keep] },
  { name: "PRINTDATE", title: msg("Date printed"), category: DATE, description: msg("When the document was last printed."), options: [picture("picture", "MMMM d, yyyy h:mm am/pm"), keep] },
  { name: "EDITTIME", title: msg("Editing time"), category: DATE, description: msg("The minutes the document has been open for editing."), options: [numberPicture, keep] },

  info("AUTHOR", msg("Author"), msg("The author in the document's details."), msg("Author")),
  info("LASTSAVEDBY", msg("Last saved by"), msg("Who saved the document last.")),
  info("TITLE", msg("Title"), msg("The title in the document's details."), msg("Title")),
  info("SUBJECT", msg("Subject"), msg("The subject in the document's details."), msg("Subject")),
  info("KEYWORDS", msg("Keywords"), msg("The keywords in the document's details."), msg("Keywords")),
  info("COMMENTS", msg("Comments"), msg("The description in the document's details."), msg("Comments")),
  { name: "FILENAME", title: msg("File name"), category: INFO, description: msg("The name of the file."), options: [toggle("path", msg("Include the folder"), "p"), textFormat, keep] },
  { name: "FILESIZE", title: msg("File size"), category: INFO, description: msg("How big the file is."), options: [toggle("kilobytes", msg("In kilobytes"), "k"), toggle("megabytes", msg("In megabytes"), "m"), numberPicture, keep] },
  { name: "NUMWORDS", title: msg("Number of words"), category: INFO, description: msg("How many words the document has."), options: [numberPicture, numberFormat, keep] },
  { name: "NUMCHARS", title: msg("Number of characters"), category: INFO, description: msg("How many characters, without spaces, the document has."), options: [numberPicture, numberFormat, keep] },
  { name: "NUMPAGES", title: msg("Number of pages"), category: INFO, description: msg("How many pages the document has."), options: [numberFormat, keep] },
  {
    name: "DOCPROPERTY",
    title: msg("Document property"),
    category: INFO,
    description: msg("A property of the document, such as its author, company or one you made yourself."),
    options: [{ id: "property", label: msg("Property"), kind: "choice", write: "arg", choices: PROPERTY_NAMES.map((p) => choice(p, p)), default: "Author" }, textFormat, keep],
  },
  { name: "DOCVARIABLE", title: msg("Document variable"), category: INFO, description: msg("A named value kept in the document."), options: [text("name", msg("Variable name"))] },
  { name: "TEMPLATE", title: msg("Template"), category: INFO, description: msg("The template the document is based on."), options: [textFormat, keep] },
  { name: "REVNUM", title: msg("Revision number"), category: INFO, description: msg("How many times the document has been saved."), options: [numberFormat, keep] },

  { name: "PAGE", title: msg("Page number"), category: NUMBERING, description: msg("The number of the page the field is on."), options: [numberFormat, keep] },
  { name: "SECTION", title: msg("Section number"), category: NUMBERING, description: msg("The number of the section the field is in."), options: [numberFormat, keep] },
  { name: "SECTIONPAGES", title: msg("Pages in the section"), category: NUMBERING, description: msg("How many pages the section has."), options: [numberFormat, keep] },
  {
    name: "SEQ",
    title: msg("Sequence"),
    category: NUMBERING,
    description: msg("A running number, such as the 3 in Figure 3. Fields with the same name count together."),
    options: [text("name", msg("Sequence name"), { default: "Figure" }), toggle("repeat", msg("Repeat the last number"), "c"), toggle("hide", msg("Hide the result"), "h"), { id: "restart", label: msg("Restart at"), kind: "text", write: "switch-value", switch: "r" }, numberFormat],
  },

  {
    name: "HYPERLINK",
    title: msg("Link"),
    category: LINKS,
    description: msg("A link to a web page, a file or a place in the document."),
    options: [text("url", msg("Address")), { id: "place", label: msg("Place in the document"), kind: "text", write: "switch-value", switch: "l", placeholder: msg("A bookmark") }, { id: "tip", label: msg("Tooltip"), kind: "text", write: "switch-value", switch: "o" }],
  },
  {
    name: "REF",
    title: msg("Cross-reference"),
    category: LINKS,
    description: msg("The text of a bookmark, or its number or place."),
    options: [text("bookmark", msg("Bookmark")), toggle("link", msg("Make it a link"), "h"), toggle("position", msg("Say whether it is above or below"), "p"), toggle("number", msg("Show its paragraph number"), "n"), toggle("full", msg("Show its full number"), "w"), textFormat, keep],
  },
  { name: "PAGEREF", title: msg("Page of a bookmark"), category: LINKS, description: msg("The page a bookmark is on."), options: [text("bookmark", msg("Bookmark")), toggle("link", msg("Make it a link"), "h"), toggle("position", msg("Say whether it is above or below"), "p"), numberFormat] },
  { name: "NOTEREF", title: msg("Note number"), category: LINKS, description: msg("The number of the footnote or endnote at a bookmark."), options: [text("bookmark", msg("Bookmark")), toggle("link", msg("Make it a link"), "h"), toggle("format", msg("Same look as the note mark"), "f"), toggle("position", msg("Say whether it is above or below"), "p")] },
  {
    name: "STYLEREF",
    title: msg("Paragraph with a style"),
    category: LINKS,
    description: msg("The text of the nearest paragraph with a style, such as the heading of the page. Use it in a header or footer."),
    options: [text("style", msg("Style name"), { default: "Heading 1" }), toggle("last", msg("The last one on the page"), "l"), toggle("number", msg("Show its paragraph number"), "n"), toggle("full", msg("Show its full number"), "w"), toggle("position", msg("Say whether it is above or below"), "p"), textFormat, keep],
  },
  {
    name: "=",
    title: msg("Formula"),
    category: MATH,
    description: msg("A calculation. Use numbers, + - * / ^ %, functions such as SUM, AVERAGE and ROUND, and in a table ABOVE, LEFT, RIGHT, BELOW or cells such as A1."),
    options: [{ id: "expression", label: msg("Formula"), kind: "text", write: "raw", placeholder: "SUM(ABOVE)" }, numberPicture],
    example: "= SUM(ABOVE)",
  },
  { name: "EQ", title: msg("Equation field"), category: MATH, description: msg("An equation written with switches, such as \\f(1,2) for a fraction or \\r(3,x) for a root."), options: [{ id: "expression", label: msg("Equation"), kind: "text", write: "raw", placeholder: "\\f(1,2)" }] },
  {
    name: "SYMBOL",
    title: msg("Symbol"),
    category: MATH,
    description: msg("A character by its number, in a font and size."),
    options: [text("code", msg("Character number"), { placeholder: "169" }), { id: "font", label: msg("Font"), kind: "text", write: "switch-value", switch: "f", placeholder: "Wingdings" }, { id: "size", label: msg("Size in points"), kind: "text", write: "switch-value", switch: "s" }],
  },

  { name: "USERNAME", title: msg("User name"), category: USER, description: msg("The name set under Tools, User information."), options: [text("value", msg("Name"), { help: msg("Leave this empty to use yours.") }), textFormat] },
  { name: "USERINITIALS", title: msg("User initials"), category: USER, description: msg("The initials set under Tools, User information."), options: [text("value", msg("Initials"), { help: msg("Leave this empty to use yours.") }), textFormat] },
  { name: "USERADDRESS", title: msg("User address"), category: USER, description: msg("The address set under Tools, User information."), options: [text("value", msg("Address"), { help: msg("Leave this empty to use yours.") })] },

  { name: "IF", title: msg("If"), category: AUTO, description: msg("Shows one text or another, depending on a comparison. Write it as: 3 > 2 \"yes\" \"no\"."), options: [{ id: "condition", label: msg("Condition and results"), kind: "text", write: "raw", placeholder: '3 > 2 "yes" "no"' }] },
  { name: "COMPARE", title: msg("Compare"), category: AUTO, description: msg("1 when a comparison is true, 0 when it is not. Write it as: 3 > 2."), options: [{ id: "condition", label: msg("Comparison"), kind: "text", write: "raw", placeholder: "3 > 2" }] },
  { name: "SET", title: msg("Set a value"), category: AUTO, description: msg("Gives a name a value that a reference field can show elsewhere. The field itself shows nothing."), options: [text("name", msg("Name")), text("value", msg("Value"))] },
  { name: "QUOTE", title: msg("Quote"), category: AUTO, description: msg("Shows the text you give it."), options: [text("text", msg("Text")), textFormat] },
];
