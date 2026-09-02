/**
 * The twenty-two channels the twin compares, described for display.
 *
 * Mirrors `channels::TABLE` in `twin-core` by hand, the same way `TWIN` and
 * `SUBSYSTEMS` in `lib/telemetry.ts` mirror their Rust counterparts. Order is the
 * measurement vector's order and every array in `TwinOutput` is indexed by it, so
 * a row inserted on the Rust side has to be inserted here at the same position.
 *
 * `scale` exists because the filter works in SI and an operator does not read
 * 130,200 Pa. It multiplies the predicted value, the residual and the standard
 * deviation alike, so a cell's three numbers stay in one unit.
 */

import { CYLINDERS, type SourceAges } from "@/lib/telemetry";

/** One compared channel, as a panel needs it. */
export interface Compared {
  /** Name as `twin-core` spells it, which is also the matrix's column label. */
  readonly name: string;
  /** Unit after {@link scale}, rendered small and dim beside the value. */
  readonly unit: string;
  /** Decimal places for the value and the residual. */
  readonly dp: number;
  /** SI to display. Applied to prediction, residual and sigma together. */
  readonly scale: number;
  /**
   * Source whose silence freezes this channel.
   *
   * Air mass flow and turbocharger speed ride the vendor auxiliary message and
   * can go stale while rpm and the temperatures keep arriving, so a cell drawing
   * either of them has to consult its own source rather than the frame's.
   */
  readonly source: keyof SourceAges;
  /**
   * A caption naming what the channel physically is.
   *
   * Worth its width here and nowhere else: TWIN is the screen where someone is
   * being asked whether to believe a disagreement, and `TORQUE` alone does not
   * say that nothing on the bus measures it.
   */
  readonly note: string;
}

const PA_PER_HPA = 100;
const PA_PER_BAR = 1e5;

function egt(i: number): Compared {
  return {
    name: `EGT ${i}`,
    unit: "K",
    dp: 0,
    scale: 1,
    source: "engine_ms",
    note: `cyl ${i} exhaust gas`,
  };
}

function cht(i: number): Compared {
  return {
    name: `CHT ${i}`,
    unit: "K",
    dp: 0,
    scale: 1,
    source: "engine_ms",
    note: `cyl ${i} head`,
  };
}

function lambda(i: number): Compared {
  return {
    name: `LAMBDA ${i}`,
    unit: "",
    dp: 2,
    scale: 1,
    source: "engine_ms",
    // Not a probe. The controller computes it, which is why its residual is
    // allowed to be several percent before anything is said about it.
    note: `cyl ${i} excess air, controller-derived`,
  };
}

const range = (n: number) => Array.from({ length: n }, (_, i) => i + 1);

export const COMPARED: readonly Compared[] = [
  {
    name: "RPM",
    unit: "rpm",
    dp: 0,
    scale: 1,
    source: "engine_ms",
    note: "crankshaft speed",
  },
  {
    name: "MAP",
    unit: "hPa",
    dp: 0,
    scale: 1 / PA_PER_HPA,
    source: "engine_ms",
    note: "intake manifold absolute",
  },
  {
    name: "MAT",
    unit: "K",
    dp: 1,
    scale: 1,
    source: "engine_ms",
    note: "intake manifold, post intercooler",
  },
  {
    name: "MAF",
    unit: "kg/s",
    dp: 3,
    scale: 1,
    source: "auxiliary_ms",
    note: "compressor inlet mass flow",
  },
  {
    name: "TURBO",
    unit: "rpm",
    dp: 0,
    scale: 1,
    source: "auxiliary_ms",
    note: "turbocharger shaft speed",
  },
  {
    name: "TORQUE",
    unit: "N·m",
    dp: 1,
    scale: 1,
    // Reconstructed in the core from broadcast load and speed, so it freezes
    // with the engine message that carries both.
    source: "engine_ms",
    note: "crank, from load and speed",
  },
  {
    name: "FUEL",
    // Kilograms, not the litres the strips show. This is the channel the filter
    // compares, and converting it would put a fuel density between the residual
    // and the number printed beside it.
    unit: "kg/h",
    dp: 2,
    scale: 1,
    source: "engine_ms",
    note: "total delivered, common rail",
  },
  {
    name: "OIL P",
    unit: "bar",
    dp: 2,
    scale: 1 / PA_PER_BAR,
    source: "engine_ms",
    note: "gallery pressure",
  },
  {
    name: "OIL T",
    unit: "K",
    dp: 1,
    scale: 1,
    source: "engine_ms",
    note: "sump, post cooler",
  },
  {
    name: "COOLANT",
    unit: "K",
    dp: 1,
    scale: 1,
    source: "engine_ms",
    note: "radiator outlet",
  },
  ...range(CYLINDERS).map(egt),
  ...range(CYLINDERS).map(cht),
  ...range(CYLINDERS).map(lambda),
];

const BY_NAME = new Map(COMPARED.map((c, i) => [c.name, i]));

/**
 * Registry ids from `store/frame.ts` that name the same physical quantity.
 *
 * OPS works in registry ids and the twin works in measurement vector names, and
 * a drill-down from one to the other has to cross that seam. Mapping here rather
 * than renaming either side keeps each name right where it is read: `egt3` is a
 * ring buffer key, `EGT 3` is what the filter calls the channel it compares.
 *
 * Deliberately partial. Boost, load, wastegate, vibration and bus voltage are
 * displayed on OPS and are not compared against the physics, so following one of
 * them here would be following a channel this screen has nothing to say about.
 */
const ALIAS = new Map<string, string>([
  ["rpm", "RPM"],
  ["map", "MAP"],
  ["maf", "MAF"],
  ["tc_rpm", "TURBO"],
  ["fuel_flow", "FUEL"],
  ["oil_p", "OIL P"],
  ["oil_t", "OIL T"],
  ["coolant_t", "COOLANT"],
  ...range(CYLINDERS).flatMap((i): [string, string][] => [
    [`egt${i}`, `EGT ${i}`],
    [`cht${i}`, `CHT ${i}`],
    [`lambda${i}`, `LAMBDA ${i}`],
  ]),
]);

/**
 * Resolve a selection to a compared channel index, or null.
 *
 * Accepts either spelling, because `Selection.channel` is written by whichever
 * screen navigated here. A name that resolves to neither is a stale link rather
 * than an error, so the caller falls back to its default instead of throwing.
 */
export function comparedIndex(name: string | null): number | null {
  if (name === null) return null;
  const direct = BY_NAME.get(name);
  if (direct !== undefined) return direct;
  const aliased = ALIAS.get(name);
  return aliased === undefined ? null : (BY_NAME.get(aliased) ?? null);
}
