/**
 * OPS: the live operations screen.
 *
 * Health rail fixed at 220px, right column fixed at 420px, schematic fluid
 * between them, telemetry block across the bottom. Fixed side columns and a
 * fluid centre is what lets the layout hold from 1600x900 to 2560x1440 without
 * anything truncating.
 */

import { Advisory } from "@/components/ops/Advisory";
import { AlertStack } from "@/components/ops/AlertStack";
import { HealthRail } from "@/components/ops/HealthRail";
import { Schematic } from "@/components/ops/Schematic";
import { Strips } from "@/components/ops/Strips";

export function Ops() {
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex min-h-0 min-w-0 flex-1 items-stretch">
        <HealthRail />
        <Schematic />
        <aside className="border-border flex w-[420px] min-w-0 shrink-0 flex-col border-l">
          <AlertStack />
          <Advisory />
        </aside>
      </div>
      <Strips />
    </div>
  );
}
