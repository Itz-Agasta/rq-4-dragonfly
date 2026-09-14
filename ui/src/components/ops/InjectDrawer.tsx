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
 * # Presets, not a form, and one confirmation
 *
 * Each row is one press. A stage demonstration is the wrong place to be choosing
 * a ramp constant from a slider, and every value here is stated in the row so
 * nothing about the injected fault is hidden behind a control.
 *
 * The press then has to be confirmed. A fault runs until CLEAR ALL and the slow
 * presets take half an hour to reach severity, so a mis-press costs the
 * demonstration rather than a keystroke.
 *
 * # Confirming closes the drawer, and the answer moves to a toast
 *
 * A 420px sheet covers the schematic and a third of the strips, which is
 * precisely what somebody who just commanded a fault wants to look at. So the
 * drawer closes on confirm and `@/store/toast` carries the confirmation, which
 * outlives it. The footer line that used to report this is gone: at 10px inside
 * a panel the operator was about to shut, it was read by nobody.
 */

import { useEffect, useState } from "react";

import { Field } from "@/components/Field";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { report } from "@/lib/report";
import { CYLINDERS } from "@/lib/telemetry";
import { toast } from "@/store/toast";

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
  /**
   * Which channels move once it has run, `{n}` standing in for the cylinder.
   *
   * **Read off `docs/fault_signatures.md`, not written from intuition.** Each
   * names the channels with a real component in that fault's generated
   * direction. `just signatures` regenerates the table; re-read these when it
   * changes.
   */
  effect: string;
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
    // EGT -0.64, LAMBDA +0.57, FUEL -0.35, TORQUE -0.31, CHT -0.23.
    effect:
      "EGT {n} and CHT {n} fall, lambda {n} rises, fuel flow and torque ease off. No certified limit trips, which is the point.",
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
    // EGT -0.67, LAMBDA +0.62, TORQUE -0.33, and fuel flow at zero: the cylinder
    // is fuelled and burns none of it, which is what separates it from coking.
    effect:
      "EGT {n} drops hard, lambda {n} rises, torque falls. Fuel flow holds, which is what separates it from coking.",
    perCylinder: true,
    severity: 0.18,
    ramp_s: 60,
  },
  {
    kind: KIND.drift,
    label: "Exhaust probe drift",
    note: "600 K/h on one probe, signal chain not oxidation",
    // EGT +1.00 and nothing else. The whole discriminator in one row.
    effect:
      "EGT {n} climbs alone. Torque, fuel and lambda {n} stay where the model puts them, so the cylinder is healthy and the probe is lying.",
    perCylinder: true,
    severity: 600,
    ramp_s: 0,
  },
  {
    kind: KIND.freeze,
    label: "Exhaust probe freeze",
    note: "probe holds its last sample, variance goes to zero",
    effect:
      "EGT {n} stops moving. The value stays plausible while the other three keep breathing, which is why variance rather than level is what catches it.",
    perCylinder: true,
    severity: 0,
    ramp_s: 0,
  },
  {
    kind: KIND.cooling,
    label: "Radiator fouling",
    note: "83% of effectiveness over 5 min, every cylinder together",
    // COOLANT +0.55 and all four CHT at +0.42. Correlation is the signature.
    effect:
      "Coolant temperature and all four head temperatures rise together. One cylinder rising alone is never this.",
    perCylinder: false,
    severity: 0.83,
    ramp_s: 300,
  },
];

/**
 * The clear command, which is the one preset that is not a fault.
 *
 * Confirmed like the rest even though it is the safe direction, because it is
 * not free: an instantaneous jump in a health parameter is not something a
 * filter tracks smoothly, so the twin raises `lost lock` on every clear. It is
 * not quick. Measured on vcan0 after clearing a 18% misfire on cylinder 3: lock
 * returned at 22 s, and the subsystem scores were still climbing at 38 s
 * (combustion 89 of a pre-fault 95). The toast carries that figure, because an
 * operator who clears a fault and sees nothing move for twenty seconds
 * reasonably concludes the command was lost.
 */
