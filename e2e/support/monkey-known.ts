// Real bugs the random clicker (monkey.spec.ts) found that are not fixed yet. Each entry silences one kind of error until
// the bug is fixed; then delete the entry. An error is "<rule>: <message>" (the text `checkApp` returns) or, for a page
// error, "U1: uncaught error: <name>: <message>". Match the message, never a whole rule, and say why it is listed and
// where the cause is when you know.
export interface MonkeyKnown {
  pattern: RegExp;
  reason: string;
}

export const MONKEY_KNOWN: MonkeyKnown[] = [];
