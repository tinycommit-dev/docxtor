//! The tool definitions the model is shown: name, description and input schema of each tool.

use serde_json::{json, Value};

pub(super) fn fmt_schema() -> Value {
    json!({
        "type": "object",
        "description": "Character formatting. Use false / \"none\" to remove.",
        "properties": {
            "bold": {"type": "boolean"}, "italic": {"type": "boolean"},
            "underline": {"type": "string", "description": "single, words, double, thick, dotted, dash, dotDash, dotDotDash, wave, none, and Word's heavy and long variants (dottedHeavy, dashedHeavy, dashLong, wavyHeavy, wavyDouble, ...)"},
            "underlineColor": {"type": "string", "description": "hex, or auto for the text colour"},
            "strike": {"type": "boolean"}, "dstrike": {"type": "boolean", "description": "double strikethrough"},
            "caps": {"type": "boolean"}, "smallCaps": {"type": "boolean"},
            "vanish": {"type": "boolean", "description": "hidden text"},
            "noProof": {"type": "boolean", "description": "do not check spelling or grammar (code, names, quotations in other languages)"},
            "outline": {"type": "boolean"}, "shadow": {"type": "boolean"}, "emboss": {"type": "boolean"}, "imprint": {"type": "boolean", "description": "engrave"},
            "color": {"type": "string", "description": "hex like 1F4E79, or auto"},
            "themeColor": {"type": "string", "description": "a theme colour (accent1, text2, ...) written beside `color`; themeTint / themeShade are hex 00 to FF"},
            "highlight": {"type": "string", "description": "yellow, green, cyan, magenta, blue, red, darkBlue, darkCyan, darkGreen, darkMagenta, darkRed, darkYellow, darkGray, lightGray, black, white, none"},
            "shading": {"type": "string", "description": "background colour of the text, hex or none"},
            "size": {"type": "number", "description": "points"},
            "font": {"type": "string"},
            "vertAlign": {"type": "string", "enum": ["superscript", "subscript", "baseline"]},
            "spacing": {"type": "integer", "description": "expanded (+) or condensed (-) by this many twentieths of a point"},
            "position": {"type": "integer", "description": "raised (+) or lowered (-) by this many half-points"},
            "scale": {"type": "integer", "description": "character width in percent, 100 is normal"},
            "kern": {"type": "integer", "description": "kern text of this size and above, in half-points; 0 is off"},
            "em": {"type": "string", "enum": ["none", "dot", "comma", "circle", "underDot"], "description": "emphasis mark"},
            "style": {"type": "string", "description": "a character style by name or id (Strong, Emphasis, ...); a linked paragraph style applies as its character style; empty clears"},
            "ligatures": {"type": "string", "description": "none, standard, contextual, historical, discretional, standardContextual, all, default"},
            "numForm": {"type": "string", "enum": ["default", "lining", "oldStyle"]},
            "numSpacing": {"type": "string", "enum": ["default", "proportional", "tabular"]}
        }
    })
}

/// Tool definitions in a provider-neutral shape: `{name, description, input_schema}`.
/// Tool definitions in a provider-neutral shape: `{name, description, input_schema}`.
pub fn tool_definitions() -> Value {
    let mut v = base_tool_definitions();
    if let Value::Array(a) = &mut v {
        a.extend(crate::ai_refs::definitions());
        a.extend(crate::ai_lists::definitions());
        a.extend(crate::shapes::ai::definitions());
        a.extend(crate::math::tool_definitions());
        a.extend(crate::ai_charts::definitions());
        a.extend(crate::ai_design::definitions());
        a.extend(crate::ai_merge::definitions());
        a.extend(crate::ai_outline::definitions());
        a.extend(crate::ai_review::definitions());
        a.extend(crate::ai_forms::definitions());
        a.extend(crate::ai_east_asian::definitions());
    }
    v
}

