/**
 * Interactive line chart for the generated validation data.
 *
 * **React owns the DOM, D3 owns the arithmetic.** Only `d3-scale`, `d3-shape` and
 * `d3-array` are imported, all of which are pure functions over numbers; every
 * mark is JSX. Letting `d3.select` mutate inside a React tree is the documented
 * way to get two libraries fighting over the same nodes, and it buys nothing here
 * because there are no transitions to drive.
 *
 * **The data is the model's own output.** `public/validation/charts.json` was
 * dumped from a sweep of the engine model, the same sweep that writes the figures
 * in `docs/model_validation.md`. It is a committed snapshot, like the parameter
 * ledger: re-dump it if the model changes. Building a permanent emitter into the
 * crate was tried and reverted, because 200 lines of hand-rolled JSON in an
 * example is a lot of machinery for a screen scheduled to move to the landing
 * page. `reference: true` marks the three published limits.
 *
 * Hover snaps to the nearest sample by x, because a crosshair that follows the
 * pointer freely invites reading a value off a part of the curve that has no
 * sample under it.
 */

import { bisector, extent } from "d3-array";
import { scaleLinear } from "d3-scale";
import { line } from "d3-shape";
import { useEffect, useMemo, useRef, useState } from "react";

export interface ChartSeries {
  label: string;
  /** Line style only. A dashed curve may still be model output. */
  dashed: boolean;
  /** A published limit or reference line rather than model output. */
  reference: boolean;
  points: [number, number][];
}

export interface ChartSpec {
  id: string;
  title: string;
  xAxis: string;
  yAxis: string;
  yRange: [number, number] | null;
  series: ChartSeries[];
}

/**
 * Every figure, keyed by the id `just validate` gave it.
 *
 * One fetch shared by all six charts. A failed load renders empty slots: the file
 * ships inside the bundle, so the only way to miss it is a broken deployment.
 */
let pending: Promise<ChartSpec[]> | null = null;

export function useCharts(): Map<string, ChartSpec> {
  const [charts, setCharts] = useState<Map<string, ChartSpec>>(new Map());
  useEffect(() => {
    pending ??= fetch("/validation/charts.json")
      .then((r) => (r.ok ? (r.json() as Promise<ChartSpec[]>) : []))
      .catch(() => []);
    let live = true;
    void pending.then((specs) => {
      if (live) setCharts(new Map(specs.map((x) => [x.id, x])));
    });
    return () => {
      live = false;
    };
  }, []);
  return charts;
}

const MARGIN = { top: 16, right: 20, bottom: 44, left: 64 };

/**
 * Luminance ramp for solid series, never a rainbow.
 *
 * `index.css` reserves every hue in this product for alarm, so four curves on one
 * axis separate by brightness instead. All four are existing tokens and the
 * dimmest is 3.31:1, which still reads as a line at 2px. A fifth curve on one
 * chart would not separate, and the answer there is a second chart.
 */
const RAMP = [
  "var(--measured)",
  "var(--muted-foreground)",
  "var(--foreground-dim)",
  "var(--structure-hi)",
];

/**
 * A published limit takes the accent. Model output never does.
 *
 * This is the one place the accent keeps its product-wide meaning on this screen:
 * a redline, a rating, a demonstrated containment speed. Colouring a model curve
 * with it would say "caution" about a result, which is the error the generated
 * figures made by indexing an accent into a positional palette.
 */
function stroke(series: ChartSeries, solidIndex: number): string {
  if (series.reference) return "var(--primary)";
  if (series.dashed) return "var(--predicted)";
  return RAMP[solidIndex % RAMP.length] ?? RAMP[0];
}
const HEIGHT = 340;
const byX = bisector<[number, number], number>((d) => d[0]).center;

/** Width from the container, so the plot fills whatever the pane gives it. */
function useWidth(ref: React.RefObject<HTMLDivElement | null>) {
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    // ResizeObserver rather than a window listener: the pane can change width
    // without the window doing so, and the listener would never fire.
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setWidth(entry.contentRect.width);
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}

/**
 * A label formatter for one axis, with the precision fixed across its own ticks.
 *
 * Precision comes from the tick interval rather than from each value, because
 * per-value formatting gave one axis `0.50`, `1`, `1.5`, `2` and the ragged
 * decimals read as a rendering fault. Thousands are grouped, so a turbocharger
 * speed is `140,082` rather than `140082`.
 */
function axisFormat(ticks: number[]): (v: number) => string {
  const step = ticks.length > 1 ? Math.abs(ticks[1] - ticks[0]) : 1;
  const decimals = step >= 1 ? 0 : Math.min(3, Math.ceil(-Math.log10(step)));
  return (v) =>
    v.toLocaleString("en", {
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    });
}

