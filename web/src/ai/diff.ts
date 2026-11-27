import { escapeHtml } from "@docxtor/editor";

/** Word-level diff rendered as HTML with <del>/<ins>. */
export function wordDiff(a: string, b: string): string {
  const A = a.split(/(\s+)/);
  const B = b.split(/(\s+)/);
  if (A.length * B.length > 250_000) return `<del>${escapeHtml(a)}</del> <ins>${escapeHtml(b)}</ins>`;
  const n = A.length;
  const m = B.length;
  const dp: Uint32Array[] = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) dp[i][j] = A[i] === B[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  let i = 0;
  let j = 0;
  let out = "";
  const flush = (tag: string, buf: string[]) => (buf.length ? `<${tag}>${escapeHtml(buf.join(""))}</${tag}>` : "");
  let del: string[] = [];
  let ins: string[] = [];
  while (i < n || j < m) {
    if (i < n && j < m && A[i] === B[j]) {
      out += flush("del", del) + flush("ins", ins);
      del = [];
      ins = [];
      out += escapeHtml(A[i]);
      i++;
      j++;
    } else if (j < m && (i >= n || dp[i][j + 1] >= dp[i + 1][j])) ins.push(B[j++]);
    else del.push(A[i++]);
  }
  return out + flush("del", del) + flush("ins", ins);
}
