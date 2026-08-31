/**
 * The bottom telemetry block: a clean 3x2 of streaming strips.
 *
 * Six cells, three per row, so column boundaries line up across both rows and the
 * lattice grammar holds. Nothing else lives in this grid — the fault-injection
 * control sits in the schematic header, because putting it here would make one
 * row four cells wide and break the alignment.
 *
 * All six share a cursor key, so hovering any strip puts the same instant under
 * the cursor on all of them.
 */

import { FAULT_CYLINDER } from "@/components/ops/data";
import { Strip, type StripSeries } from "@/components/Strip";
import { CYLINDERS, type Frame } from "@/lib/telemetry";

const SYNC = "ops";

/**
 * Per-cylinder series with one cylinder brought forward.
 *
 * Luminance carries which channel is selected: the cylinder of interest is drawn
 * at full weight and its neighbours are ghosted. Colour is not spent here.
 */
function cylinderSeries(prefix: string, focus: number): StripSeries[] {
  const others = Array.from({ length: CYLINDERS }, (_, i) => i + 1)
    .filter((n) => n !== focus)
    .map((n) => ({ id: `${prefix}${n}`, emphasis: "ghost" as const }));
  // Only the focused cylinder gets a twin trace. Four dashed lines that lie on
  // top of each other until one cylinder departs is four times the ink for the
  // same information, and it buries the pair an operator is being asked to read.
  //
  // Predicted is drawn last, so it lands on top. Underneath it is invisible
  // wherever the twin is locked, because a 2px measured trace covers a 1.5px line
  // at the same height completely; the strip would then promise a comparison and
  // show one line. Dashed over solid is legible whether they agree or not.
  return [
    ...others,
    { id: `${prefix}${focus}`, emphasis: "measured" as const },
    { id: `${prefix}${focus}_twin`, emphasis: "predicted" as const },
  ];
}

/**
 * How far a cylinder may sit from its neighbours before the readout alarms, K.
 *
 * Deviation from the mean of the other cylinders is computable from measurement
 * alone — it needs no twin — and it is what a per-cylinder exhaust spread gauge
 * has always shown. Until a fault exists this evaluates to roughly zero and
 * nothing is accented, which is the honest state.
 */
const SPREAD_LIMIT_K = 25;

function spreadAlarm(pick: (f: Frame) => number[], cylinder: number) {
  return (frame: Frame): boolean => {
    const values = pick(frame);
    const self = values[cylinder - 1];
    if (self === undefined || !Number.isFinite(self)) return false;
    const rest = values.filter((_, i) => i !== cylinder - 1).filter(Number.isFinite);
    if (rest.length === 0) return false;
    const mean = rest.reduce((a, b) => a + b, 0) / rest.length;
    return Math.abs(self - mean) > SPREAD_LIMIT_K;
  };
}

/*
 * Minimum plot spans, passed to each strip below. Each is roughly twice the
 * channel's steady-state noise, so cruise reads flat and an excursion worth
 * looking at fills a useful fraction of the panel. EGT's 60 K is set so the
 * per-cylinder spread a coking injector produces is unmissable without being
 * clipped.
 */
const EGT_SERIES = cylinderSeries("egt", FAULT_CYLINDER);
const CHT_SERIES = cylinderSeries("cht", FAULT_CYLINDER);
const RPM_SERIES: StripSeries[] = [{ id: "rpm" }, { id: "rpm_twin", emphasis: "predicted" }];
const MAP_SERIES: StripSeries[] = [{ id: "map" }, { id: "map_twin", emphasis: "predicted" }];
const OIL_SERIES: StripSeries[] = [{ id: "oil_p" }, { id: "oil_p_twin", emphasis: "predicted" }];
// Vibration carries no twin trace, and that is not an omission. Nothing in a mean
// value engine model produces a crankcase acceleration, so there is no prediction
// to draw and a dashed line here would be an invention.
const VIB_SERIES: StripSeries[] = [{ id: "vib" }];

const egtAlarm = spreadAlarm((f) => f.egt_k, FAULT_CYLINDER);
const chtAlarm = spreadAlarm((f) => f.cht_k, FAULT_CYLINDER);

export function Strips() {
  return (
    <div className="border-border grid min-h-0 shrink-0 basis-[26%] grid-cols-3 grid-rows-2 border-t">
      <div className="border-border min-h-0 min-w-0 border-r border-b">
        <Strip title="RPM · crank" readout="rpm" series={RPM_SERIES} syncKey={SYNC} minSpan={60} />
      </div>
      <div className="border-border min-h-0 min-w-0 border-r border-b">
        <Strip title="MAP · intake" readout="map" series={MAP_SERIES} syncKey={SYNC} minSpan={40} />
      </div>
      <div className="border-border min-h-0 min-w-0 border-b">
        <Strip
          title={`EGT · cyl 1–${CYLINDERS}`}
          readout={`egt${FAULT_CYLINDER}`}
          series={EGT_SERIES}
          syncKey={SYNC}
          minSpan={60}
          alarmWhen={egtAlarm}
        />
      </div>
      <div className="border-border min-h-0 min-w-0 border-r">
        <Strip
          title={`CHT · cyl 1–${CYLINDERS}`}
          readout={`cht${FAULT_CYLINDER}`}
          series={CHT_SERIES}
          syncKey={SYNC}
          minSpan={24}
          alarmWhen={chtAlarm}
        />
      </div>
      <div className="border-border min-h-0 min-w-0 border-r">
        <Strip
          title="Oil pressure"
          readout="oil_p"
          series={OIL_SERIES}
          syncKey={SYNC}
          minSpan={0.4}
        />
      </div>
      <div className="min-h-0 min-w-0">
        <Strip
          title="Vibration RMS"
          readout="vib"
          series={VIB_SERIES}
          syncKey={SYNC}
          minSpan={0.5}
        />
      </div>
    </div>
  );
}