const CLEAR: Preset = {
  kind: KIND.clear,
  label: "Clear all faults",
  note: "every cylinder and the radiator back to nominal, at once",
  // The clear card lists what it is ending and says "back to a healthy engine",
  // and stops there. It carries no `expect` row: the twin dropping lock for a few
  // seconds is real, measured, and already on the alert stack when it happens,
  // and spelling it out here made the undo read as more consequential than the
  // fault it undoes.
  effect: "",
  perCylinder: false,
  severity: 0,
  ramp_s: 0,
};

/** The `POST /api/fault` body, which is also what the confirmation card shows. */
function command(preset: Preset, cylinder: number) {
  return {
    kind: preset.kind,
    cylinder: preset.perCylinder ? cylinder : 0,
    severity: preset.severity,
    ramp_s: preset.ramp_s,
  };
}

/**
 * A cylinder with a spark leaving it.
 *
 * Drawn here rather than in `app/glyphs.tsx`, which is the rail's set and is
 * documented as such. Same hand: 1.5px strokes, no fill, no rounded joins.
 */
function InjectGlyph() {
  return (
    <svg
      viewBox="0 0 16 16"
      width="13"
      height="13"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="butt"
      aria-hidden="true"
    >
      <path d="M2.5 5.5h6v8h-6z" />
      <path d="M11 2.5l-2.5 4h4l-2.5 4" />
    </svg>
  );
}

/** One row as the clear card names it back. */
function faultName(preset: Preset, cylinder: number): string {
  return preset.perCylinder ? `${preset.label} · cyl ${cylinder}` : preset.label;
}

export function InjectDrawer() {
  const [open, setOpen] = useState(false);
  const [cylinder, setCylinder] = useState(3);
  /** The preset the confirmation card is holding, or null when it is closed. */
  const [pending, setPending] = useState<Preset | null>(null);
  /**
   * What this drawer has commanded and not yet cleared, so the clear card can
   * name it rather than asking about "every fault".
   *
   * **Commanded, not measured.** Nothing on the wire reports which faults are
   * running, so this is a record of presses and a reload empties it while the
   * faults do not stop.
   */
  const [commanded, setCommanded] = useState<string[]>([]);

  // `F` opens the drawer. Teammates could not find the button on the schematic
  // header, and a key is the affordance that survives someone else driving.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      if (target?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName ?? "")) {
        return;
      }
      if (event.key !== "f" && event.key !== "F") return;
      // Drop any armed confirmation. This toggle sets the controlled prop
      // directly, so Radix never fires `onOpenChange` and the card's own cancel
      // never runs; `pending` would survive the close and the next press would
      // reopen onto a confirmation for a fault the operator walked away from,
      // one click from committing it. Every other close path is a card the
      // dialog owns, which does clear it.
      setPending(null);
      setOpen((was) => !was);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const inject = (preset: Preset) => {
    const clearing = preset.kind === KIND.clear;
    // Closed before the request resolves, not after. The sheet covers the
    // schematic and a third of the strips, and the point of pressing this is to
    // watch those; the toast reports the outcome either way.
    setOpen(false);

    void fetch("/api/fault", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(command(preset, cylinder)),
    })
      .then((response) => {
        // The core answers 503 when the queue has backed up, which means the bus
        // is down. Saying so beats a control that looks like it worked.
        if (!response.ok) {
          toast("No route to the bus", "Nothing was commanded. Check the CAN link.", "crit");
          return;
        }
        const name = faultName(preset, cylinder);
        if (clearing) {
          toast(
            "All faults cleared",
            "The engine is nominal now. The twin re-locks in about 20 s and the health scores follow.",
          );
        } else {
          toast(`${name} commanded`, preset.note);
        }
        setCommanded((held) => (clearing ? [] : held.includes(name) ? held : [...held, name]));
      })
      .catch((error: unknown) => {
        report("fault command failed", error);
        toast("No route to the bus", "Nothing was commanded. Check the CAN link.", "crit");
      });
  };

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <SheetTrigger asChild>
            {/* Full size rather than `sm`, and carrying a glyph. Teammates
                could not find this: at 7px tall beside two lines of dim label
                text it read as part of the panel heading rather than as the one
                control on the screen. */}
            <Button className="shrink-0 gap-[9px]" data-tour="ops-inject">
              <InjectGlyph />
              INJECT FAULT
            </Button>
          </SheetTrigger>
        </TooltipTrigger>
        <TooltipContent side="bottom" sideOffset={6}>
          Command a fault on the simulator
          <Kbd className="ml-2">F</Kbd>
        </TooltipContent>
      </Tooltip>
      <SheetContent side="right" className="w-[420px] sm:max-w-[420px]">
        <SheetHeader>
          <SheetTitle className="t-section">INJECT FAULT</SheetTitle>
          <SheetDescription className="t-small text-muted-foreground">
            Faults for the simulator, so the twin can be watched catching them. None of these reach
            a real engine.
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
              <Button size="sm" className="shrink-0" onClick={() => setPending(preset)}>
                INJECT
              </Button>
            </div>
          ))}
        </div>

        <div className="flex items-center justify-between gap-3 px-4 py-3">
          <span className="label-micro min-w-0 truncate normal-case">
            {commanded.length === 0
              ? "nothing commanded this session"
              : `${commanded.length} running`}
          </span>
          <Button size="sm" variant="ghost" className="shrink-0" onClick={() => setPending(CLEAR)}>
            CLEAR ALL
          </Button>
        </div>

        <Confirm
          preset={pending}
          cylinder={cylinder}
          commanded={commanded}
          onClose={() => setPending(null)}
          onConfirm={inject}
        />
      </SheetContent>
    </Sheet>
  );
}

