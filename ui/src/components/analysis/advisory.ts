/**
 * What to do about each fault the library can name.
 *
 * MOCK: the four fields per row are authored maintenance text, replaced by the
 * operator's own task cards when there are any. What selects the row is live: the
 * hypothesis index comes from the posterior, and the remaining life beside it
 * comes from the fit. `docs/mvp.md` 1 licenses a templated advisory fed by real
 * values, and the panel carries a `◊ INFERRED` tag so nothing here is mistaken for
 * a manufacturer's schedule.
 *
 * The field set is the artboard's: action, DUR, PARTS, RISK. Durations and part
 * numbers are **estimated** and class-typical for a 2-litre common-rail diesel;
 * EASA TCDS E.200 publishes neither.
 *
 * The `risk` lines are the exception and are not authored freely. Each is the
 * consequence this engine model actually produces when the fault runs, which is
 * why they differ in kind rather than in severity: a lying thermocouple damages
 * nothing and a dry gallery damages everything. The artboard's own risk line
 * quotes figures the measurements do not support, and `handover.md` 4 says prefer
 * the measured set wherever the two disagree.
 */

/** Rows are indexed by hypothesis, in `HYPOTHESES` order. */
export interface Task {
  /** What to do, one line. */
  action: string;
  /** Time on the aircraft and where the work happens. **estimated** */
  duration: string;
  /** What to have on the shelf first. **estimated** */
  parts: string;
  /** What happens if the mission continues instead. */
  risk: string;
}

export const TASKS: readonly (Task | null)[] = [
  null,
  {
    action: "Replace injector · cyl 3",
    duration: "2.5\u00a0h · line-replaceable",
    parts: "INJ-CR-4B ×1 · seal kit SK-119",
    risk: "If deferred: the nozzle passes less each hour and the smoke limiter clips rated power",
  },
  {
    action: "Compression test · cyl 3",
    duration: "3.0\u00a0h · cylinder access",
    parts: "GP-11 glow plug · harness HN-3C",
    risk: "If deferred: unburnt fuel washes the bore and dilutes the oil; boost falls with turbine inlet",
  },
  {
    action: "Replace thermocouple · EGT-3",
    duration: "0.75\u00a0h · line-replaceable",
    parts: "TC-K-207 ×1 · gasket G-44",
    risk: "If deferred: no damage accrues, but one reading is wrong in a known direction",
  },
  {
    action: "Back-flush radiator core",
    duration: "1.5\u00a0h · cowling off",
    parts: "coolant 6 L · core seals CS-8",
    risk: "If deferred: coolant margin narrows; the 100 C limit is met low and hot, not at altitude",
  },
  {
    action: "Borescope compressor wheel",
    duration: "2.0\u00a0h · intake off",
    parts: "filter element AF-22",
    risk: "If deferred: boost falls away earlier and the critical altitude knee moves down",
  },
  {
    action: "Borescope turbine · check shaft play",
    duration: "2.0\u00a0h · exhaust off",
    parts: "cartridge TC-330, if play is out of limits",
    risk: "If deferred: the same lapse as compressor erosion; no outlet temperature here, so the pair is named as a pair",
  },
  {
    action: "Check oil pick-up and relief valve",
    duration: "1.0\u00a0h · sump access",
    parts: "oil 5 L · filter OF-19",
    risk: "If deferred: gallery pressure falls through the 2.5 bar TCDS floor, bearings unprotected",
  },
  {
    action: "Replace intake filter element",
    duration: "0.5\u00a0h · line-replaceable",
    parts: "filter element AF-22",
    risk: "If deferred: volumetric efficiency falls and power lapses earlier with altitude",
  },
];
