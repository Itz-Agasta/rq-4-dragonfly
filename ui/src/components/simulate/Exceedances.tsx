/**
 * What the leg does to the engine, and when.
 *
 * The hero is the verdict, and an empty forecast is a result rather than an empty
 * state: a projection that crosses nothing over six hours is the answer an
 * operator wants, and a panel that goes blank on it would be hiding the good news
 * behind the same treatment as a broken feed.
 *
 * Each row carries the provenance of the limit it crossed, because the two are
 * not the same claim. Oil and coolant limits come from the type certificate;
 * exhaust and head limits are estimated, since the certificate publishes neither
 * and on this engine they are managed by the controller's own fuelling limiter
 * rather than presented to a pilot. A row that crosses an estimated limit is a
 * statement about our number, and it says so.
 */

import { fmt, missionClock } from "@/lib/fmt";
import type { Exceedance, Projection } from "@/lib/projection";

/** How close a channel came to its limit, in the channel's own unit. */
interface Margin {
  channel: string;
  unit: string;
  limit: number;
  peak: number;
  /** Whether the binding end is the floor, so the label reads the right way. */
  below: boolean;
  /** Limit less peak. Never negative here: a channel that crossed is an exceedance. */
  margin: number;
  published: boolean;
}

/**
 * What is left in each channel that did not cross, tightest first.
 *
 * A forecast that crosses nothing is the answer an operator wants, and a panel
 * that then goes blank has thrown away the useful half of it: **how close** it
 * came is what says whether the leg is comfortable or marginal. Computed here
 * rather than served, because it is the peak the daemon already sends minus the
 * limit it already sends, and a third copy could only disagree.
 */
function margins(projection: Projection): Margin[] {
  const crossed = new Set(projection.exceedances.map((one) => one.channel));
  return projection.series
    .flatMap((series) => {
      if (crossed.has(series.name)) return [];
      // Whichever end binds. Oil pressure holds a published floor as well as a
      // ceiling, and measuring it against the ceiling alone reported the most
      // headroom on the channel that was closest to failing.
      const ends: Margin[] = [];
      if (series.limit !== null) {
        const peak = Math.max(...series.values);
        ends.push({
          channel: series.name,
          unit: series.unit,
          limit: series.limit,
          peak,
          margin: series.limit - peak,
          below: false,
          published: series.published,
        });
      }
      if (series.floor !== null) {
        const trough = Math.min(...series.values);
        ends.push({
          channel: series.name,
          unit: series.unit,
          limit: series.floor,
          peak: trough,
          margin: trough - series.floor,
          below: true,
          published: series.published,
        });
      }
      // One row per channel, the tighter end, so a channel cannot appear twice
      // and claim both comfort and danger.
      return ends.toSorted((a, b) => a.margin - b.margin).slice(0, 1);
    })
    .toSorted((a, b) => a.margin - b.margin);
}

/** Seconds as a countdown, which is what an exceedance is read as. */
function inLabel(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  if (m > 0) return `${m}m ${String(s).padStart(2, "0")}s`;
  return `${s}s`;
}

export function Exceedances({ projection }: { projection: Projection }) {
  const first = projection.exceedances[0];
  const hours = fmt(projection.horizon_s / 3600, 2);

  return (
    <div className="border-border flex w-[300px] shrink-0 flex-col overflow-hidden border-l">
      <div className="border-border flex h-9 shrink-0 items-center border-b px-4">
        <span className="t-section">EXCEEDANCE FORECAST</span>
      </div>

      <div className="border-border shrink-0 border-b px-4 pt-3 pb-[14px]">
        {first ? (
          <>
            <div className="label-micro mb-[6px]">first limit crossed</div>
            <div className="num text-primary text-[26px] leading-none">
              {first.channel} <span className="text-[16px]">in {inLabel(first.in_s)}</span>
            </div>
            <div className="label-micro mt-[8px] leading-[1.5] normal-case">
              {projection.exceedances.length} of the {countLimited(projection)} limited channels
              cross inside {hours} h. Nothing here has happened; it is what the model says this
              engine does if the leg is flown.
            </div>
          </>
        ) : (
          <>
            <div className="label-micro mb-[6px]">over the whole leg</div>
            <div className="num text-[26px] leading-none">NO LIMIT CROSSED</div>
            <div className="label-micro mt-[8px] leading-[1.5] normal-case">
              Every limited channel holds for {hours} h at this degradation. The headroom is drawn
              in each cell, against the limit.
            </div>
          </>
        )}
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        {projection.exceedances.map((one) => (
          <Row key={one.channel} exceedance={one} />
        ))}
        <div className="border-border shrink-0 border-b px-4 pt-3 pb-2">
          <span className="label-micro">closest approach · limit less peak</span>
        </div>
        {margins(projection).map((one) => (
          <MarginRow key={one.channel} margin={one} />
        ))}
      </div>
    </div>
  );
}

/** How many projected channels have a limit at all, for the hero's denominator. */
function countLimited(projection: Projection): number {
  return projection.series.filter((s) => s.limit !== null).length;
}

function MarginRow({ margin }: { margin: Margin }) {
  return (
    <div className="border-border shrink-0 border-b px-4 py-[7px]">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-muted-foreground text-[11px] whitespace-nowrap">
          {margin.channel}
        </span>
        <span className="num text-[13px] whitespace-nowrap">
          {fmt(margin.margin, Math.abs(margin.margin) >= 100 ? 0 : 1)}
          <span className="text-muted-foreground ml-1 text-[10px]">{margin.unit || "spare"}</span>
        </span>
      </div>
      <div className="label-micro mt-[2px] flex justify-between normal-case">
        <span>
          {margin.below ? "low" : "peak"} {fmt(margin.peak, 1)} of {fmt(margin.limit, 1)}
          {margin.below ? " min" : ""}
        </span>
        <span className={margin.published ? "" : "text-foreground-dim"}>
          {margin.published ? "published" : "◊ estimated"}
        </span>
      </div>
    </div>
  );
}

function Row({ exceedance }: { exceedance: Exceedance }) {
  return (
    <div className="border-border shrink-0 border-b px-4 py-[10px]">
      <div className="flex items-baseline justify-between gap-2">
        <span className="t-body">{exceedance.channel}</span>
        <span className="num text-primary text-[13px]">in {inLabel(exceedance.in_s)}</span>
      </div>
      <div className="label-micro mt-[5px] flex justify-between normal-case">
        <span>
          {exceedance.below ? "minimum" : "limit"} {fmt(exceedance.limit, 1)} ·{" "}
          {exceedance.below ? "low" : "peak"} {fmt(exceedance.peak, 1)}
        </span>
        <span className={exceedance.published ? "" : "text-foreground-dim"}>
          {exceedance.published ? "published" : "◊ estimated"}
        </span>
      </div>
      <div className="label-micro mt-[3px] normal-case">
        at {missionClock(exceedance.t_s)} mission time
      </div>
    </div>
  );
}
