/**
 * What was flown, and what it cost to fly it.
 *
 * The altitude trace is the profile's own shape, drawn here rather than as a
 * channel because it is the independent variable: every thermal channel to the
 * right of it is arguing about this line.
 *
 * The speed figure is measured on the request that produced this projection, not
 * quoted from a benchmark. It is the one number on the screen that is about the
 * software rather than the engine, which is why it sits with the run facts and
 * not with the forecast.
 *
 * The seeded health is here rather than left implicit because it is the screen's
 * whole claim. Without it a viewer has to take on trust that the engine flown
 * forward is the degraded one; with it they can read which parameter is off
 * nominal and by how much, and check it against the health rail on OPS.
 */

import { BOX, path, scaleOf } from "@/components/trace";
import { fmt, missionClock } from "@/lib/fmt";
import type { Preset, Projection, SeedParam } from "@/lib/projection";

/**
 * Consumed life below which a parameter is called nominal.
 *
 * The filter's estimates wander by a fraction of a percent on a healthy engine,
 * so a panel listing every parameter that is not exactly nominal would list all
 * ten, every time, and say nothing.
 */
const OFF_NOMINAL = 0.02;

export function Outlook({ projection, preset }: { projection: Projection; preset: Preset }) {
  const altitude = scaleOf(projection.altitude_ft.map((ft) => ft / 1000));

  return (
    <div className="border-border flex w-[260px] shrink-0 flex-col overflow-hidden border-r">
      <div className="border-border flex h-9 shrink-0 items-center border-b px-4">
        <span className="t-section">PROJECTED LEG</span>
      </div>

      <div className="border-border flex h-[130px] shrink-0 flex-col border-b px-4 pt-[10px] pb-2">
        <div className="flex items-baseline justify-between gap-[10px]">
          <span className="label-micro">pressure alt · kft</span>
          <span className="num text-foreground-dim text-[10px]">
            {altitude.flat
              ? `${fmt(altitude.lo, 1)} constant`
              : `${fmt(altitude.lo, 1)} – ${fmt(altitude.hi, 1)}`}
          </span>
        </div>
        <svg
          width="100%"
          height="100%"
          viewBox={`0 0 ${BOX} ${BOX}`}
          preserveAspectRatio="none"
          className="mt-[7px] block min-h-0 flex-1"
        >
          {[30, 65].map((y) => (
            <line
              key={y}
              x1="0"
              y1={y}
              x2={BOX}
              y2={y}
              stroke="var(--grid)"
              strokeWidth="1"
              vectorEffect="non-scaling-stroke"
            />
          ))}
          <path
            d={path(altitude.values, altitude)}
            fill="none"
            stroke="var(--predicted)"
            strokeWidth="1.5"
            strokeDasharray="4 3"
            vectorEffect="non-scaling-stroke"
          />
        </svg>
      </div>

      <div className="border-border shrink-0 border-t px-4 pt-[10px] pb-[14px]">
        <div className="label-micro mb-[6px]">the run</div>
        <Row label="Seeded at" unit="" value={missionClock(projection.from_t_s)} />
        <Row label="Horizon" unit="h" value={fmt(projection.horizon_s / 3600, 2)} />
        <Row label="Sample" unit="s" value={fmt(projection.sample_s, 2)} />
        <Row label="Fuel burn" unit="L" value={fmt(projection.fuel_burn_l, 1)} />
      </div>

      <Seed health={projection.seed_health} />

      <div className="border-border shrink-0 border-t border-b px-4 pt-[10px] pb-[14px]">
        <div className="label-micro mb-[6px]">model forward, not the filter</div>
        <div className="num text-[22px] leading-none">
          {Math.round(projection.speed_x).toLocaleString()}
          <span className="text-muted-foreground ml-1 text-[11px]">× real time</span>
        </div>
        <div className="label-micro mt-[6px] leading-[1.5] normal-case">
          {fmt(projection.horizon_s / 3600, 2)} h of {preset.label.toLowerCase()} in{" "}
          {fmt(projection.wall_ms / 1000, 2)} s, at the model&rsquo;s 200 Hz step. Measured on this
          request.
        </div>
      </div>
    </div>
  );
}

/**
 * The engine this was flown as.
 *
 * Shown as value over failure threshold, which is exactly how OPS's health rail
 * shows the same parameter, so the two screens encode one quantity one way. The
 * raw value alone is illegible: an injector coefficient of 0.814 means nothing
 * without the 0.620 it is heading for.
 */
function Seed({ health }: { health: SeedParam[] }) {
  const degraded = health.filter((one) => one.consumed >= OFF_NOMINAL);

  return (
    <div className="border-border flex max-h-[45%] shrink-0 flex-col overflow-y-auto border-t px-4 pt-[10px] pb-[14px]">
      <div className="label-micro mb-[6px] shrink-0">seeded from the twin&rsquo;s estimate</div>
      {degraded.length === 0 ? (
        <div className="label-micro leading-[1.5] normal-case">
          Every health parameter within {Math.round(OFF_NOMINAL * 100)}% of nominal. This is a
          projection of an engine the filter believes is well.
        </div>
      ) : (
        degraded.map((one) => (
          <div
            key={one.name}
            className="border-border flex items-baseline justify-between gap-[10px] border-t py-[5px]"
          >
            <span className="text-muted-foreground text-[11px] leading-[1.4] whitespace-nowrap">
              {one.name}
            </span>
            <span className="num text-[13px] whitespace-nowrap">
              {fmt(one.value, 3)} / {fmt(one.failure, 3)}
              <span className="text-primary ml-[6px] text-[10px]">
                {fmt(one.consumed * 100, 0)}%
              </span>
            </span>
          </div>
        ))
      )}
    </div>
  );
}

function Row({ label, unit, value }: { label: string; unit: string; value: string }) {
  return (
    <div className="border-border flex items-baseline justify-between gap-[10px] border-t py-[5px]">
      <span className="text-muted-foreground text-[11px] leading-[1.4] whitespace-nowrap">
        {label}
      </span>
      <span className="num text-[13px] whitespace-nowrap">
        {value}
        {unit ? <span className="text-muted-foreground ml-1 text-[10px]">{unit}</span> : null}
      </span>
    </div>
  );
}
