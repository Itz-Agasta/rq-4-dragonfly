/**
 * SIMULATE: the engine on the wing, flown through the rest of a mission.
 *
 * What separates this from a second run of the simulator is the seed. The daemon
 * starts the model from the state and the health parameters the estimator
 * currently holds, so the engine projected is the degraded one being watched
 * rather than a healthy one flying the same profile. With no estimate there is
 * nothing to project and the screen says so; it does not fall back to a nominal
 * engine, which would look identical and mean something else entirely.
 *
 * **Departure from `design.md` 6.** The artboard puts the four presets in REPLAY
 * as a launcher, on the reading that SIMULATE re-simulates a recorded mission and
 * so needs no controls of its own. Seeding from the live twin makes that wrong:
 * the profile is the only thing this screen has to be told, a recording is not
 * where that choice belongs, and routing it through another screen costs a
 * navigation and a piece of shared state to save one row of buttons.
 */

import { useCallback, useEffect, useState } from "react";

import { Channels } from "@/components/simulate/Channels";
import { Exceedances } from "@/components/simulate/Exceedances";
import { Outlook } from "@/components/simulate/Outlook";
import { Button } from "@/components/ui/button";
import { NoEstimateError, PRESETS, type Preset, type Projection, project } from "@/lib/projection";
import { report } from "@/lib/report";

type Status = "running" | "ready" | "no-estimate" | "error";

export function Simulate() {
  const [preset, setPreset] = useState<Preset>(PRESETS[0]!);
  const [status, setStatus] = useState<Status>("running");
  const [projection, setProjection] = useState<Projection | null>(null);
  const [error, setError] = useState("");

  // The fetch alone, with no synchronous state write in it, so the effect below
  // starts a request rather than starting another render.
  const load = useCallback((next: Preset) => {
    let cancelled = false;
    project(next)
      .then((result) => {
        if (cancelled) return;
        setProjection(result);
        setStatus("ready");
      })
      .catch((cause: unknown) => {
        if (cancelled) return;
        if (cause instanceof NoEstimateError) {
          setStatus("no-estimate");
          return;
        }
        report("projecting a mission", cause);
        setError(cause instanceof Error ? cause.message : String(cause));
        setStatus("error");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Pressing a profile is what changes the screen's mind about which one it is
  // showing; the load is the same either way.
  const choose = (next: Preset) => {
    setPreset(next);
    setStatus("running");
    load(next);
  };

  // The rating point on arrival, so the screen has an answer rather than an
  // invitation. Every other profile is one press away. `status` already starts
  // at "running", so nothing needs setting here.
  useEffect(() => load(PRESETS[0]!), [load]);

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="border-border flex h-9 shrink-0 items-center gap-1 border-b px-3">
        <span className="t-section text-muted-foreground mr-2">PROFILE</span>
        {PRESETS.map((one) => (
          <Button
            key={one.id}
            size="sm"
            variant={one.id === preset.id ? "outline" : "ghost"}
            aria-pressed={one.id === preset.id}
            disabled={status === "running"}
            onClick={() => choose(one)}
          >
            {one.label}
          </Button>
        ))}
        <span className="label-micro ml-auto normal-case">{preset.note}</span>
      </div>

      {status === "ready" && projection ? (
        <div className="flex min-h-0 min-w-0 flex-1 items-stretch">
          <Outlook projection={projection} preset={preset} />
          <Channels projection={projection} />
          <Exceedances projection={projection} />
        </div>
      ) : (
        <Waiting status={status} error={error} label={preset.label} />
      )}
    </div>
  );
}

function Waiting({ status, error, label }: { status: Status; error: string; label: string }) {
  const [title, note] =
    status === "running"
      ? [`PROJECTING ${label}`, "running the model forward from the twin's current estimate"]
      : status === "no-estimate"
        ? [
            "NO TWIN ESTIMATE",
            "a projection starts from the engine the filter believes in; there is nothing to start from until the twin locks",
          ]
        : ["PROJECTION FAILED", error];

  return (
    <div className="flex min-h-0 flex-1 items-center justify-center">
      <div className="max-w-[420px] text-center">
        <div className="t-section text-muted-foreground">{title}</div>
        <div className="label-micro mt-2 leading-[1.5] normal-case">{note}</div>
      </div>
    </div>
  );
}
