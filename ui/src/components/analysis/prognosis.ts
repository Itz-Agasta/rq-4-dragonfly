/**
 * The axis every remaining-life figure on the screen is placed on, and the
 * parameter the trajectory follows.
 */

import type { Parameter } from "@/lib/signatures";
import type { Frame } from "@/lib/telemetry";

/**
 * Floor of the shared log axis, hours.
 *
 * The floor sets which row is legible, so it is placed under the shortest life
 * the bus actually produces rather than at a round number. A coking injector
 * measures 0.67 h: at a 1 h floor it clamps against the left stop, and at a 4 h
 * floor it lands in the leftmost few percent of the track crushed under its own
 * label, while every healthy subsystem gets a clearly placed tick. The one row
 * that matters becomes the one row that cannot be read. Five decades from 0.1 h
 * still leave about 60 px each.
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
  parameters.forEach((_, i) => {
    const consumed = p.parameter[i]?.consumed ?? 0;
    if (consumed > most) {
      most = consumed;
      best = i;
    }
  });
  return best < 0 ? null : best;
}
