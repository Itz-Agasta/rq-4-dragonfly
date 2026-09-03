/**
 * FLEET: the same twin, one row per airframe.
 *
 * Answers `ps.md`'s "fleet-level health monitoring infrastructures", the one
 * bullet nothing else covers. The roster is static by decision, `docs/mvp.md` 6
 * cut-ladder item one, and states that on screen: every other screen is fed from
 * the bus, so plausible tails and indices read as live unless the page says
 * otherwise. Provenance is a dashed hairline and a tag, never a hue.
 *
 * Master and detail, not one wide table: eight rows cannot fill 1440px at any
 * sane row pitch, and the prose column strands ~1700px at 2560.
 */

import { useState } from "react";
import { Link } from "react-router";

import { type Airframe, ROSTER, STATE_LABEL, TOTALS } from "@/components/fleet/roster";
import { grouped, NO_VALUE } from "@/lib/fmt";

// The `1fr` is the index bar, not a text column: five columns of short strings
// leave ~570px of slack wherever it goes, and a 0-to-100 index is the one value
// here that reads better as a width than as digits.
const COLUMNS = "260px 116px 132px minmax(0, 1fr) 56px 96px";

// Measured on this station, not authored. Tails-per-core is stated as the
// division it is: no fleet host was built, so nothing benchmarked it.
const TWIN_CORE_PCT = 2.6;
const TAILS_PER_CORE = Math.floor(100 / TWIN_CORE_PCT);

const SCALING: [string, string, string][] = [
  ["TWIN COST PER TAIL", `${TWIN_CORE_PCT}% of a core`, "1.30 ms per frame at 20 Hz, measured"],
  ["INGEST PER TAIL", "2.4 kB · 20 Hz", "48 kB/s on the wire, measured"],
  ["TAILS PER CORE", `~${TAILS_PER_CORE}`, "arithmetic from the two above, not a benchmark"],
];

// `--ok` is grey by contract, so a healthy fleet is quiet and only the rows
// wanting attention take a hue.
function stateClass(state: Airframe["state"]): string {
  if (state === "grounded") return "text-[var(--crit)]";
  if (state === "advisory") return "text-[var(--primary)]";
  if (state === "station") return "text-foreground";
  return "text-muted-foreground";
}

function Summary() {
  return (
    <div className="lattice shrink-0" style={{ gridTemplateColumns: "repeat(4, minmax(0, 1fr))" }}>
      {[
        ["AIRFRAMES MONITORED", grouped(TOTALS.airframes)],
        ["ENGINE HOURS", grouped(TOTALS.engineHours)],
        ["OPEN ADVISORIES", grouped(TOTALS.advisories)],
        ["WITHHELD", grouped(TOTALS.grounded)],
      ].map(([label, value]) => (
        <section key={label} className="cell flex flex-col gap-1">
          <span className="label-micro">{label}</span>
          <span className="num t-value text-foreground">{value}</span>
        </section>
      ))}
    </div>
  );
}

function Row({
  one,
  selected,
  onSelect,
}: {
  one: Airframe;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      // The selected marker is a bar rather than a fill: the nav rail already
      // owns inversion, and a second inverted block would read as a second
      // active screen.
      className={`border-border focus-visible:ring-ring relative grid min-h-[56px] flex-1 items-center border-b px-4 text-left outline-none focus-visible:ring-1 focus-visible:-outline-offset-1 ${
        selected ? "bg-[var(--muted)]" : "hover:bg-[var(--accent)]"
      }`}
      style={{ gridTemplateColumns: COLUMNS }}
    >
      {selected && (
        <span className="bg-foreground absolute top-0 bottom-0 left-0 w-[2px]" aria-hidden="true" />
      )}
      <span className="flex min-w-0 flex-col gap-0.5">
        <span className="num t-small text-foreground">{one.tail}</span>
        <span className="label-micro truncate">
          {one.base} · {one.engine}
        </span>
      </span>
      <span className={`label-micro ${stateClass(one.state)}`}>{STATE_LABEL[one.state]}</span>
      <span className="t-small text-muted-foreground truncate">{one.subsystem ?? NO_VALUE}</span>
      {/* No index means no track at all, not an empty one. An unfilled track
          reads as a score of zero, and the absence of a score is not a score;
          `handover.md` 3 records the same trap on the mission overview. */}
      {one.index === null ? (
        <span aria-hidden="true" />
      ) : (
        <span className="mr-4 flex h-[3px] min-w-0 bg-[var(--grid)]" aria-hidden="true">
          <span
            className={
              one.state === "grounded"
                ? "bg-[var(--crit)]"
                : one.state === "advisory"
                  ? "bg-[var(--primary)]"
                  : "bg-[var(--structure-hi)]"
            }
            style={{ width: `${one.index}%` }}
          />
        </span>
      )}
      <span
        className={`num text-[18px] leading-none ${
          one.index === null ? "text-foreground-dim" : stateClass(one.state)
        }`}
      >
        {one.index ?? NO_VALUE}
      </span>
      <span className="num t-small text-muted-foreground">
        {one.rulHours === null ? NO_VALUE : `${grouped(one.rulHours)} h`}
      </span>
    </button>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <span className="label-micro">{label}</span>
      <span className="num t-small text-foreground text-right">{value}</span>
    </div>
  );
}

