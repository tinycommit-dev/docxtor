# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue.

Use GitHub's private vulnerability reporting: open the **Security** tab of [github.com/tinycommit-dev/docxtor](https://github.com/tinycommit-dev/docxtor) and choose **Report a vulnerability**. Include what you found, the steps or the file that shows it, and the version or commit.

You can expect a first reply within a few days. The problem is confirmed, a fix is worked out with you, and the release notes say when it ships and who found it, unless you prefer not to be named. Please allow time for a fix before telling anyone else.

## What is in scope

- **The engine on hostile files.** A `.docx` (or an RTF, ODT, HTML, Markdown or other file Docxtor opens) that makes the engine crash in a way that loses data, run for an unreasonable time, use unbounded memory, write outside where it should, or produce a saved file that executes something when opened.
- **The app** (`web/`). Script injection from a document, a comment, a template, a pasted fragment or an AI reply; a document or extension that reads or sends data it should not; a way to make the app send a file to an address the person did not choose.
- **The packages**: `@docxtor/core`, `@docxtor/extensions` and `@docxtor/editor`, including the `<docxtor-editor>` element, the extension host and the CLI and MCP server (`docxtor`, `docxtor-mcp`).
- **The build and release files in this repository**, such as the CI workflows and the published archives.

## What is out of scope

- Problems in a service Docxtor can be pointed at (an AI provider, LanguageTool, Zotero), or in an extension you installed yourself.
- A document that looks different in Word, or a feature that is missing: please open an ordinary issue.
- Reports that need a person's device or browser to be already compromised.

## Privacy

What stays in the browser, which outside addresses the app can call and when, and how to send nothing anywhere: [PRIVACY.md](PRIVACY.md). A place where Docxtor sends something that page does not say is in scope for a report.

## Supported versions

Docxtor has not had a stable release yet. Fixes go to the latest version on `main`.