/** Single values outside an axis, where no sibling ticks set the precision. */
function tick(v: number): string {
  if (Number.isInteger(v)) return v.toLocaleString("en");
  const a = Math.abs(v);
  if (a >= 10) return v.toFixed(0);
  if (a >= 1) return v.toFixed(1);
  return v.toFixed(2);
}

export function Chart({ spec, caption }: { spec: ChartSpec; caption: string }) {
  const host = useRef<HTMLDivElement>(null);
  const width = useWidth(host);
  const [hover, setHover] = useState<number | null>(null);

  const plot = useMemo(() => {
    const all = spec.series.flatMap((s) => s.points);
    const [x0 = 0, x1 = 1] = extent(all, (d) => d[0]);
    const [dy0 = 0, dy1 = 1] = extent(all, (d) => d[1]);
    const [y0, y1] = spec.yRange ?? [dy0, dy1];

    const x = scaleLinear()
      .domain([x0, x1])
      .range([MARGIN.left, Math.max(MARGIN.left + 1, width - MARGIN.right)]);
    const y = scaleLinear()
      .domain([y0, y1])
      .nice()
      .range([HEIGHT - MARGIN.bottom, MARGIN.top]);

    const path = line<[number, number]>()
      .x((d) => x(d[0]))
      .y((d) => y(d[1]));

    const xTicks = x.ticks(7);
    const yTicks = y.ticks(6);
    return { x, y, path, xTicks, yTicks, fx: axisFormat(xTicks), fy: axisFormat(yTicks) };
  }, [spec, width]);

  // The series carrying the hover readout is the first solid one: the dashed
  // series are limits and references, and a limit has no value worth probing.
  const solidOrder = useMemo(() => {
    const order = new Map<string, number>();
    let n = 0;
    for (const s of spec.series) {
      if (!s.dashed) order.set(s.label, n++);
    }
    return order;
  }, [spec]);

  const probe = spec.series.find((s) => !s.dashed) ?? spec.series[0];
  const index = hover === null || !probe ? null : byX(probe.points, plot.x.invert(hover));

  return (
    <figure className="border-border mx-auto flex w-full max-w-[1100px] min-w-0 flex-col border">
      <figcaption className="border-border text-foreground border-b px-4 py-2.5 text-[13px]">
        {spec.title}
      </figcaption>

      <div ref={host} className="min-w-0">
        {width > 0 ? (
          <svg
            viewBox={`0 0 ${width} ${HEIGHT}`}
            width={width}
            height={HEIGHT}
            className="block"
            role="img"
            aria-label={`${spec.title}. ${spec.xAxis} against ${spec.yAxis}.`}
            onPointerMove={(e) =>
              setHover(e.clientX - e.currentTarget.getBoundingClientRect().left)
            }
            onPointerLeave={() => setHover(null)}
          >
            <title>{spec.title}</title>

            {plot.yTicks.map((t) => (
              <g key={t}>
                <line
                  x1={MARGIN.left}
                  x2={width - MARGIN.right}
                  y1={plot.y(t)}
                  y2={plot.y(t)}
                  stroke="var(--grid)"
                />
                <text
                  x={MARGIN.left - 10}
                  y={plot.y(t) + 4}
                  textAnchor="end"
                  className="num"
                  fill="var(--muted-foreground)"
                  fontSize="11"
                >
                  {plot.fy(t)}
                </text>
              </g>
            ))}

            {plot.xTicks.map((t) => (
              <g key={t}>
                <line
                  x1={plot.x(t)}
                  x2={plot.x(t)}
                  y1={MARGIN.top}
                  y2={HEIGHT - MARGIN.bottom}
                  stroke="var(--grid)"
                />
                <text
                  x={plot.x(t)}
                  y={HEIGHT - MARGIN.bottom + 18}
                  textAnchor="middle"
                  className="num"
                  fill="var(--muted-foreground)"
                  fontSize="11"
                >
                  {plot.fx(t)}
                </text>
              </g>
            ))}

            <line
              x1={MARGIN.left}
              x2={width - MARGIN.right}
              y1={HEIGHT - MARGIN.bottom}
              y2={HEIGHT - MARGIN.bottom}
              stroke="var(--border)"
            />

            {/* Solid is measured or modelled, dashed is a limit or a reference.
                Never a hue: the accent is spent on alarms across this product. */}
            {spec.series.map((s) => (
              <path
                key={s.label}
                d={plot.path(s.points) ?? undefined}
                fill="none"
                stroke={stroke(s, solidOrder.get(s.label) ?? 0)}
                strokeWidth={s.dashed ? 1.5 : 2}
                strokeDasharray={s.dashed ? "6 4" : undefined}
              />
            ))}

            {index !== null && probe?.points[index] ? (
              <g>
                <line
                  x1={plot.x(probe.points[index][0])}
                  x2={plot.x(probe.points[index][0])}
                  y1={MARGIN.top}
                  y2={HEIGHT - MARGIN.bottom}
                  stroke="var(--structure-hi)"
                />
                {spec.series
                  .filter((s) => !s.dashed)
                  .map((s) => {
                    const p = s.points[byX(s.points, probe.points[index][0])];
                    return p ? (
                      <circle
                        key={s.label}
                        cx={plot.x(p[0])}
                        cy={plot.y(p[1])}
                        r={3}
                        fill="var(--background)"
                        stroke={stroke(s, solidOrder.get(s.label) ?? 0)}
                        strokeWidth={1.5}
                      />
                    ) : null;
                  })}
              </g>
            ) : null}

            <text
              x={(MARGIN.left + width - MARGIN.right) / 2}
              y={HEIGHT - 6}
              textAnchor="middle"
              fill="var(--muted-foreground)"
              fontSize="11"
            >
              {spec.xAxis}
            </text>
            <text
              x={14}
              y={(MARGIN.top + HEIGHT - MARGIN.bottom) / 2}
              textAnchor="middle"
              fill="var(--muted-foreground)"
              fontSize="11"
              transform={`rotate(-90 14 ${(MARGIN.top + HEIGHT - MARGIN.bottom) / 2})`}
            >
              {spec.yAxis}
            </text>
          </svg>
        ) : (
          <div style={{ height: HEIGHT }} />
        )}
      </div>

      <Readout spec={spec} probe={probe} index={index} solidOrder={solidOrder} />

      <figcaption className="border-border text-muted-foreground border-t px-4 py-2.5 text-[12px] leading-[1.6]">
        {caption}
      </figcaption>
    </figure>
  );
}

