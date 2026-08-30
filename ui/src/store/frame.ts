/**
 * The channel registry: every scalar the interface can display, described once.
 *
 * A channel knows its label, its unit, how many decimals it is worth, how to read
 * itself out of a frame, and which bus source is responsible for it. That last
 * part is the reason this file exists rather than each panel reaching into the
 * frame directly: freshness is per source, not per frame, and a panel that
 * renders a value without knowing whose silence would freeze it cannot say when
 * it has gone stale.
 */

import { kelvinToCelsius, metresToFeet, msToKnots } from "@/lib/fmt";
import { CYLINDERS, type Frame, type SourceAges } from "@/lib/telemetry";

/** Which bus source last updated a channel. Keys of {@link SourceAges}. */
export type SourceKey = keyof SourceAges;

export interface Channel {
  /** Stable id. Also the ring-buffer key. */
  readonly id: string;
  /** Short label, as it appears above a readout. */
  readonly label: string;
  /** Unit, rendered small and dim beside the value. */
  readonly unit: string;
  /** Decimal places. */
  readonly dp: number;
  /** The source whose age determines whether this value is live. */
  readonly source: SourceKey;
  /** Read the value out of a frame, already in display units. */
  readonly get: (f: Frame) => number;
}

const PA_PER_HPA = 100;
const PA_PER_BAR = 1e5;

function cylinder(
  prefix: string,
  label: string,
  unit: string,
  dp: number,
  pick: (f: Frame) => number[],
): Channel[] {
  return Array.from({ length: CYLINDERS }, (_, i) => ({
    id: `${prefix}${i + 1}`,
    label: `${label} ${i + 1}`,
    unit,
    dp,
    source: "engine_ms" as SourceKey,
    get: (f: Frame) => pick(f)[i] ?? Number.NaN,
  }));
}

/**
 * Every channel, keyed by id.
 *
 * Turbocharger speed, mass air flow, wastegate position and the vibration
 * features come from the vendor auxiliary message rather than the standard engine
 * status, which is why they carry a different source and can go stale
 * independently of rpm and the temperatures.
 */
export const CHANNELS: Readonly<Record<string, Channel>> = Object.fromEntries(
  [
    {
      id: "rpm",
      label: "RPM · CRANK",
      unit: "rpm",
      dp: 0,
      source: "engine_ms",
      get: (f) => f.rpm,
    },
    {
      id: "map",
      label: "MAP · INTAKE",
      unit: "hPa",
      dp: 0,
      source: "engine_ms",
      get: (f) => f.map_pa / PA_PER_HPA,
    },
    {
      id: "boost",
      label: "BOOST",
      unit: "bar",
      dp: 2,
      source: "engine_ms",
      get: (f) => f.boost_pa / PA_PER_BAR,
    },
    {
      id: "oil_p",
      label: "OIL PRESSURE",
      unit: "bar",
      dp: 2,
      source: "engine_ms",
      get: (f) => f.oil_p_pa / PA_PER_BAR,
    },
    {
      id: "oil_t",
      label: "OIL TEMP",
      unit: "K",
      dp: 0,
      source: "engine_ms",
      get: (f) => f.oil_t_k,
    },
    {
      id: "coolant_t",
      label: "COOLANT",
      unit: "K",
      dp: 0,
      source: "engine_ms",
      get: (f) => f.coolant_t_k,
    },
    {
      id: "lambda",
      label: "LAMBDA",
      unit: "",
      dp: 2,
      source: "engine_ms",
      get: (f) => f.lambda,
    },
    {
      id: "fuel_flow",
      label: "FUEL FLOW",
      unit: "L/h",
      dp: 1,
      source: "engine_ms",
      get: (f) => f.fuel_flow_lph,
    },
    {
      id: "load",
      label: "LOAD",
      unit: "%",
      dp: 0,
      source: "engine_ms",
      get: (f) => f.load_pct,
    },
    {
      id: "tc_rpm",
      label: "TURBO SPEED",
      unit: "rpm",
      dp: 0,
      source: "auxiliary_ms",
      get: (f) => f.tc_rpm,
    },
    {
      id: "maf",
      label: "MAF · COMPRESSOR IN",
      unit: "kg/s",
      dp: 3,
      source: "auxiliary_ms",
      get: (f) => f.maf_kgs,
    },
    {
      id: "wastegate",
      label: "WASTEGATE",
      unit: "",
      dp: 2,
      source: "auxiliary_ms",
      get: (f) => f.wastegate,
    },
    {
      id: "vib",
      label: "VIBRATION RMS",
      unit: "g",
      dp: 2,
      source: "auxiliary_ms",
      get: (f) => f.vib_rms_g,
    },
    {
      id: "bus_v",
      label: "BUS",
      unit: "V",
      dp: 1,
      source: "power_ms",
      get: (f) => f.bus_v,
    },
    {
      id: "altitude",
      label: "ALT",
      unit: "ft",
      dp: 0,
      source: "air_data_ms",
      get: (f) => metresToFeet(f.altitude_m),
    },
    {
      id: "oat",
      label: "OAT",
      unit: "°C",
      dp: 0,
      source: "air_data_ms",
      get: (f) => kelvinToCelsius(f.oat_k),
    },
    {
      id: "ias",
      label: "IAS",
      unit: "kt",
      dp: 0,
      source: "air_data_ms",
      get: (f) => msToKnots(f.ias_ms),
    },
    ...cylinder("egt", "EGT", "K", 0, (f) => f.egt_k),
    ...cylinder("cht", "CHT", "K", 0, (f) => f.cht_k),
    ...cylinder("lambda", "λ", "", 2, (f) => f.lambda_k),
  ].map((c) => [c.id, c as Channel]),
);

/** Every channel id that gets a ring buffer. */
export const RECORDED: readonly string[] = Object.keys(CHANNELS);

/** Look a channel up, or throw at startup rather than render `undefined`. */
export function channel(id: string): Channel {
  const c = CHANNELS[id];
  if (!c) throw new Error(`unknown channel: ${id}`);
  return c;
}
