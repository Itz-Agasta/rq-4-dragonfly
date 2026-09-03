/**
 * The demonstration fleet roster.
 *
 * MOCK: every value below is authored. Replaced by a fleet service aggregating
 * one twin per airframe; the wire shape already exists, one `Frame` per tail
 * plus the seven indices in `lib/telemetry.ts`, so this screen changes its
 * source and not its shape.
 *
 * This station's own airframe carries no authored health figures. Its numbers
 * are live on OPS, and a second authored set is a second answer to one question.
 */

import { AIRFRAME } from "@/components/app/TopBar";
import type { SUBSYSTEMS } from "@/lib/telemetry";

/** Which of the seven indices is worst, named as the rest of the app names it. */
type Subsystem = (typeof SUBSYSTEMS)[number];

/**
 * `station` is this GCS's own aircraft and is not a health state. It is here so
 * the roster can say which row the rest of the app is about.
 */
export type FleetState = "monitored" | "advisory" | "grounded" | "station";

export interface Airframe {
  tail: string;
  /** Operating location, abbreviated. */
  base: string;
  engine: string;
  /** Engine hours since overhaul. */
  hours: number;
  state: FleetState;
  /** Null on a healthy airframe and on this station's own. */
  subsystem: Subsystem | null;
  /** Worst subsystem index, 0 to 100. Null where there is no authored figure. */
  index: number | null;
  /** Remaining useful life at the limiting subsystem, hours. */
  rulHours: number | null;
  /** Sentence case on purpose: it is prose, not a label. */
  advisory: string;
}

export const ROSTER: Airframe[] = [
  {
    tail: "TAPAS-AF03",
    base: "ATR-CTD",
    engine: "E4P-330118",
    hours: 1284,
    state: "monitored",
    subsystem: null,
    index: 97,
    rulHours: null,
    advisory: "Nothing open.",
  },
  {
    tail: "TAPAS-AF05",
    base: "ATR-CTD",
    engine: "E4P-330126",
    hours: 962,
    state: "advisory",
    subsystem: "Air Path",
    index: 78,
    rulHours: 240,
    advisory: "Turbomachine efficiency product drifting. Inspect at next phase.",
  },
  {
    tail: AIRFRAME,
    base: "ATR-CTD",
    engine: "E4P-330131",
    hours: 1147,
    state: "station",
    subsystem: null,
    index: null,
    rulHours: null,
    advisory: "Bound to this station. Health is live on OPS.",
  },
  {
    tail: "TAPAS-AF09",
    base: "AFS-HND",
    engine: "E4P-330140",
    hours: 611,
    state: "monitored",
    subsystem: null,
    index: 99,
    rulHours: null,
    advisory: "Nothing open.",
  },
  {
    tail: "TAPAS-AF11",
    base: "AFS-HND",
    engine: "E4P-330152",
    hours: 1503,
    state: "advisory",
    subsystem: "Thermal",
    index: 71,
    rulHours: 96,
    advisory: "Radiator effectiveness down 11%. Clean core before hot-weather tasking.",
  },
  {
    tail: "TAPAS-AF12",
    base: "AFS-HND",
    engine: "E4P-330155",
    hours: 428,
    state: "monitored",
    subsystem: null,
    index: 98,
    rulHours: null,
    advisory: "Nothing open.",
  },
  {
    tail: "TAPAS-AF14",
    base: "ATR-CTD",
    engine: "E4P-330163",
    hours: 1790,
    state: "grounded",
    subsystem: "Fuel/Injection",
    index: 44,
    rulHours: 12,
    advisory: "Injector 2 coking. Nozzle replacement raised, aircraft withheld.",
  },
  {
    tail: "TAPAS-AF16",
    base: "AFS-HND",
    engine: "E4P-330171",
    hours: 205,
    state: "monitored",
    subsystem: null,
    index: 99,
    rulHours: null,
    advisory: "Nothing open.",
  },
];

/**
 * Counted from `ROSTER` rather than authored beside it.
 *
 * A hand-typed total is the `compared.ts` mirroring defect in miniature: it
 * agrees until a row is added and then quietly does not.
 */
export const TOTALS = {
  airframes: ROSTER.length,
  engineHours: ROSTER.reduce((sum, one) => sum + one.hours, 0),
  advisories: ROSTER.filter((one) => one.state === "advisory").length,
  grounded: ROSTER.filter((one) => one.state === "grounded").length,
};

/** Label and provenance for each state. Order is display order in the legend. */
export const STATE_LABEL: Record<FleetState, string> = {
  station: "THIS STATION",
  monitored: "MONITORED",
  advisory: "ADVISORY",
  grounded: "WITHHELD",
};
