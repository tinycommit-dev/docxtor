# Engine modules: AI tools and fidelity

The modules of `docxtor-core` that give the AI, the CLI and the MCP server their tools, and the module that scores how faithfully a file survives a save. Part of the [architecture](index.md) pages; [how the engine works](engine-concepts.md) comes first. The other module pages are [files and the model](engine-files-and-model.md), [rendering and layout data](engine-rendering-and-layout.md) and [content features](engine-content-features.md). The tools themselves are described for users in [AI, the CLI and MCP](../ai/index.md).

## DocScript and the tool executor

`ai` holds DocScript, the tool definitions and the tool executor. Every host shares them, so the in-app assistant, the CLI and the MCP server see the same tools and each tool becomes ordinary ops.

## Tool modules

The tools of a feature live in a module of their own and are merged into `ai`'s tool list and executor. `ai` collects each module's `definitions()` into one list, and routes a call to the module that owns the tool.

| Module | Tools for |
|---|---|
| `ai_refs` | Endnotes, captions, cross-references, the index, tables of figures and updating fields |
| `ai_lists` | The table of contents (options, entries, removal), index options and AutoMark |
| `ai_charts` | Charts, and DocScript's chart table |
| `ai_design` | Styles, themes, style sets and paragraph spacing |
| `ai_merge` | Mail merge: look at the merge, start one from a list of recipients, put merge fields in a letter, read what a record would give |
| `ai_outline` | Heading levels and moving a section |
| `ai_review` | Word counts, and the inspector for personal information and hidden data |
| `ai_forms` | Content controls, form fields and protection. The assistant can fill a form in and look at what a document restricts; it cannot set or remove a password |
| `ai_east_asian` | Phonetic guides, the Asian typography switches, the East Asian font and layout of text, the document grid and the line-breaking rules |

Some feature modules carry their tools beside the feature instead: `shapes/ai` for shapes, and `math` for `insert_equation` and `set_equation`. They are merged the same way.

## Fidelity

`fidelity` measures round-trip fidelity. It compares two packages, typically an original file and Docxtor's re-save of it, and produces a score: how many parts are identical, and for the others a structural similarity of the XML and whether the text is equal. That turns "Docxtor preserves your document" into a number that CI tracks on every commit (see [Fidelity with Word](../fidelity.md)).
