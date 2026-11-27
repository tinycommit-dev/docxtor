// Builds small .docx files for the browser tests: sections with their own paper, margins, how they start,
// headers and footers. (The engine can add a header to the last section only, so documents whose sections
// have different headers have to be made outside it.) Nothing here is a fixture: every test builds the
// document it needs and opens it with `openBuilt`.
export * from "./docx-zip";
export * from "./docx-core";
export * from "./docx-chartex";
export * from "./docx-smartart";
export * from "./docx-charts";
