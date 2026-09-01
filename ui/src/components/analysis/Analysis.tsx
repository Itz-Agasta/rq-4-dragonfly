/**
 * ANALYSIS: what is wrong, why, and how long it has.
 *
 * Diagnosis column fixed at 300px, prognosis at 340px, matrix and attribution
 * fluid between them. The same fixed-sides-fluid-centre arrangement OPS uses, and
 * for the same reason: the matrix is the thing that must grow with the viewport,
 * because it is 22 columns wide and everything else is a list.
 */

import { Attribution } from "@/components/analysis/Attribution";
import { DetectionBar } from "@/components/analysis/DetectionBar";
import { Hypotheses } from "@/components/analysis/Hypotheses";
import { Matrix } from "@/components/analysis/Matrix";
import { Prognosis } from "@/components/analysis/Prognosis";
import { useMatrix } from "@/components/analysis/signatures";

export function Analysis() {
  const data = useMatrix();

  if (!data) {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <span className="label-micro">loading the signature matrix</span>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <DetectionBar />
      <div className="lattice min-h-0 min-w-0 flex-1 grid-cols-[300px_minmax(0,1fr)_340px]">
        <Hypotheses data={data} />
        <div className="flex min-h-0 min-w-0 flex-col">
          <div className="min-h-0 flex-1">
            <Matrix data={data} />
          </div>
          <div className="h-[300px] shrink-0">
            <Attribution channels={data.channels} />
          </div>
        </div>
        <Prognosis parameters={data.parameters} />
      </div>
    </div>
  );
}
