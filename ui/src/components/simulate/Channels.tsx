/**
 * Every projected channel, against the limit a conventional monitor watches it
 * with.
 *
 * The limit is drawn inside the same scale as the trace, by the same arithmetic,
 * and the scale is floored at the limit so a channel sitting well under one shows
 * its headroom rather than filling the cell and implying there is none. That
 * headroom is the point of the screen when nothing is crossed.
 *
 * A channel is accented only where it is **past** its limit, so the accent means
 * exactly one thing here and it is never spent on ordinary data.
 */

import { BOX, path, type Scale, scaleOf, span, yOf } from "@/components/trace";
import { fmt } from "@/lib/fmt";
import type { ProjectedSeries, Projection } from "@/lib/projection";

/**
 * Decimal places that make a channel's own movement visible.
 *
 * Taken from the span rather than from the unit. Fuel flow moves 0.2 kg/h over a
 * four hour cruise, and at the unit's natural precision that reads `11 - 11`
 * beside a trace climbing the full cell, which looks like a rendering fault and
 * is a rounding one.
 */
function places(lo: number, hi: number): number {
  const range = hi - lo;
  if (!(range > 0)) return Math.abs(hi) >= 100 ? 0 : 1;
  if (range >= 50) return 0;
  if (range >= 5) return 1;
  return 2;
}

/**
 * Channels that must be drawn against each other, keyed by what they measure.
 *
 * The four cylinders of a quantity share one scale. Scaled individually they each
 * fill their own cell, and the coked cylinder running a hundred kelvin cooler than
 * its neighbours draws the same shape as they do, in the same place, with only a
 * number to say otherwise. That is the one comparison this engine's fault is made
 * of.
 */
function groupOf(name: string): string {
  return name.replace(/ \d+$/, "");
}

/** One shared scale per group, floored at the group's limit. */
function scales(projection: Projection): Map<string, Scale> {
  const members = new Map<string, ProjectedSeries[]>();
  for (const series of projection.series) {
    const key = groupOf(series.name);
    const found = members.get(key);
    if (found) found.push(series);
    else members.set(key, [series]);
  }

  const out = new Map<string, Scale>();
  for (const [key, group] of members) {
    const limit = group[0]?.limit ?? null;
    const floor = group[0]?.floor ?? null;
    const scale = span(group.map((one) => scaleOf(one.values, limit ?? undefined)));
    // A floor is pulled into view the way a limit is. Drawn outside the box it
    // would be a limit the trace is silently never compared against.
    out.set(key, floor === null ? scale : { ...scale, lo: Math.min(scale.lo, floor) });
  }
  return out;
}

export function Channels({ projection }: { projection: Projection }) {
  const shared = scales(projection);

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="border-border flex h-9 shrink-0 items-center justify-between border-b px-4">
        <span className="t-section">PROJECTED CHANNELS</span>
        <span className="label-micro normal-case">
          {projection.t_s.length} samples at {fmt(projection.sample_s, 2)} s · dashed line is the
          certified limit
        </span>
      </div>
      <div className="grid min-h-0 min-w-0 flex-1 grid-cols-4 grid-rows-4">
        {projection.series.map((series) => (
          <Cell
            key={series.name}
            series={series}
            scale={shared.get(groupOf(series.name)) ?? scaleOf(series.values)}
          />
        ))}
      </div>
    </div>
  );
}

function Cell({ series, scale }: { series: ProjectedSeries; scale: Scale }) {
  const limit = series.limit;
  // Two scales on purpose. The trace is drawn against the shared one, floored at
  // the limit so the headroom is visible; the readout states the range this
  // channel's **own data** covered. Labelling the drawn scale would report a
  // channel as having reached a limit, or a neighbour's extreme, that it never
  // approached.
  const data = scaleOf(series.values);
  const dp = places(data.lo, data.hi);

  // Above the limit, with the sample either side kept, so a single-sample
  // crossing draws a segment rather than a lone unpainted move.
  const over =
    limit === null
      ? null
      : series.values.map((value, i) => {
          const near =
            value > limit ||
            (series.values[i - 1] ?? -Infinity) > limit ||
            (series.values[i + 1] ?? -Infinity) > limit;
          return near ? value : Number.NaN;
        });
  const crossed = over?.some((v) => Number.isFinite(v)) ?? false;

  const floor = series.floor;
  const under =
    floor === null
      ? null
      : series.values.map((value, i) => {
          const near =
            value < floor ||
            (series.values[i - 1] ?? Infinity) < floor ||
            (series.values[i + 1] ?? Infinity) < floor;
          return near ? value : Number.NaN;
        });
  const sank = under?.some((v) => Number.isFinite(v)) ?? false;

  return (
    <div className="border-border flex min-h-0 min-w-0 flex-col border-r border-b px-3 pt-2 pb-1">
      <div className="flex items-baseline justify-between gap-2">
        <span className="t-small whitespace-nowrap">
          {series.name}
          {series.unit ? (
            <span className="text-muted-foreground ml-1 text-[10px]">{series.unit}</span>
          ) : null}
        </span>
        <span className="num text-foreground-dim text-[10px] whitespace-nowrap">
          {data.flat ? `${fmt(data.lo, dp)} constant` : `${fmt(data.lo, dp)}–${fmt(data.hi, dp)}`}
        </span>
      </div>

      <svg
        width="100%"
        height="100%"
        viewBox={`0 0 ${BOX} ${BOX}`}
        preserveAspectRatio="none"
        className="mt-[6px] block min-h-0 min-w-0 flex-1"
      >
        {limit !== null && !scale.flat ? (
          <line
            x1="0"
            y1={yOf(limit, scale)}
            x2={BOX}
            y2={yOf(limit, scale)}
            stroke="var(--crit)"
            strokeWidth="1"
            strokeDasharray="5 4"
            vectorEffect="non-scaling-stroke"
          />
        ) : null}
        <path
          d={path(series.values, scale)}
          fill="none"
          stroke="var(--measured)"
          strokeWidth="2"
          vectorEffect="non-scaling-stroke"
        />
        {floor !== null && !scale.flat ? (
          <line
            x1="0"
            y1={yOf(floor, scale)}
            x2={BOX}
            y2={yOf(floor, scale)}
            stroke="var(--crit)"
            strokeWidth="1"
            strokeDasharray="5 4"
            vectorEffect="non-scaling-stroke"
          />
        ) : null}
        {over && crossed ? (
          <path
            d={path(over, scale)}
            fill="none"
            stroke="var(--primary)"
            strokeWidth="2"
            vectorEffect="non-scaling-stroke"
          />
        ) : null}
        {under && sank ? (
          <path
            d={path(under, scale)}
            fill="none"
            stroke="var(--primary)"
            strokeWidth="2"
            vectorEffect="non-scaling-stroke"
          />
        ) : null}
      </svg>

      <div className="label-micro mt-[3px] flex justify-between normal-case">
        <span>
          {limit === null ? "no limit" : `limit ${fmt(limit, Math.abs(limit) >= 100 ? 1 : 2)}`}
          {floor === null ? "" : ` · min ${fmt(floor, Math.abs(floor) >= 100 ? 1 : 2)}`}
        </span>
        {limit === null ? null : (
          <span className={series.published ? "" : "text-foreground-dim"}>
            {series.published ? "published" : "\u25CA estimated"}
          </span>
        )}
      </div>
    </div>
  );
}
