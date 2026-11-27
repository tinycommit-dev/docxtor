// Plain text → Markdown for the engine. (HTML, RTF and Docxtor's own format are read by the engine itself: see paste-convert.ts.)

const esc = (s: string) => s.replace(/([\\*_~`[\]$])/g, "\\$1");

/** Plain text → Markdown that reproduces it literally (each line a paragraph). */
export function textToMarkdown(text: string): string {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const lit = (l: string) => esc(l).replace(/^(\s*)(#|>|-|\+|\|)/, "$1\\$2").replace(/^(\s*\d+)\./, "$1\\.");
  if (lines.length === 1) return lit(lines[0]);
  return lines.map((l) => (l.trim() ? lit(l) : "")).join("\n\n").replace(/\n{3,}/g, "\n\n");
}
