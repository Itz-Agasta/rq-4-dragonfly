/**
 * The axis every remaining-life figure on the screen is placed on, and the
 * parameter the trajectory follows.
 */

import type { Parameter } from "@/components/analysis/signatures";
import type { Frame } from "@/lib/telemetry";

/**
 * Floor of the shared log axis, hours.
 *
 * `docs/design/PUNCHLIST.md` ANALYSIS [A] asks for a 1 h floor against the
 * artboard's 4, because at 4 h the only row that matters lands in the leftmost
 * few percent of the track, crushed under its own label, while every healthy
 * subsystem gets a clearly placed tick: the failing row becomes the one row that
 * cannot be read.
 *
 * The floor is 0.1 h rather than 1 h for that same reason carried one decade
 * further. A remaining life under an hour is not hypothetical here: measured on
 * the bus with a coking injector it reads 0.67 h, which at a 1 h floor clamps
 * hard against the left stop and is invisible for exactly the reason the
 * punchlist item was written. Five decades still leave about 60 px each.
 */
export const AXIS_MIN_H = 0.1;

/** Ceiling of the shared log axis, hours. Mirrors `rul::HORIZON_H`. */
export const AXIS_MAX_H = 1000;

/** Decade ticks, drawn and labelled. */
export const AXIS_TICKS = [0.1, 1, 10, 100, 1000];

const SPAN = Math.log10(AXIS_MAX_H) - Math.log10(AXIS_MIN_H);

/** Position of an hours figure on the shared log axis, 0 to 1. */
export function logX(hours: number): number {
  if (!Number.isFinite(hours)) return 1;
  const clamped = Math.min(Math.max(hours, AXIS_MIN_H), AXIS_MAX_H);
  return (Math.log10(clamped) - Math.log10(AXIS_MIN_H)) / SPAN;
}

/**
 * The health parameter the screen is about, or null before there is one.
 *
 * The limiting subsystem's driver when something is declining, and otherwise the
 * parameter furthest from nominal. The second case is deliberate: on a healthy
 * engine "nothing is failing and this is the one to watch" is a more useful
 * trajectory than an empty panel, and the same choice `rul::soonest` makes.
 */
export function focus(frame: Frame, parameters: Parameter[]): number | null {
  const p = frame.prognosis;
  if (!p || parameters.length === 0) return null;

  const limiting = p.limiting;
  if (limiting !== null) {
    const driver = p.subsystem[limiting]?.driver;
    const i = parameters.findIndex((entry) => entry.name === driver);
    if (i >= 0) return i;
  }

  let best = -1;
  let most = -Infinity;
  p.parameter.forEach((r, i) => {
    if (r.consumed > most) {
      most = r.consumed;
      best = i;
    }
  });
  return best < 0 ? null : best;
}
