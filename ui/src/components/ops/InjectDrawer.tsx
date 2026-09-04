/**
 * The fault injection drawer.
 *
 * **This commands the simulator, not an engine.** Nothing on a real aircraft
 * would accept a message that damages it, and the panel says so rather than
 * leaving it to be inferred. It exists because a demonstration has to be able to
 * break the engine while somebody is watching the twin catch it.
 *
 * The command goes `POST /api/fault` to the core, which publishes a DroneCAN
 * vendor frame onto the same bus the telemetry arrives on, three times with one
 * sequence number. Routing it over the bus rather than a side channel means
 * there is one transport to explain and the CAN link is visibly bidirectional.
 *
 * # The cylinder selector outruns the diagnosis, and says so
 *
 * Detection works on any cylinder: the residual, the CUSUM and the alert stack
 * all name whichever channel moved. **Isolation does not.** `twin-core`'s
 * hypothesis catalogue generates its per-cylinder rows for cylinder 3 only, so a
 * fault injected elsewhere is caught and reported but cannot be named, and the
 * posterior spreads instead of landing. Measured on the bus: a misfire commanded
 * on cylinder 2 gave EGT 2 at -6.01 sigma with the drift alarm firing, and a
 * diagnosis of NOMINAL at 44.8%.
 *
 * The row under the selector says this rather than the selector being limited to
 * one cylinder, because being able to show detection generalising is worth more
 * than hiding that isolation has not been generated for the other three yet.
 *
 * # Presets, not a form
 *
 * Each row is one press. A stage demonstration is the wrong place to be choosing
 * a ramp constant from a slider, and every value here is stated in the row so
 * nothing about the injected fault is hidden behind a control.
 */

import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { report } from "@/lib/report";
import { CYLINDERS } from "@/lib/telemetry";

/** Mirrors `FaultKind` in `dronecan-ice`. The wire carries the number. */
const KIND = {
  clear: 0,
  coking: 1,
  misfire: 2,
  drift: 3,
  freeze: 4,
  cooling: 5,
} as const;

interface Preset {
  kind: number;
  label: string;
  /** What the fault does, and every number the command carries. */
  note: string;
  /** Whether the row offers a cylinder choice. */
  perCylinder: boolean;
  severity: number;
  ramp_s: number;
}

/**
 * The four faults the library models, plus the two that exercise the instrument
 * path.
 *
 * Severities are the demonstration values, not the physical extremes: coking to
 * 72% of nominal flow is what `docs/fault_signatures.md` was generated against,
 * and the drift rate is the signal-chain fault rather than probe oxidation, which
 * is far too slow to see inside a demonstration.
 */
const PRESETS: readonly Preset[] = [
  {
    kind: KIND.coking,
    label: "Injector coking",
    note: "nozzle to 72% of nominal flow over 30 min",
    perCylinder: true,
    severity: 0.72,
    // Must outlast the demonstration. A ramp that settles inside it leaves the
    // parameter constant, so there is no decline to fit and the remaining life
    // vanishes mid-run. Detection does not pay for the slower ramp; handover 9.4.
    ramp_s: 1800,
  },
  {
    kind: KIND.misfire,
    label: "Cylinder misfire",
    note: "18% of firings fail, over 1 min",
    perCylinder: true,
    severity: 0.18,
    ramp_s: 60,
  },
  {
    kind: KIND.drift,
    label: "Exhaust probe drift",
    note: "600 K/h on one probe, signal chain not oxidation",
    perCylinder: true,
    severity: 600,
    ramp_s: 0,
  },
  {
    kind: KIND.freeze,
    label: "Exhaust probe freeze",
    note: "probe holds its last sample, variance goes to zero",
    perCylinder: true,
    severity: 0,
    ramp_s: 0,
  },
  {
    kind: KIND.cooling,
    label: "Radiator fouling",
    note: "83% of effectiveness over 5 min, every cylinder together",
    perCylinder: false,
    severity: 0.83,
    ramp_s: 300,
  },
];

export function InjectDrawer() {
  const [cylinder, setCylinder] = useState(3);
  const [sent, setSent] = useState<string | null>(null);

  const inject = (preset: Preset) => {
    const body = {
      kind: preset.kind,
      cylinder: preset.perCylinder ? cylinder : 0,
      severity: preset.severity,
      ramp_s: preset.ramp_s,
    };
    void fetch("/api/fault", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    })
      .then((response) => {
        // The core answers 503 when the queue has backed up, which means the bus
        // is down. Saying so beats a control that looks like it worked.
        setSent(
          response.ok
            ? `${preset.label}${preset.perCylinder ? ` · cyl ${cylinder}` : ""} commanded`
            : "no route to the bus",
        );
      })
      .catch((error: unknown) => {
        report("fault command failed", error);
        setSent("no route to the bus");
      });
  };

  return (
    <Sheet>
      <SheetTrigger asChild>
        <Button size="sm" className="shrink-0">
          INJECT FAULT
        </Button>
      </SheetTrigger>
      <SheetContent side="right" className="w-[420px] sm:max-w-[420px]">
        <SheetHeader>
          <SheetTitle className="t-section">INJECT FAULT</SheetTitle>
          <SheetDescription className="t-small text-muted-foreground">
            Commands the simulator over the CAN bus, the same link the telemetry arrives on. No
            engine accepts these.
          </SheetDescription>
        </SheetHeader>

        <div className="border-border flex flex-col gap-2 border-b px-4 pb-3">
          <div className="flex items-center gap-2">
            <span className="label-micro">cylinder</span>
            {Array.from({ length: CYLINDERS }, (_, i) => i + 1).map((n) => (
              <Button
                key={n}
                size="sm"
                variant={n === cylinder ? "outline" : "ghost"}
                onClick={() => setCylinder(n)}
                aria-pressed={n === cylinder}
              >
                {n}
              </Button>
            ))}
          </div>
          <span className="label-micro leading-[1.4] normal-case">
            Detection works on any cylinder. The hypothesis catalogue is generated for cylinder 3,
            so a fault elsewhere is caught and named by channel but not by cause.
          </span>
        </div>

        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
          {PRESETS.map((preset) => (
            <div
              key={preset.label}
              className="border-border flex items-start justify-between gap-3 border-b px-4 py-3"
            >
              <div className="min-w-0">
                <div className="t-body">
                  {preset.label}
                  {preset.perCylinder ? (
                    <span className="text-foreground-dim"> · cyl {cylinder}</span>
                  ) : null}
                </div>
                <div className="label-micro mt-1 normal-case">{preset.note}</div>
              </div>
              <Button size="sm" className="shrink-0" onClick={() => inject(preset)}>
                INJECT
              </Button>
            </div>
          ))}
        </div>

        <div className="flex items-center justify-between gap-3 px-4 py-3">
          <span className="label-micro min-w-0 truncate normal-case" role="status">
            {sent ?? "nothing commanded this session"}
          </span>
          <Button
            size="sm"
            variant="ghost"
            className="shrink-0"
            onClick={() =>
              inject({
                kind: KIND.clear,
                label: "Clear",
                note: "",
                perCylinder: false,
                severity: 0,
                ramp_s: 0,
              })
            }
          >
            CLEAR ALL
          </Button>
        </div>
      </SheetContent>
    </Sheet>
  );
}