function Detail({ one }: { one: Airframe }) {
  const station = one.state === "station";
  return (
    <div className="border-border flex w-[360px] shrink-0 flex-col border-l">
      <div className="border-border flex h-9 shrink-0 items-center justify-between border-b px-4">
        <span className="t-section text-foreground num">{one.tail}</span>
        <span className={`label-micro ${stateClass(one.state)}`}>{STATE_LABEL[one.state]}</span>
      </div>

      <div className="flex flex-col gap-2 px-4 py-3">
        <Field label="BASE" value={one.base} />
        <Field label="ENGINE S/N" value={one.engine} />
        <Field label="HOURS SINCE OVERHAUL" value={grouped(one.hours)} />
        <Field label="LIMITING SUBSYSTEM" value={one.subsystem ?? NO_VALUE} />
        <Field label="INDEX" value={one.index === null ? NO_VALUE : String(one.index)} />
        <Field
          label="REMAINING LIFE"
          value={one.rulHours === null ? NO_VALUE : `${grouped(one.rulHours)} h`}
        />
      </div>

      <div className="border-border border-t px-4 py-3">
        <span className="label-micro">ADVISORY</span>
        <p className="text-muted-foreground mt-1.5 text-[12px] leading-[1.5]">
          {one.advisory}
          {station && (
            <>
              {" "}
              <Link
                to="/ops"
                className="text-foreground focus-visible:ring-ring underline underline-offset-2 outline-none hover:opacity-70 focus-visible:ring-1"
              >
                Open OPS
              </Link>
            </>
          )}
        </p>
      </div>

      <div className="flex-1" />

      {/* Pinned to the foot of the pane: it is about the fleet, not about the
          selected tail, and it is the only measured thing on this screen. */}
      <div className="border-border shrink-0 border-t px-4 py-2">
        <span className="label-micro">WHAT ONE MORE TAIL COSTS</span>
      </div>
      {SCALING.map(([label, value, note]) => (
        <div key={label} className="border-border shrink-0 border-t px-4 py-2">
          <div className="flex items-baseline justify-between gap-3">
            <span className="label-micro">{label}</span>
            <span className="num t-small text-foreground">{value}</span>
          </div>
          <span className="label-micro text-foreground-dim normal-case">{note}</span>
        </div>
      ))}
    </div>
  );
}

export function Fleet() {
  // An operator opens this screen to find what needs attention, so the withheld
  // airframe is selected rather than the first row.
  const [tail, setTail] = useState(
    () => (ROSTER.find((one) => one.state === "grounded") ?? ROSTER[0]!).tail,
  );
  const selected = ROSTER.find((one) => one.tail === tail) ?? ROSTER[0]!;

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="border-border flex h-9 shrink-0 items-center gap-3 border-b px-3">
        <span className="t-section text-muted-foreground">WING</span>
        <span className="label-micro">
          {TOTALS.airframes} AIRFRAMES · 2 LOCATIONS · ONE TWIN PER TAIL
        </span>
        <span className="label-micro text-foreground-dim ml-auto border border-dashed border-[var(--foreground-dim)] px-2 py-0.5">
          ◊ STATIC ROSTER · NOT A LIVE FEED
        </span>
      </div>

      <div className="flex min-h-0 min-w-0 flex-1">
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <Summary />

          <div
            className="border-border text-muted-foreground grid shrink-0 items-center border-b px-4 py-2"
            style={{ gridTemplateColumns: COLUMNS }}
          >
            {["AIRFRAME", "STATE", "LIMITING", "HEALTH", "IDX", "REM LIFE"].map((heading) => (
              <span key={heading} className="label-micro">
                {heading}
              </span>
            ))}
          </div>

          {/* Rows share the leftover height; at a fixed pitch eight of them
              leave a third of the screen empty. */}
          <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
            {ROSTER.map((one) => (
              <Row
                key={one.tail}
                one={one}
                selected={one.tail === selected.tail}
                onSelect={() => setTail(one.tail)}
              />
            ))}
          </div>

          <div className="border-border text-foreground-dim shrink-0 border-t px-4 py-2 text-[11px] leading-[1.4]">
            Each row is one instance of the twin on this station: the same model, the same residual
            generator and the same seven indices, aggregated per tail. Replacing this roster is a
            change of source, not of screen.
          </div>
        </div>

        <Detail one={selected} />
      </div>
    </div>
  );
}
