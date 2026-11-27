# Mail merge

Mail merge fills one document with the details of many people: a letter for each customer, a message for each member, a sheet of address labels, an envelope for each name. It is under the Mailings menu and works the way Word's does. Docxtor writes the merge into the file as Word writes it, so Word can carry on with it.

## The steps

1. **Write the document** with the text that is the same for everyone. Or use File, New from a template, Letter.
2. **Start the merge.** Mailings, Start mail merge, then Letters, Email messages, Labels, Envelopes or Directory. Normal document stops a merge and leaves the text as it is.
3. **Choose the recipients.** Mailings, Select recipients, From a CSV file or From an Excel file. The first row of the file must hold the column names, such as First Name, Last Name and City. Extensions can add other sources.
4. **Check the list.** Mailings, Edit recipient list (Alt+Shift+E, ⌥⇧E) shows the people. You can untick the ones to leave out, filter the list and find people. Find duplicates lists records that look alike. Match fields tells Docxtor which of your columns is the address, the name and so on.
5. **Put in the merge fields.** Place the cursor where a detail goes and choose Mailings, Insert merge field (Alt+Shift+F, ⌥⇧F), then a column name. A field looks like «First Name». For letters, Address block and Greeting line add a whole address or "Dear Ada," in one go. Highlight merge fields makes every field easy to see.
6. **Add rules if you need them.** Mailings, Rules has If…Then…Else, Ask, Fill-in, Skip record if, Next record, Merge record # and others, for text that depends on the person.
7. **Preview.** Mailings, Preview results shows the document with one person's details. First record, Previous record, Next record and Last record move through the list, and Find recipient jumps to a name.
8. **Finish.** Mailings, Finish and merge (Alt+Shift+N, ⌥⇧N) makes the result.
   - **A new document**, with a section for each person, that you can read and edit.
   - **A PDF document.**
   - **Print merged documents** sends every record to the print dialog, each starting on a new page.

If your merge uses Ask or Fill-in rules, Docxtor asks the questions once before it makes the result.

## Email

Choose Email messages at step 2. Write the subject line, which can contain fields, and tell Docxtor which column holds each person's email address. Mailings, Send email messages offers the message as plain text, HTML, or with the letter attached as a Word or PDF file.

Docxtor has no mail server of its own, so it does not send anything itself. The ways to send that come with it save a .zip of .eml files, which most mail programs open, and open your own mail app. Extensions can add a way to send through a service you use. Look under Extensions, Manage extensions if none is on.

## Labels and envelopes

Mailings, Labels asks for the address and the sheet you will print on (a label vendor and product number, or your own size), and makes either a single label or a full page of the same label in a new document. Mailings, Envelopes asks for the delivery and return addresses and the envelope size, and makes the envelope the first page of the document. For labels from a list of people, choose Labels in Start mail merge, select the recipients, insert the address block in the first label, then choose Update labels so each label takes the next person.

## Good to know

- The recipient list is kept inside the document, so the document and its list travel together. A list you choose from a file is read once; if the file changes, choose it again.
- A rule that skips a record leaves that person out of the result and the preview says so.
- Merges that go through a PDF or a new document run in your browser. Nothing is uploaded.