/**
 * The confirmation card.
 *
 * Rendered inside the drawer, not beside it: as a sibling the drawer's focus
 * trap holds focus away from the card. `preset` carries the open state too, so
 * a closing card cannot be read still naming a fault.
 */
function Confirm({
  preset,
  cylinder,
  commanded,
  onClose,
  onConfirm,
}: {
  preset: Preset | null;
  cylinder: number;
  commanded: string[];
  onClose: () => void;
  onConfirm: (preset: Preset) => void;
}) {
  if (!preset) return null;
  const clearing = preset.kind === KIND.clear;

  return (
    <AlertDialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <AlertDialogContent className="w-[380px] gap-0 p-0 sm:max-w-[380px]">
        <AlertDialogHeader className="border-border place-items-start gap-0 border-b px-4 py-[10px] text-left">
          <AlertDialogTitle className="t-section">
            {clearing ? "CLEAR ALL FAULTS?" : "CONFIRM INJECTION"}
          </AlertDialogTitle>
        </AlertDialogHeader>

        <div className="px-4 py-[14px]">
          {clearing ? (
            <Clearing commanded={commanded} />
          ) : (
            <>
              <Field label="fault">{preset.label}</Field>
              {preset.perCylinder ? <Field label="cylinder">{cylinder}</Field> : null}
              <Field label="profile">{preset.note}</Field>
              <Field label="expect">
                <AlertDialogDescription className="text-inherit">
                  {preset.effect.replaceAll("{n}", String(cylinder))}
                </AlertDialogDescription>
              </Field>
              <Field label="ends">only when the faults are cleared from this panel</Field>
            </>
          )}
        </div>

        <AlertDialogFooter className="border-border gap-2 border-t px-4 py-[10px]">
          <AlertDialogCancel size="sm" variant="ghost">
            CANCEL
          </AlertDialogCancel>
          <AlertDialogAction size="sm" variant="outline" onClick={() => onConfirm(preset)}>
            {clearing ? "CLEAR" : "INJECT"}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

/**
 * What the clear is about to end.
 *
 * A sentence, not a spec sheet. An injection is a parameterised command and the
 * fields are what is commanded; a clear is a yes or no, so it takes the shape a
 * desktop confirmation takes. Two shapes from one component is correct here.
 *
 * An empty list is not a claim that the engine is clean: the drawer knows what
 * it commanded, not what the simulator runs, and a fault set on the command line
 * or before a reload is cleared without ever appearing here.
 */
function Clearing({ commanded }: { commanded: string[] }) {
  return (
    <AlertDialogDescription className="text-foreground text-[12px] leading-[1.6]">
      {commanded.length === 0
        ? "Whatever the simulator is running will be cleared."
        : `${listed(commanded)} will be cleared.`}{" "}
      The engine returns to healthy.
    </AlertDialogDescription>
  );
}

/** `a`, `a and b`, `a, b and c`. */
function listed(names: string[]): string {
  if (names.length === 1) return names[0]!;
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;
}
