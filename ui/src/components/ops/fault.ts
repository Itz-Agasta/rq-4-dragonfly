/**
 * Which cylinder the engine is disagreeing on, measured rather than authored.
 *
 * Replaces a `FAULT_CYLINDER = 3` constant that was correct for exactly one
 * demonstration and wrong the moment the fault moved.
 *
 * # Why the residual and not the diagnosis
 *
 * The diagnosis names a cylinder in its hypothesis string, and reading it would
 * mean parsing a digit out of `INJECTOR 3 COKING`. The per-cylinder residuals say
 * the same thing without the string handling, and they say it about the
 * measurement rather than about the conclusion drawn from it. That difference
 * matters on a lying probe: with `EGT 3 SENSOR DRIFT` the engine is healthy and
 * the instrument is not, and the honest thing for a schematic to mark is where
 * the disagreement sits. The advisory panel is what says whether to open the
 * engine or distrust the reading.
 *
 * # Gated on detection, for the reason attribution is
 *
 * Ranking residuals on a healthy engine ranks noise, and the accented bore would
 * wander between the four cylinders several times a minute. It answers *which*
 * cylinder and never *whether* there is one.
 */

import { useRef, useState } from "react";

import { useLiveSink } from "@/lib/live";
import { CYLINDERS, isFresh, TWIN } from "@/lib/telemetry";

/** Per-cylinder channel blocks, each four entries long, in measurement order. */
const BLOCKS = [TWIN.EGT, TWIN.CHT, TWIN.LAMBDA];

/**
 * How much worse the leading cylinder has to be before the mark moves to it.
 *
 * Without it two cylinders sitting at a similar residual trade the accent back
 * and forth, and a mark that moves is read as a fault that is moving.
 */
const MARGIN = 1.2;

/** The 1-based cylinder to mark, or 0 for none. */
export function useFaultCylinder(): number {
  const [cylinder, setCylinder] = useState(0);
  const current = useRef(0);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const fired = twin?.detection.drift === true || twin?.detection.anomaly === true;
    if (!twin || !fired || !isFresh(frame.ages.engine_ms)) {
      if (current.current !== 0) {
        current.current = 0;
        setCylinder(0);
      }
      return;
    }

    // Summed across the three per-cylinder channels rather than taking the
    // largest, so a cylinder that is out on exhaust temperature, head
    // temperature and excess air together outranks one that is out on a single
    // channel. That is the shape of a real cylinder fault.
    let best = 0;
    let bestScore = 0;
    let runnerUp = 0;
    for (let c = 0; c < CYLINDERS; c += 1) {
      let score = 0;
      for (const base of BLOCKS) {
        const v = twin.normalised[base + c];
        if (v !== undefined && Number.isFinite(v)) score += Math.abs(v);
      }
      if (score > bestScore) {
        runnerUp = bestScore;
        bestScore = score;
        best = c + 1;
      } else if (score > runnerUp) {
        runnerUp = score;
      }
    }

    const decided = bestScore > runnerUp * MARGIN ? best : current.current;
    if (decided !== current.current) {
      current.current = decided;
      setCylinder(decided);
    }
  });

  return cylinder;
}
