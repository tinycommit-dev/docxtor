// A message as a mailto: link, for the person's own mail app. Pure, so it runs in Node too.
import type { EmailMessage } from "@docxtor/extensions";

/** The most a mailto: link holds before some mail apps and systems refuse it. */
export const MAILTO_LIMIT = 2000;

/** The link that opens a message in the mail app, with the text cut (and marked) when the whole thing would be too long. */
export function mailtoLink(m: Pick<EmailMessage, "to" | "subject" | "text">, limit = MAILTO_LIMIT): { url: string; cut: boolean } {
  const to = m.to.replace(/[\s?#&%]/g, encodeURIComponent);
  const crlf = (s: string) => s.replace(/\r?\n/g, "\r\n");
  // A lone half of an emoji (from a cut, or from the document) makes encodeURIComponent throw.
  const whole = (s: string) => s.replace(/[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/g, "\uFFFD");
  // Cut before an emoji rather than through it.
  const head = (n: string, k: number) => (k > 0 && k < n.length && /[\uD800-\uDBFF]/.test(n[k - 1]) ? n.slice(0, k - 1) : n.slice(0, k));
  const make = (body: string) => `mailto:${to}?subject=${encodeURIComponent(whole(m.subject))}&body=${encodeURIComponent(whole(crlf(body)))}`;
  const text = m.text ?? "";
  let url = make(text);
  if (url.length <= limit) return { url, cut: false };
  // Keep as much as fits, counting letters rather than bytes.
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    if (make(`${head(text, mid)}…`).length <= limit) lo = mid;
    else hi = mid - 1;
  }
  url = make(`${head(text, lo).trimEnd()}…`);
  return { url, cut: true };
}
