/**
 * Report a bug that would otherwise be silent.
 *
 * Both callers guard against something that cannot happen in a correct build and
 * did happen: a frame that fails to decode, and a live sink that throws. Each was
 * swallowed by a `try` with nowhere to report to, and each cost a session to
 * find, because the symptom is a screen at its placeholders while the socket
 * delivers frames normally.
 *
 * The console is the right surface for these two. They are developer diagnostics
 * for a defect, not operator signals: an operator can do nothing about a decode
 * failure, and what they need to know is that the readouts are not current, which
 * the staleness rules already say.
 */
export function report(what: string, error: unknown): void {
  // eslint-disable-next-line no-console -- the whole point of this module
  console.error(what, error);
}