/**
 * The values under the crosshair, in a fixed row rather than a floating tooltip.
 *
 * A tooltip that follows the pointer covers the curve it is describing and moves
 * the numbers while the reader is trying to read them. A fixed row costs 28px and
 * holds still. With no hover it shows the legend, so the row is never empty and
 * the layout never jumps.
 */
function Readout({
  spec,
  probe,
  index,
  solidOrder,
}: {
  spec: ChartSpec;
  probe: ChartSeries | undefined;
  index: number | null;
  solidOrder: Map<string, number>;
}) {
  const at = index !== null && probe ? probe.points[index] : undefined;

  return (
    <div className="border-border text-muted-foreground flex flex-wrap items-baseline gap-x-5 gap-y-1 border-t px-4 py-2 text-[12px]">
      {at ? (
        <>
          <span className="num text-foreground">
            {spec.xAxis.split(",")[0]} {tick(at[0])}
          </span>
          {/* Model output only. A reference line is a limit, and a vertical one
              has no y to read at a given x: probing the critical-altitude marker
              returned the top of its own line and printed it as a power. */}
          {spec.series
            .filter((s) => !s.dashed)
            .map((s) => {
              const p = s.points[byX(s.points, at[0])];
              return p ? (
                <span key={s.label} className="num">
                  {s.label} <span className="text-foreground">{tick(p[1])}</span>
                </span>
              ) : null;
            })}
        </>
      ) : (
        spec.series.map((s) => (
          <span key={s.label} className="flex items-center gap-2">
            <svg width="18" height="8" aria-hidden="true">
              <line
                x1="0"
                x2="18"
                y1="4"
                y2="4"
                stroke={stroke(s, solidOrder.get(s.label) ?? 0)}
                strokeWidth={s.dashed ? 1.5 : 2}
                strokeDasharray={s.dashed ? "5 3" : undefined}
              />
            </svg>
            {s.label}
          </span>
        ))
      )}
    </div>
  );
}

/**
 * One figure by id, or nothing until the data arrives.
 *
 * The slot reserves its height so the document does not reflow under the reader
 * when six charts land at once.
 */
export function Plot({
  id,
  charts,
  caption,
}: {
  id: string;
  charts: Map<string, ChartSpec>;
  caption: string;
}) {
  const spec = charts.get(id);
  if (!spec) {
    return <div className="mx-auto w-full max-w-[1100px]" style={{ height: HEIGHT + 96 }} />;
  }
  return <Chart spec={spec} caption={caption} />;
}
