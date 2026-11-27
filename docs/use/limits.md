# Limits

What Docxtor does not do, and where it works.

## What is missing

- **Comments and tracked changes are kept but not shown.** A file with comments or tracked changes opens, and saves with them still in it, so Word shows them. Docxtor does not draw comments or let you add, answer or resolve them, and it does not let you accept or reject tracked changes. A tracked deletion appears as a labelled placeholder instead of struck-through text. They are next on the [roadmap](../roadmap.md). Tools, Compare documents shows the differences between two versions of a document, and Combine documents merges them, as a stand-in.
- **No real-time editing with other people.** There is no shared cursor or live co-editing, and no sharing links or accounts. You share a file. See [save, download and share](saving-sharing.md).
- **SmartArt and WordArt are drawn, not edited.** Docxtor shows them from the file and keeps them; change them in Word.
- **Page breaks can differ from Word's.** Docxtor lays the pages out with your browser, so a page can end a line earlier or later than in Word, most of all when a font is missing.
- **Typing right after a field's result** such as a date or a link can extend the result's text.
- **No phone or tablet app.** The web app works at phone and tablet sizes, but editing a long document is easier with a keyboard.

## Browsers

| Browser | What works |
|---|---|
| Chrome, Edge and other Chromium browsers | Everything. Every browser test of Docxtor runs on Chromium. |
| Firefox and Safari | The core of editing: opening a document, typing, selecting, bold and italic, lists, tables, pasting, writing with an input method, undo, find and replace, saving to your Library, downloading a .docx, printing and the start screen. |

What Firefox and Safari lack:

- **Opening and saving files in place.** Open a file keeps a copy in your Library, Save downloads the .docx, and Open a folder is not offered.
- **Columns on screen.** A section with columns shows as one column. The columns are kept in the file, and Word shows them.
- A few small things, such as the eyedropper in colour pickers and reading rich content from the clipboard with a button. Press the paste key instead.

[Browsers](../browsers.md) has the full list and how each case is tested.

## Where it is stored

Your documents are in this browser's storage. Clearing the site's data deletes them. The Library does not sync between computers or browsers. Download a copy of what matters (File, Download, Download all documents).

Offline use and installing the app need the page to be served over HTTPS.

## Size

Very long documents work, up to a novel the length of War and Peace. Typing speed and memory by document size are on [the performance pages](../performance/index.md).
