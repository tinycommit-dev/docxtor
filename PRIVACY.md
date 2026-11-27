# Privacy

Docxtor edits your documents in your browser. There is no Docxtor server that receives them, and no account. This page says what stays on your computer, what can leave it, when, and how to stop it. The list of addresses is checked by a test (`e2e/privacy.spec.ts` records every request the app and the embedded editor make, and fails when one is not in [`e2e/data/outside-hosts.json`](e2e/data/outside-hosts.json)); `npm test` checks that this page names exactly the addresses in that file.

## What stays in your browser

- **Your documents.** Opening a file reads it in the page. The Library, the version history, the trash and the recovery copies are kept in your browser's storage (an IndexedDB database named `docxtor`). Nothing is uploaded. When you use "Open file" or "Save" with a file on your computer, the browser remembers the handle to that file, not its contents, in the same database.
- **Your settings.** Theme, language, which extensions are on, the assistant's settings, your snippets and sources are kept in your browser's local storage (keys starting with `docxtor.`).
- **Your keys.** The key for the assistant's provider (and your Zotero and LanguageTool keys, if you use them) is saved as plain text in local storage on your computer: the assistant's in `docxtor.ai.v2`, the others in `docxtor.ext.<extension name>`. The page sends a key only to the address of the service it belongs to: your provider's address, or the base address you typed for it. The test checks this: it gives each provider a different made-up key and fails if one turns up in a request to any other address, or in an address at all. Anyone who can use your browser profile, and any script running on the same site, can read these keys, so use a key that has a spending limit and that you can cancel. Clearing the site's data in your browser removes them.
- **Spelling.** The word lists are files of the app itself. Checking spelling sends nothing anywhere.

Docxtor's own page has no analytics, no tracking and no advertising, and its code sets no cookies.

## What can leave your computer

Most of these happen only after you do something: look a word up, turn an extension on, send a message to the assistant. Two happen without that, and they are the first two rows.

| Address | What it is | When | What is sent |
| --- | --- | --- | --- |
| `fonts.googleapis.com` and `fonts.gstatic.com` | Google Fonts | **Without asking, and on by default.** In the app: when a document uses one of the twenty Google font families (the Google Fonts extension), when you open the font menu (the menu lists those twenty families, so the page asks for all of them), and when you download a PDF of a document that uses one. In an editor embedded in another page: when the editor starts, unless the page that embeds it passes `fonts: false` or the address of its own font file. | The names of the font families, in the address. Google also sees your IP address, your browser, and the address of the page. No document text. |
| `en.wiktionary.org` | Tools, Dictionary | When you look a word up. The extension is on by default. | The word. |
| `api.datamuse.com` | More synonyms in the thesaurus | After you turn the Datamuse extension on (it is off by default). | The word you look up. |
| `doi.org` | Citations, From a DOI or URL | When you look a DOI up. | The DOI you typed. |
| `localhost:23119` | Zotero on this computer | When you open Citations, From Zotero, and as you search. | Your search words, to a program on your own computer. |
| `api.zotero.org` | Your Zotero web library | After you choose the web library in Zotero settings and give a user ID and key. | Your search words, your user ID in the address, and your key in a header. |
| `api.languagetool.org` | Grammar checking by LanguageTool | After you turn the LanguageTool extension on (it is off by default, and asks you first). You can give it another server in its settings. | **The text of your document**, paragraph by paragraph, again after edits, with its language, and your LanguageTool user name and key if you gave them. |
| `api.anthropic.com`, `api.openai.com`, `openrouter.ai`, `generativelanguage.googleapis.com`, `api.groq.com`, `api.mistral.ai` | The assistant, with that provider | After you choose the provider, give its key, and send a message. | **Your message and the document**: its name, its text and the part you selected, and what the assistant reads or changes while it works, with your key in a header. OpenRouter also gets the address of the page, as it asks. |
| `localhost:11434`, `localhost:1234` | The assistant, with Ollama or LM Studio | After you choose the provider and send a message. | The same as above, to a program on your own computer. Nothing leaves it. |

Every request your browser makes also carries what a browser always sends: its name and version, your language, and the address of the page it comes from (just the site, not the document). Anthropic's own library adds its name, its version and the browser's name.

These happen at an address you give, so there is no fixed list:

