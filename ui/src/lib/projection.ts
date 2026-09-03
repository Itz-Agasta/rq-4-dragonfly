/**
 * The mission projection API.
 *
 * A projection is the engine the twin currently believes in, flown forward
 * through a mission profile by the model alone. It is a one-shot fetch and not
 * telemetry: nothing here goes near the render loop or the telemetry store, and
 * the result is ordinary React state.
 *
 * The daemon answers **409** when the twin has no estimate. That is not an error
 * to retry: a projection with no seed would be a healthy engine flying a profile,
 * which is a simulator run wearing a twin's label, and the screen says so rather
 * than drawing one.
 */

import { decode } from "@msgpack/msgpack";

/** One projected channel over the horizon. Mirrors `project::Series`. */
export interface ProjectedSeries {
  name: string;
  unit: string;
  /** The certified limit, or null for a channel nothing alarms on. */
  limit: number | null;
  /** Whether that limit is published or estimated. Meaningless without a limit. */
  published: boolean;
  values: number[];
}

/** A limit the projection says will be crossed. Mirrors `project::Exceedance`. */
export interface Exceedance {
  channel: string;
  limit: number;
  /** Mission time of the crossing, absolute. */
  t_s: number;
  /** Seconds from the seed to the crossing. */
  in_s: number;
  peak: number;
  published: boolean;
}

/** One health parameter the projection was seeded with. Mirrors `project::SeedParam`. */
export interface SeedParam {
  name: string;
  value: number;
  /** Value at which the subsystem no longer meets its duty. */
  failure: number;
  /** How far from nominal towards failure, 0 to 1. */
  consumed: number;
}

/** What one projection produced. Mirrors `project::Projection`. */
export interface Projection {
  profile: string;
  from_t_s: number;
  horizon_s: number;
  sample_s: number;
  t_s: number[];
  altitude_ft: number[];
  series: ProjectedSeries[];
  /** Soonest first. Empty means the engine holds the leg. */
  exceedances: Exceedance[];
  fuel_burn_l: number;
  /** Measured on the request, not quoted from a benchmark. */
  speed_x: number;
  wall_ms: number;
  /** The health estimate this was seeded with, worst first. */
  seed_health: SeedParam[];
}

/** A profile the daemon will fly, and how far forward it is worth flying. */
export interface Preset {
  /** Name the API takes. */
  id: string;
  label: string;
  /** What the profile does, in the fewest words that distinguish it. */
  note: string;
  hours: number;
}

/**
 * The problem statement's four scenarios, plus the operating point they are read
 * against.
 *
 * Each horizon is the shortest one that shows the profile's whole argument, so a
 * projection costs the least wall clock that answers the question. The high
 * altitude sweep is 20 minutes because that is its key point table; the
 * transients profile is 5 minutes for the same reason. Cruise and endurance are
 * open ended, so their horizons are a leg rather than a table.
 *
 * Wall clock is the binding constraint on the two open ended ones, not physics.
 * The daemon runs about 2,000x real on this machine while it is also flying the
 * simulator and serving the feed, so a four hour cruise is seven seconds and an
 * eight hour one is fifteen. Cruise is the horizon this screen opens on, and a
 * screen whose first paint costs twelve seconds argues against the number it is
 * there to show.
 */
export const PRESETS: Preset[] = [
  { id: "cruise", label: "CRUISE", note: "22.4 kft · the rating point", hours: 2 },
  { id: "high-altitude", label: "HIGH ALTITUDE", note: "2 to 30 kft and back", hours: 0.34 },
  { id: "endurance", label: "ENDURANCE", note: "18 kft · low power", hours: 4 },
  { id: "hot-weather", label: "HOT WEATHER", note: "sea level · ISA+30 · full power", hours: 1 },
  { id: "transients", label: "TRANSIENTS", note: "repeated fuelling steps", hours: 0.09 },
];

/** Raised when the twin has no estimate to project from. */
export class NoEstimateError extends Error {
  constructor() {
    super("the twin has no estimate to project from");
    this.name = "NoEstimateError";
  }
}

/** Fly the engine the twin currently believes in through `preset`. */
export async function project(preset: Preset): Promise<Projection> {
  const query = new URLSearchParams({ profile: preset.id, hours: String(preset.hours) });
  const response = await fetch(`/api/project?${query.toString()}`);
  if (response.status === 409) throw new NoEstimateError();
  if (!response.ok) throw new Error(`projecting ${preset.id}: ${response.status}`);
  return decode(await response.arrayBuffer()) as Projection;
}
