// Real bugs the matrix (matrix.spec.ts) found that are not fixed yet. Each entry silences one finding until the bug is
// fixed; then delete the entry. A finding is "<rule>: <message>"; `mode` and `action` may be "*" for any. Say why it is
// listed, and where the cause is when you know. Never list a whole rule: match the message.
export interface MatrixKnown {
  mode: string;
  action: string;
  pattern: RegExp;
  reason: string;
}

export const MATRIX_KNOWN: MatrixKnown[] = [];
