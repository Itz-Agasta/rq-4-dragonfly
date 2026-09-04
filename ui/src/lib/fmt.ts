/**
 * Number and time formatting for readouts.
 *
 * Everything here returns a string that is stable in width for a given magnitude,
 * because these land in elements that update twenty times a second.
 */

/** U+2212. The ASCII hyphen is narrower than a digit and makes columns jump. */
const MINUS = "−";

/** What a readout shows when its value is not available. */
export const NO_VALUE = "—";

/**
 * Fixed-decimal value with a proper minus sign.
 *
 * Returns {@link NO_VALUE} rather than `NaN` for anything non-finite: a readout
 * that says nothing is honest, one that says `NaN` is a bug report shown to an
 * operator.
 */
export function fmt(value: number, dp = 0): string {
  if (!Number.isFinite(value)) return NO_VALUE;
  return value.toFixed(dp).replace("-", MINUS);
}

/**
 * A posterior as a percentage, held short of certainty.
 *
 * The likelihood ratio saturates in f64 once one signature fits and the rest do
 * not, so an uncapped panel prints `100.0%` for a posterior measured at
 * 0.9999840984. Another decimal only moves the same problem to 99.99%. The `>`
 * says the cap is doing the work, as it does for the CUSUM in `DetectionBar`.
 *
 * `exact` for the null hypothesis, which reads 1.0 because isolation is gated on
 * detection rather than because a ratio saturated. Capping that one prints
 * `>99.9%` on a healthy engine and claims an uncertainty the number does not
 * carry.
 */
export function posteriorPct(posterior: number, exact = false): string {
  if (!Number.isFinite(posterior)) return NO_VALUE;
  const percent = posterior * 100;
  return !exact && percent >= 99.95 ? ">99.9" : fmt(percent, 1);
}

/**
 * Fixed-decimal value that always carries a sign character.
 *
 * The plus is what keeps a column of residuals aligned: without it a positive
 * value is one glyph narrower than the negative above it and the decimal points
 * stop lining up, which tabular figures alone do not fix.
 */
export function signed(value: number, dp = 0): string {
  if (!Number.isFinite(value)) return NO_VALUE;
  return value >= 0 ? `+${fmt(value, dp)}` : fmt(value, dp);
}

/** Thousands-grouped integer, for rpm and similar wide values. */
export function grouped(value: number): string {
  if (!Number.isFinite(value)) return NO_VALUE;
  return Math.round(value).toLocaleString("en-US").replace("-", MINUS);
}

/** Hours at the precision the magnitude supports. */
function figure(hours: number): string {
  return fmt(hours, hours < 10 ? 2 : hours < 100 ? 1 : 0);
}

/**
 * A remaining life, split into its figure and its unit.
 *
 * **Hours on every readout, no minutes anywhere.** The subsystem table sits on an
 * axis ticked `0.1 h` to `1k`, and a row in minutes cannot be compared against the
 * row above it or placed against the tick beneath it. Split from its unit because
 * a `0.53 h` returned whole wraps the `h` onto its own line at hero size.
 *
 * Zero is a parameter past its threshold, worded rather than rounded into a
 * number. Use {@link lifeHours} for an interval bound, where zero means something
 * else entirely.
 */
export function remainingLife(hours: number): { value: string; unit: string } {
  if (!Number.isFinite(hours)) return { value: NO_VALUE, unit: "" };
  if (hours <= 0) return { value: "PAST", unit: "limit" };
  return { value: figure(hours), unit: "h" };
}

/**
 * A life in hours on one line, for a readout with no separate unit slot.
 *
 * Zero prints `0.00 h` and never {@link remainingLife}'s `PAST limit`: on a p50
 * zero means the parameter is spent, on a p10 it only means the crossing could be
 * now, and an interval reaching back to the present is the common case.
 */
export function lifeHours(hours: number): string {
  return Number.isFinite(hours) ? `${figure(hours)} h` : NO_VALUE;
}

/** `p10` to `p90`, unit once. `null` above is a bound the fit cannot set. */
export function lifeRange(p10: number, p90: number | null): string {
  return p90 === null ? `${figure(p10)} h – open` : `${figure(p10)} – ${figure(p90)} h`;
}

/** Seconds since start as `T+HH:MM:SS`. */
export function missionClock(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "T+00:00:00";
  const whole = Math.floor(seconds);
  const h = Math.floor(whole / 3600);
  const m = Math.floor((whole % 3600) / 60);
  const s = whole % 60;
  return `T+${pad2(h)}:${pad2(m)}:${pad2(s)}`;
}

function pad2(n: number): string {
  return n.toString().padStart(2, "0");
}

/** Kelvin to Celsius. The air data computer reports K; operators read C. */
export function kelvinToCelsius(k: number): number {
  return k - 273.15;
}

/** Metres to feet. */
export function metresToFeet(m: number): number {
  return m / 0.3048;
}

/** Metres per second to knots. */
export function msToKnots(v: number): number {
  return v / 0.514_444;
}