pub(super) fn base_tool_definitions() -> Value {
    json!([
        {
            "name": "read_document",
            "description": "Read the document as DocScript (ids, styles, inline Markdown). Long documents are windowed: pass `start` (a block id like \"p120\") to continue.",
            "input_schema": {"type": "object", "properties": {
                "start": {"type": "string", "description": "Block id to start from"},
                "limit": {"type": "integer", "description": "Max top-level blocks (default 400)"}
            }}
        },
        {
            "name": "get_outline",
            "description": "List the document's headings with their paragraph ids. A document with tabs also lists them, each with its id, in order, with the headings under the tab they are in.",
            "input_schema": {"type": "object", "properties": {}}
        },
        {
            "name": "search",
            "description": "Find paragraphs containing text, in the text and in headers, footers, notes and text boxes. Returns the matching paragraphs as DocScript. The query is plain text in which ^p is a paragraph mark, ^t a tab, ^l a line break, ^m a page break, ^? any character, ^# any digit, ^$ any letter and ^^ a caret; with `wildcards` it is Word's wildcard syntax (? * [a-z] [!x] {n,m} @ < > and (groups)); with `regex` a JavaScript regular expression.",
            "input_schema": {"type": "object", "required": ["query"], "properties": {
                "query": {"type": "string"},
                "ignore_case": {"type": "boolean"},
                "whole_word": {"type": "boolean", "description": "Only whole words"},
                "wildcards": {"type": "boolean"},
                "regex": {"type": "boolean"},
                "prefix": {"type": "boolean", "description": "Only at the start of a word"},
                "suffix": {"type": "boolean", "description": "Only at the end of a word"},
                "ignore_punct": {"type": "boolean"},
                "ignore_space": {"type": "boolean"},
                "ignore_marks": {"type": "boolean", "description": "Letters with accents match plain letters"},
                "sounds_like": {"type": "boolean"},
                "main_only": {"type": "boolean", "description": "Leave out headers, footers, notes and text boxes"},
                "find_format": {"type": "object", "description": "Only text with this formatting: bold, italic, underline, strike, caps, smallCaps (booleans), font, size (points), color (hex), highlight, vertAlign, style (a character style id), lang, paraStyle, align. With an empty query, all text that has it."}
            }}
        },
        {
            "name": "replace_text",
            "description": "Replace text (like str_replace). Keeps the formatting of the replaced text. Fails if not found. Restrict to paragraphs with `ids`; set `all` to replace every occurrence. Without `ids` it also replaces in headers, footers, notes and text boxes (set `main_only` to leave them). Special characters work as in `search`: ^p in `find` joins paragraphs, ^p in `replace` splits them, ^t is a tab, ^l a line break, ^m a page break, ^& the found text. With `wildcards`, \\1 in `replace` is the first (group); with `regex`, $1 is. `ignore_case` with `keep_case` gives the replacement the capitals of what it replaces. `format` (bold, italic, underline, strike, caps, smallCaps, font, size, color, highlight, vertAlign, style) and `para_format` (style, align, ...) are put on the replacement; with an empty `replace` they change the matched text and leave its words alone.",
            "input_schema": {"type": "object", "required": ["find"], "properties": {
                "find": {"type": "string", "description": "Text to find (no Markdown); empty with find_format to find all text that has the formatting"},
                "replace": {"type": "string", "description": "Plain replacement text"},
                "ids": {"type": "array", "items": {"type": "string"}},
                "all": {"type": "boolean"},
                "ignore_case": {"type": "boolean"},
                "keep_case": {"type": "boolean"},
                "whole_word": {"type": "boolean"},
                "wildcards": {"type": "boolean"},
                "regex": {"type": "boolean"},
                "prefix": {"type": "boolean"},
                "suffix": {"type": "boolean"},
                "ignore_punct": {"type": "boolean"},
                "ignore_space": {"type": "boolean"},
                "ignore_marks": {"type": "boolean"},
                "sounds_like": {"type": "boolean"},
                "main_only": {"type": "boolean"},
                "find_format": {"type": "object", "description": "Only text with this formatting (see search)"},
                "format": {"type": "object", "description": "Character formatting to put on the replacement"},
                "para_format": {"type": "object", "description": "Paragraph formatting to put on the paragraphs of the replacement"},
                "language": {"type": "string", "description": "A language tag such as en-GB to put on the replacement"}
            }}
        },
        {
            "name": "rewrite_paragraph",
            "description": "Replace one paragraph's content with inline Markdown, keeping its style (heading, list, alignment). Objects like images in it are removed, so prefer replace_text for those.",
            "input_schema": {"type": "object", "required": ["id", "markdown"], "properties": {
                "id": {"type": "string"}, "markdown": {"type": "string"}
            }}
        },
        {
            "name": "insert_content",
            "description": "Insert new content written in Markdown after (or before) a block. Supports # headings, - bullets, 1. numbered items (indent 2 spaces per level), - [ ] / - [x] checklist items, > quotes, ``` code, | tables |, --- page breaks and inline formatting. Without after/before, appends at the end.",
            "input_schema": {"type": "object", "required": ["markdown"], "properties": {
                "after": {"type": "string"}, "before": {"type": "string"}, "markdown": {"type": "string"}
            }}
        },
        {
            "name": "insert_html",
            "description": "Insert content written as HTML after (or before) a block, keeping what HTML can say that Markdown cannot: fonts, sizes, colours, highlights, alignment, indents, spacing, lists, tables with widths, borders and shading, links and pictures given as data: addresses. Reads the HTML Word, Google Docs and web pages put on the clipboard. Without after/before, appends at the end.",
            "input_schema": {"type": "object", "required": ["html"], "properties": {
                "after": {"type": "string"}, "before": {"type": "string"}, "html": {"type": "string"}
            }}
        },
        {
            "name": "delete_blocks",
            "description": "Delete paragraphs or tables by id.",
            "input_schema": {"type": "object", "required": ["ids"], "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}
            }}
        },
        {
            "name": "format_text",
            "description": "Apply character formatting to text inside a paragraph, matched by its exact plain text (or the whole paragraph if `text` is omitted).",
            "input_schema": {"type": "object", "required": ["id", "format"], "properties": {
                "id": {"type": "string"},
                "text": {"type": "string", "description": "Exact text to format (first occurrence unless all=true)"},
                "all": {"type": "boolean"},
                "format": fmt_schema()
            }}
        },
        {
            "name": "change_case",
            "description": "Change the case of text in a paragraph, keeping its formatting: sentence (first letter of each sentence in capitals), lower, upper, title (each word starts with a capital) or toggle. Casing follows the paragraph's language (Turkish has a dotted and a dotless i).",
            "input_schema": {"type": "object", "required": ["id", "mode"], "properties": {
                "id": {"type": "string"},
                "text": {"type": "string", "description": "Exact text to change (first occurrence unless all=true); omit for the whole paragraph"},
                "all": {"type": "boolean"},
                "mode": {"type": "string", "enum": ["sentence", "lower", "upper", "title", "toggle"]}
            }}
        },
        {
            "name": "set_link",
            "description": "Turn text in a paragraph into a hyperlink (or remove the link when url is empty).",
            "input_schema": {"type": "object", "required": ["id", "text"], "properties": {
                "id": {"type": "string"}, "text": {"type": "string"}, "url": {"type": "string"}
            }}
        },
        {
            "name": "set_paragraph_style",
            "description": "Set the style of paragraphs, e.g. Title, Subtitle, Heading 1..6, Normal, Quote, Code, List Paragraph, or any style in list_styles.",
            "input_schema": {"type": "object", "required": ["ids", "style"], "properties": {
                "ids": {"type": "array", "items": {"type": "string"}}, "style": {"type": "string"}
            }}
        },
        {
            "name": "format_paragraphs",
            "description": "Paragraph layout: alignment (left, center, right, both, distribute), spacing in points, line spacing, indents in points, tab stops, text direction, outline level, lists, checklists (list_style \"checklist\"; checked ticks and strikes the item).",
            "input_schema": {"type": "object", "required": ["ids"], "properties": {
                "ids": {"type": "array", "items": {"type": "string"}},
                "align": {"type": "string", "enum": ["left", "center", "right", "both", "distribute"], "description": "In a right-to-left paragraph left is the start side (the right), as in Word's file"},
                "space_before_pt": {"type": "number"}, "space_after_pt": {"type": "number"},
                "line_spacing": {"type": "number", "description": "1, 1.15, 1.5, 2 (any multiple of a line)"},
                "line_spacing_exactly_pt": {"type": "number", "description": "Exact line height in points"},
                "line_spacing_at_least_pt": {"type": "number", "description": "Minimum line height in points"},
                "indent_left_pt": {"type": "number"}, "indent_right_pt": {"type": "number"}, "first_line_pt": {"type": "number", "description": "negative = hanging"},
                "mirror_indents": {"type": "boolean", "description": "Left and right indents become inside and outside indents"},
                "dont_add_space_between_same_style": {"type": "boolean"},
                "dont_hyphenate": {"type": "boolean"},
                "direction": {"type": "string", "enum": ["ltr", "rtl"], "description": "Text direction of the paragraph"},
                "outline_level": {"type": "integer", "description": "0 is body text, 1 to 9 are the outline levels"},
                "tab_stops": {"type": "array", "description": "Replaces the paragraph's tab stops (an empty list clears them)", "items": {"type": "object", "required": ["position_pt"], "properties": {
                    "position_pt": {"type": "number", "description": "From the left margin"},
                    "kind": {"type": "string", "enum": ["left", "center", "right", "decimal", "bar"]},
                    "leader": {"type": "string", "enum": ["none", "dot", "hyphen", "underscore", "heavy", "middleDot"]}
                }}},
                "list": {"type": "string", "enum": ["bullet", "decimal", "none"]},
                "list_style": {"type": "string", "enum": ["none", "disc", "diamond", "arrow", "star", "arrowhead", "box", "decimal", "paren", "outline", "upper", "roman", "zero", "checklist"], "description": "Restyles the whole list the paragraphs are in, or starts one"},
                "checked": {"type": "boolean", "description": "Check or uncheck checklist items"},
                "list_level": {"type": "integer"},
                "keep_with_next": {"type": "boolean", "description": "Keep on the same page as the next paragraph"},
                "keep_lines_together": {"type": "boolean", "description": "Don't split the paragraph across pages"},
                "page_break_before": {"type": "boolean", "description": "Start the paragraph on a new page"},
                "prevent_single_lines": {"type": "boolean", "description": "Widow/orphan control: avoid a lone first or last line on a page"},
                "shading": {"type": "string", "description": "Background colour as hex like FFF2CC, or \"none\""},
                "borders": {"type": "string", "enum": ["none", "box", "top", "bottom", "left", "right"], "description": "A thin line around the paragraph or on one side; replaces existing borders"}
            }}
        },
        {
            "name": "insert_break",
            "description": "Insert a page break, column break or section break at the end of some text (or at the end of the paragraph). Section breaks split the paragraph and start a new section: next_page, continuous, even_page or odd_page says how the new section starts. Not available in tables, headers, footers or footnotes.",
            "input_schema": {"type": "object", "required": ["id", "kind"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the break follows (first occurrence); default is the end of the paragraph"},
                "kind": {"type": "string", "enum": ["page", "column", "next_page", "continuous", "even_page", "odd_page"]}
            }}
        },
        {
            "name": "set_page_layout",
            "description": "Page layout for the whole document or one section: orientation, margins, text columns, line numbers, page colour and hyphenation. Sections are shown as section_break tags in read_document; pass `section` (the id of any paragraph in the section) to change just that section.",
            "input_schema": {"type": "object", "properties": {
                "section": {"type": "string", "description": "Paragraph id inside the section (omit for every section)"},
                "orientation": {"type": "string", "enum": ["portrait", "landscape"]},
                "margins_pt": {"type": "object", "description": "Margins in points; omitted sides stay as they are", "properties": {
                    "top": {"type": "number"}, "right": {"type": "number"}, "bottom": {"type": "number"}, "left": {"type": "number"}}},
                "columns": {"type": "integer", "description": "1 to 6 text columns"},
                "column_spacing_pt": {"type": "number"},
                "column_line": {"type": "boolean", "description": "Draw a line between columns"},
                "line_numbers": {"type": "string", "enum": ["off", "each_page", "each_section", "continuous"]},
                "page_color": {"type": "string", "description": "Hex like FFF2CC, or \"none\" (whole document)"},
                "hyphenation": {"type": "boolean", "description": "Automatic hyphenation (whole document)"},
                "page_numbers_start": {"type": "integer", "description": "Restart page numbers at this number in the section; 0 continues from the section before"},
                "page_number_format": {"type": "string", "enum": ["decimal", "lowerRoman", "upperRoman", "lowerLetter", "upperLetter"]}
            }}
        },
        {
            "name": "add_bookmark",
            "description": "Bookmark text so links can point at it: the first occurrence of `text` in a paragraph, or the whole paragraph when `text` is omitted. Names start with a letter and have only letters, digits and underscores (40 characters at most) and must be new; delete or rename an existing one first. Link to it with set_link and a url like \"#Intro\".",
            "input_schema": {"type": "object", "required": ["id", "name"], "properties": {
                "id": {"type": "string"},
                "text": {"type": "string", "description": "Exact text to bookmark (first occurrence); default is the whole paragraph"},
                "name": {"type": "string", "description": "e.g. Results_2024"}
            }}
        },
        {
            "name": "delete_bookmark",
            "description": "Remove a bookmark by name. Links that point at it are left as they are.",
            "input_schema": {"type": "object", "required": ["name"], "properties": {"name": {"type": "string"}}}
        },
        {
            "name": "insert_horizontal_line",
            "description": "Insert a horizontal line (a rule across the text width) after some text in a paragraph, or after the paragraph when `after_text` is omitted. An empty paragraph becomes the line. Shown in read_document as a paragraph tagged hr.",
            "input_schema": {"type": "object", "required": ["id"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the line follows (first occurrence); default is the end of the paragraph"}
            }}
        },
        {
            "name": "set_watermark",
            "description": "Put a text watermark such as DRAFT or CONFIDENTIAL behind the text on every page of every section (it goes in the headers; the headers' own text is kept). Use text \"none\" to remove the watermark.",
            "input_schema": {"type": "object", "required": ["text"], "properties": {
                "text": {"type": "string", "description": "One line, or \"none\" to remove the watermark"},
                "layout": {"type": "string", "enum": ["diagonal", "horizontal"], "description": "Default diagonal"},
                "color": {"type": "string", "description": "Hex like C0C0C0 (default light grey)"},
                "font": {"type": "string", "description": "Default is the document's body font"},
                "opacity": {"type": "number", "description": "0 to 1, default 0.5"}
            }}
        },
        {
            "name": "check_accessibility",
            "description": "Check the document for accessibility problems: images without alt text, skipped heading levels, tables without a header row or with merged cells, vague link text (\"click here\"), low colour contrast, typed lists, runs of blank lines, and a missing title or language. Returns one line per issue with the paragraph id. Fix them with apply_ops (set_image alt text, table_props header), set_paragraph_style, rewrite_paragraph, set_link, set_document_properties and set_language.",
            "input_schema": {"type": "object", "properties": {}}
        },
        {
            "name": "set_document_properties",
            "description": "Set the document's title, subject, author, keywords, description (Word's Comments), category, status, company or manager (the file's properties, not text on the page), and its custom properties. Properties you leave out stay as they are; an empty string removes one. `custom` adds or changes custom properties (type text, number, date as 2026-10-14, or bool); `remove_custom` lists names to remove.",
            "input_schema": {"type": "object", "properties": {
                "title": {"type": "string"}, "subject": {"type": "string"}, "author": {"type": "string"},
                "keywords": {"type": "string"}, "description": {"type": "string"},
                "category": {"type": "string"}, "status": {"type": "string"}, "company": {"type": "string"}, "manager": {"type": "string"},
                "custom": {"type": "array", "items": {"type": "object", "required": ["name", "value"], "properties": {
                    "name": {"type": "string"}, "type": {"type": "string", "enum": ["text", "number", "date", "bool"]}, "value": {"type": "string"}
                }}},
                "remove_custom": {"type": "array", "items": {"type": "string"}}
            }}
        },
        {
            "name": "set_language",
            "description": "Set the language of the text as a BCP 47 tag such as en-US, es-ES or fr-FR, so spell checkers and screen readers use the right one. Without `ids` it sets the document's default language; with `ids`, only the text of those paragraphs (for passages in another language).",
            "input_schema": {"type": "object", "required": ["lang"], "properties": {
                "lang": {"type": "string", "description": "e.g. en-US"},
                "ids": {"type": "array", "items": {"type": "string"}, "description": "Paragraph ids; omit for the whole document"}
            }}
        },
        {
            "name": "edit_table",
            "description": "Add or delete rows/columns, addressed by the id of a paragraph in the target cell.",
            "input_schema": {"type": "object", "required": ["cell", "action"], "properties": {
                "cell": {"type": "string"},
                "action": {"type": "string", "enum": ["row_above", "row_below", "col_left", "col_right", "delete_row", "delete_col", "delete_table"]}
            }}
        },
        {
            "name": "set_table_design",
            "description": "Give a table one of Word's built-in designs and choose which parts of it show. `design` is a name such as \"Table Grid\", \"Plain Table 1\" to \"Plain Table 5\", \"Grid Table 1 Light\", \"Grid Table 2\" to \"Grid Table 7 Colorful\" and \"List Table 1 Light\" to \"List Table 7 Colorful\", the last two kinds also with \" Accent 1\" to \" Accent 6\" (for example \"Grid Table 4 Accent 1\"), or the name of a table style the document already has. The options are Word's Table style options: header_row, total_row, first_column, last_column, banded_rows and banded_columns. Leave out `design` to change only the options.",
            "input_schema": {"type": "object", "required": ["cell"], "properties": {
                "cell": {"type": "string", "description": "The id of a paragraph in the table"},
                "design": {"type": "string"},
                "header_row": {"type": "boolean"},
                "total_row": {"type": "boolean"},
                "first_column": {"type": "boolean"},
                "last_column": {"type": "boolean"},
                "banded_rows": {"type": "boolean"},
                "banded_columns": {"type": "boolean"}
            }}
        },
        {
            "name": "list_styles",
            "description": "List the paragraph and character styles defined in the document.",
            "input_schema": {"type": "object", "properties": {}}
        },
        {
            "name": "set_header_footer",
            "description": "Set the text of the page header or footer (created if missing). {PAGE} and {NUMPAGES} insert live page numbers. Replaces the first paragraph of the header/footer. With link_to_previous (and no text) it only changes whether a later section shares the previous section's header/footer.",
            "input_schema": {"type": "object", "required": ["kind"], "properties": {
                "kind": {"type": "string", "enum": ["header", "footer"]},
                "variant": {"type": "string", "enum": ["default", "first"], "description": "first = a different header/footer on the first page"},
                "link_to_previous": {"type": "boolean", "description": "Word's Link to previous, for a section after the first: true = show the previous section's header/footer again (this section's own is dropped); false = give this section its own copy of what it shows now so the two can differ. Text, if given, is then written into that section's own header/footer."},
                "section": {"type": "string", "description": "A paragraph id in the section to change (default: the last section). Later sections without their own header/footer show it too."},
                "text": {"type": "string", "description": "Inline Markdown, e.g. \"**Acme** — Page {PAGE} of {NUMPAGES}\""}
            }}
        },
        {
            "name": "insert_footnote",
            "description": "Add a footnote right after some text in a paragraph. Numbering is automatic.",
            "input_schema": {"type": "object", "required": ["id", "after_text", "text"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the note mark follows (first occurrence)"},
                "text": {"type": "string", "description": "The note's text"}
            }}
        },
        {
            "name": "list_sources",
            "description": "List the document's sources (its bibliography database, like Word's Manage Sources) with their tags, the citation style in use and how often each source is cited. Cite one with `cite` using its tag.",
            "input_schema": {"type": "object", "properties": {}}
        },
        {
            "name": "add_source",
            "description": "Add a source to the document's list of sources so it can be cited. Give its fields, or `bibtex` with BibTeX entries (as copied from Google Scholar, Zotero or a publisher) to add several at once. The tag (like Smi20) is made for you when left out. Never invent details you were not given.",
            "input_schema": {"type": "object", "properties": {
                "bibtex": {"type": "string", "description": "BibTeX text with one or more entries; the other fields are then ignored"},
                "tag": {"type": "string", "description": "Short key such as Smi20; made from the first author and year when omitted"},
                "type": {"type": "string", "enum": ["book", "article", "chapter", "conference", "report", "website", "thesis", "misc"]},
                "authors": {"type": "array", "items": {"type": "string"}, "description": "Names as \"Smith, John\" or \"John Smith\"; an organisation in braces: \"{World Health Organization}\""},
                "editors": {"type": "array", "items": {"type": "string"}, "description": "Editors of the book a chapter is in, written like authors"},
                "title": {"type": "string"},
                "container_title": {"type": "string", "description": "Journal, book, proceedings or website the source sits in"},
                "year": {"type": "string"}, "month": {"type": "string", "description": "Number or name"}, "day": {"type": "string"},
                "publisher": {"type": "string", "description": "Publisher, or the institution of a report or thesis"},
                "place": {"type": "string", "description": "City of publication"},
                "volume": {"type": "string"}, "issue": {"type": "string"}, "pages": {"type": "string", "description": "e.g. 45-67"},
                "edition": {"type": "string"}, "url": {"type": "string"}, "doi": {"type": "string"},
                "accessed": {"type": "string", "description": "YYYY-MM-DD, for web sources"},
                "genre": {"type": "string", "description": "e.g. Doctoral dissertation, Technical report"}
            }}
        },
        {
            "name": "cite",
            "description": "Insert a citation (a Word citation field, formatted in the document's style) right after some text in a paragraph, or at the paragraph's end. The sources must already be in the list (add_source). Several tags make one combined citation. Numbering and 2020a/2020b letters are kept right automatically.",
            "input_schema": {"type": "object", "required": ["id", "tags"], "properties": {
                "id": {"type": "string"},
                "after_text": {"type": "string", "description": "Exact text the citation follows (first occurrence); default is the end of the paragraph"},
                "tags": {"type": "array", "items": {"type": "string"}, "description": "Source tags, e.g. [\"Smi20\"]"},
                "page": {"type": "string", "description": "Page or pages, e.g. 23 or 23-25"}
            }}
        },
        {
            "name": "insert_bibliography",
            "description": "Insert the reference list (a Word bibliography field) listing the cited sources in the document's style, with the style's heading (References, Works Cited or Bibliography). It lists only sources that are cited, so cite them first. At the end of the document unless `after` is given.",
            "input_schema": {"type": "object", "properties": {
                "after": {"type": "string", "description": "Block id to insert after (default: the end of the document)"},
                "heading": {"type": "string", "description": "Heading text; an empty string leaves it out"}
            }}
        },
        {
            "name": "set_citation_style",
            "description": "Switch the document's citation style (apa, mla, chicago, ieee or harvard, or the id of a CSL style the app offers; an unknown id is answered with the ones that exist). Every citation and the bibliography are written again in the new style; Word keeps editing them. A note style (Chicago notes) turns the citations into footnotes.",
            "input_schema": {"type": "object", "required": ["style"], "properties": {"style": {"type": "string"}}}
        },
        {
            "name": "apply_ops",
            "description": "Advanced: apply raw Docxtor ops atomically (see Docxtor op reference). Sources and citations: {op:\"update_source\",tag,source:{...}}, {op:\"delete_source\",tag,force?}, {op:\"edit_citation\",at:{p,o},tags?,page?,suppress_author?,suppress_year?,suppress_title?}, {op:\"update_bibliography\"} (writes every citation and the bibliography again). Positions are {p: paragraph id, o: UTF-16 offset}. Bookmarks: {op:\"insert_bookmark\",from,to,name} (a collapsed range is a point), {op:\"rename_bookmark\",from:\"Old\",to:\"New\"} (links and REF fields follow), {op:\"delete_bookmark\",name}. Also {op:\"insert_horizontal_line\",at} and {op:\"set_watermark\",text,layout?,color?,font?,opacity?}. Alt text: {op:\"set_image\",at:{p,o},alt:\"...\"} at the image's position. The same op formats a picture: crop:[left,top,right,bottom] (fractions of the whole picture, negative pads; the size follows unless width or height is given), rot (degrees), flip_h, flip_v, geometry (rect, roundRect, ellipse, triangle, diamond, hexagon, star5, ...; corner for roundRect), border:{color,width_pt,dash} or \"none\", opacity (0 to 1), brightness and contrast (-1 to 1), recolor:{kind:grayscale|blackWhite|sepia|duotone,color1?,color2?} or \"none\", color_change:{from,to?}, shadow:{blur_pt,dist_pt,angle,color,alpha}, glow:{radius_pt,color,alpha}, soft_edge (points), reflection:{size,alpha,dist_pt} (each also \"none\"), and reset:true to take all of it off. A floating picture is placed with wrap (inline, square, tight, through, topBottom, behind, front), wrap_text (bothSides, left, right, largest), x and y in points with rel_x (page, margin, column, character, leftMargin, rightMargin, insideMargin, outsideMargin) and rel_y (page, margin, paragraph, line, topMargin, bottomMargin, insideMargin, outsideMargin), align (left, center, right, inside, outside), align_y (top, center, bottom, inside, outside), dist:[top,bottom,left,right], lock_anchor and allow_overlap. A table's header row: {op:\"table_props\",at:<paragraph in the table>,header:true}. List ops: {op:\"set_list_style\",ids,style}, {op:\"restart_numbering\",id,start?}, {op:\"continue_numbering\",id}, {op:\"set_checked\",ids,checked}. Your own lists: {op:\"define_list\",ids?,def:{name?,levels:[{format,text,start?,align?,suffix?,left?,hanging?,tab?,font?,size?,bold?,italic?,color?,restart?,legal?,style?}]}} writes a new definition (format is bullet or a number format such as decimal, upperRoman, cardinalText or japaneseCounting; text is the bullet or the label with counts, e.g. \"%1.%2.\"; style links a heading, e.g. \"Heading 1\"; indents are twips; up to nine levels, the rest copy the last) and puts the paragraphs in ids in it; its result's count is the numId. {op:\"set_numbering_value\",id,value,continue_previous?} gives a list item a number. {op:\"sort_paragraphs\",ids,keys?:[{type:\"text\"|\"number\"|\"date\",descending?,field?}],case_sensitive?,header?} sorts paragraphs in place. Shapes: {op:\"set_shape\",at:{p,o},w?,h?,rot?,flip_h?,flip_v?,name?,alt?, fill?: \"none\" | \"RRGGBB\" | {kind:\"solid\",color,alpha?} | {kind:\"gradient\",stops:[{pos 0-100,color,alpha?}],angle?,path?:\"circle\"|\"rect\"|\"shape\",scaled?} | {kind:\"pattern\",pattern:\"ltDnDiag\",fg,bg} | {kind:\"picture\",media,tile?,crop?,alpha?}, line?:\"none\"|{color?,width_pt?,dash?,alpha?,compound?:\"dbl\"|\"thickThin\"|\"thinThick\"|\"tri\",cap?,join?,head_arrow?,tail_arrow?,head_w?,head_len?,tail_w?,tail_len?}, effects?:\"none\"|{shadow?,inner_shadow?,glow?,soft_edge_pt?,reflection?,bevel?,extrusion?} (each an object of its settings in points, or \"none\"), body?:{ins_l?,ins_t?,ins_r?,ins_b?,v_anchor?,anchor_center?,auto_fit?:\"none\"|\"shape\"|\"text\",wrap_text?,vert?,columns?,column_gap_pt?}, member?}; a picture fill's media is a name from add_media. Styles and design (the tools define_style, create_style_from_selection, delete_style, set_theme, set_style_set and set_paragraph_spacing wrap these): {op:\"set_style_definition\",style?,def:{name?,kind?,basedOn?,next?,quick?,autoUpdate?,hidden?,priority?,ppr?:{...as format_paragraphs},rpr?:{...as format_text},lang?}} (without style it makes a new one), {op:\"create_style\",name,kind?,from,to}, {op:\"update_style\",style,from,to}, {op:\"delete_style\",style}, {op:\"set_theme\",theme?|definition?:{name,colors:{name,colors:{accent1:\"4472C4\",...}},fonts:{name,major,minor}},colors?,fonts?}, {op:\"set_style_set\",style_set?|definition?:{name,styles:\"<w:style ...>\",spacing?}}, {op:\"set_paragraph_spacing\",preset?|spacing?:{before?,after?,line?}}. Page layout (twips; leave out `section` for every section, `from:true` for that section and the ones after it): {op:\"set_section\",section?,from?,gutter?,rtl_gutter?,header?,footer?,v_align?:top|center|both|bottom,text_direction?,start?}, {op:\"set_page_borders\",section?,from?,borders:{offsetFrom:page|text,display:allPages|firstPage|notFirstPage,zOrder:front|back,top?,left?,bottom?,right?:{val,sz,space,color}}|null}, {op:\"set_columns\",count,widths?:[twips],spaces?:[twips]} (widths make unequal columns), {op:\"set_page_numbering\",chap_style?:1-9 or 0,chap_sep?} (chapter page numbers like 2-1), {op:\"insert_page_number\",place:top|bottom|margin_left|margin_right,align?,style?:plain|x_of_y} and {op:\"remove_page_numbers\"}, table of authorities {op:\"toa\",action:\"mark_citation\",at:{p,o},from?,long,short?,category:1-16} (1 Cases, 2 Statutes, 3 Other authorities, 4 Rules, 5 Treatises, 6 Regulations, 7 Constitutional provisions), {op:\"toa\",action:\"mark_all\",long,short?,category}, {op:\"toa\",action:\"insert\",after?|before?,options?:{category?,passim?,keepFormatting?,leader?,look?}} and {op:\"toa\",action:\"update\"} (update_fields updates it too), cover pages {op:\"insert_cover_page\",design:\"docxtor.simple\"|\"docxtor.band\"|\"docxtor.sidebar\"|\"docxtor.frame\"|\"docxtor.academic\"|\"docxtor.report\"|\"docxtor.blocks\"|\"docxtor.centered\"|\"docxtor.abstract\"} (replaces the cover page there; its boxes show the title, subtitle and author properties, so set them with set_doc_props) and {op:\"remove_cover_page\"}, ready-made headers and footers {op:\"insert_page_part\",kind:header|footer,design,section?} (header designs docxtor.blank, docxtor.three, docxtor.title, docxtor.titleDate, docxtor.banded, docxtor.titlePage; footer designs docxtor.blank, docxtor.three, docxtor.pageOf, docxtor.authorPage, docxtor.banded, docxtor.companyDate), {op:\"set_layout_settings\",mirror_margins?,gutter_at_top?,even_and_odd_headers?,book_fold?,print_two_on_one?,hyphenation_zone?,consecutive_hyphen_limit?,do_not_hyphenate_caps?}, {op:\"set_page_background\",fill:{kind:solid|gradient|radial|pattern|texture|picture,color?,color2?,angle?,media?}|null}. Notes: {op:\"set_note_props\",kind:footnote|endnote,section?,format?,start?,restart?:continuous|eachSect|eachPage,pos?:pageBottom|beneathText|sectEnd|docEnd} and {op:\"insert_custom_note\",at,kind,mark,text?}. Frames: {op:\"set_frame\",ids,frame:{w?,h?,wrap?,hAnchor?,vAnchor?,xAlign?,yAlign?}|null} and {op:\"set_drop_cap\",id,kind:none|drop|margin,lines?,distance?,font?}. Tables (at is a paragraph in the table; from and to are paragraphs in the first and last cell of a selection): {op:\"table_format\",at,width?:{type:\"auto\"|\"dxa\"|\"pct\",value},align?:\"left\"|\"center\"|\"right\",indent?,wrap?:\"none\"|\"around\",autofit?,cellMargins?:[top,right,bottom,left],spacing?,title?,description?,borders?:{top?,bottom?,left?,right?,insideH?,insideV?}} sets table properties (widths in twips, pct in fiftieths of a percent; an edge is {val:\"single\"|\"dotted\"|\"dashed\"|\"double\"|\"none\"|\"clear\",sz?,color?}); {op:\"table_rows\",from,to?,all?,height?:{value,rule:\"auto\"|\"atLeast\"|\"exact\"},cantSplit?,header?}; {op:\"table_column\",at,col?,width}; {op:\"table_cells\",from,to,action:{type:\"format\",width?,valign?,margins?,noWrap?,textDirection?:\"lrTb\"|\"tbRl\"|\"btLr\"}} or action:{type:\"borders\",top?,bottom?,left?,right?,insideH?,insideV?,diagDown?,diagUp?} or action:{type:\"split\",cols?,rows?} (cols and rows split a cell into that many; left out a merged cell is unmerged); {op:\"split_table\",at} splits before the row holding at; {op:\"table_cell\",at,action:\"insert\"|\"delete\",shift:\"right\"|\"down\"|\"left\"|\"up\"}; {op:\"sort_table\",from,to?,keys:[{type,descending?,field}],header?} sorts rows by column numbers (field counts from 1); {op:\"convert_to_table\",ids,separator:\"tabs\"|\"commas\"|\"paragraphs\"|\"other\",separator_char?,columns?,autofit?} and {op:\"convert_to_text\",at,separator,separator_char?}; insert_table also takes autofit (\"fixed\", \"contents\", \"window\") and column_width. A formula in a cell is an insert_field op with field \"= SUM(ABOVE)\" (ABOVE, LEFT, RIGHT, BELOW, or cells such as A1:B2).",
            "input_schema": {"type": "object", "required": ["ops"], "properties": {"ops": {"type": "array", "items": {"type": "object"}}}}
        }
    ])
}
