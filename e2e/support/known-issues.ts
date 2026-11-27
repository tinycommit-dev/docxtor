// Real bugs in Docxtor that the watchdog in helpers.ts found in tests that already existed.
// Each entry silences one error pattern in one test until the bug is fixed; then delete the entry.
// `test` is "<spec file> › <describe titles …> › <test title>"; `pattern` is matched against the
// error message and its stack. Keep patterns specific: never match everything.
export interface KnownIssue {
  test: string;
  pattern: RegExp;
  reason: string;
}

export const KNOWN_ISSUES: KnownIssue[] = [
];