- **A picture from a web address** (Insert, Image, From a URL): your browser fetches that address, with no cookies and no referrer.
- **A citation style from an address** (Citations, Add a style from a CSL file): your browser fetches that address.
- **A paste that has a picture on the web in it:** your browser fetches that picture, with no cookies, so it can be kept in the document.
- **The assistant's Custom provider, or a base address you set for a provider:** the same as for any provider, to that address.
- **Links.** Opening a link in a document (or a Help link to the project's page) opens a new tab for it, only when you choose it. Docxtor opens web and email addresses and nothing else. A link that tries to run script is refused with a notice.

## What a document can make the page do

A document is a file someone else may have made. The hostile documents in `e2e/hostile.spec.ts` (script, addresses and styles in every place text can hide: paragraphs, links, fields, picture descriptions, document details, comments, styles and fonts, list text, SVG pictures, embedded HTML, custom XML, themes, shapes, charts, page colours) are opened in the app with every request refused. None of them runs script, opens a window or a dialog, or gets the page to ask for an address it was not already going to ask for. What a document can still cause, and you may notice:

- **Google Fonts.** A document that uses one of the twenty Google font families makes the app ask Google for that family when it opens (the first row of the table). It is the one thing a document can cause that leaves your computer without you asking; turning the Google Fonts extension off stops it.
- **Linked pictures.** A picture a document links on the web is not fetched when it opens. Its placeholder has a button, Show linked pictures, and only that fetches it. When you download the document as a web page or as Markdown, the address of a linked picture stays in the file, so the page loads it when someone opens it.
- **Links.** A link in a document opens only when you choose it, and only if it is a web or email address.

## What the hosted site keeps

The hosted app, at `docxtor.tinycommit.dev`, is a set of static files on Cloudflare Pages. Docxtor runs no server of its own: no database, no function, no log. The files carry no analytics and no tracking, the site's code sets no cookies, and the headers it sends (`deploy/_headers`) set none. Cloudflare, which serves the files, sees each request for them (your IP address, the time, the file and your browser) as any web host does, and its own policy says what it keeps and for how long. Cloudflare offers an optional analytics script for sites on Pages. Docxtor's files do not contain it, and the site's Content-Security-Policy (`script-src 'self'`, in `deploy/_headers`) would not let a script from another address run.

The app can be used offline after the first visit: a service worker keeps the app's own files in your browser, and fetches nothing else.

## Sending nothing anywhere: self-hosting

Run Docxtor from your own server and turn off the two defaults above. The [self-hosting archive and Docker image](docs/deploy.md) hold everything the app needs, including the fonts and the spelling dictionaries.

1. Under Extensions, Manage extensions, turn the **Google Fonts** extension off. Then the app asks Google for nothing. Fonts that Word's documents use are shown with the bundled look-alikes.
2. Leave the assistant, LanguageTool, Datamuse and Zotero's web library alone, or point the assistant at a model you run yourself (Ollama, LM Studio, or your own address under Custom).
3. To make sure, and to make it so for everyone who uses your copy, change the Content-Security-Policy in `deploy/_headers` (and the nginx and Caddy files made from it, see [Putting Docxtor online](docs/deploy.md)): take `https://fonts.googleapis.com` and `https://fonts.gstatic.com` out of `style-src` and `font-src`, take `https: http:` out of `img-src`, take `https:` out of `media-src`, and make `connect-src` just `'self' data: blob:`. Leave the rest as it is: `script-src 'self' 'wasm-unsafe-eval'` and `worker-src 'self' blob:` are what the engine needs. The browser then refuses every request to another address, including the assistant's, so the assistant works only if you serve a gateway of your own from the same site. `e2e/privacy.spec.ts` runs the app with exactly this policy and the Google Fonts extension off, opens a document that uses a Google font, opens the font menu and downloads a PDF, and checks that nothing is requested and nothing is refused.

For the editor embedded in your own page: pass `fonts: false`, or `fonts: "/your/fonts.css"` to serve the same families yourself. The editor itself makes no other request of its own. See [Embedding Docxtor](docs/embedding.md).

## Questions and problems

If you find a place where Docxtor sends something that this page does not say, that is a bug: please report it as described in [SECURITY.md](SECURITY.md).
