/**
 * Six channels over the whole mission, with the twin drawn against two of them.
 *
 * The screen has to argue divergence from a model rather than "a cylinder got
 * hot", which any thermocouple could say, so EGT and CHT carry the dashed
 * predicted trace. The four context channels are drawn dim: luminance is a data
 * channel and spending it evenly across six panels of stationary noise wastes
 * it on the four carrying no part of the story.
 */

import { useMemo, useRef } from "react";

import { BOX, path, series, span } from "@/components/replay/trace";
import { fmt, missionClock } from "@/lib/fmt";
import { Live, useLiveSink } from "@/lib/live";
import { type Frame, TWIN } from "@/lib/telemetry";
import { channel } from "@/store/frame";
import { session, useReplay } from "@/store/replay";

/** Points per trace. A strip is about 400 px wide at 1600. */
const MAX_POINTS = 480;

interface StripSpec {
  /** Channel registry id of the trace that carries the story. */
  id: string;
  /** Sibling cylinders, drawn as thin ghosts on one shared scale. */
  ghosts: string[];
  /** The twin's prediction of `id`, dashed. */
  predicted?: string;
  /** Context rather than evidence, drawn dim. */
  context: boolean;
}

export function MissionStrips() {
  const frames = useReplay((s) => s.frames);
  const specs = useMemo(() => layout(frames), [frames]);

  return (
    <div className="relative flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="border-border flex h-9 min-w-0 shrink-0 items-center justify-between gap-5 border-b px-[18px]">
        <span className="t-section whitespace-nowrap">TELEMETRY · FULL MISSION</span>
        <span className="label-micro whitespace-nowrap">
          playhead{" "}
          <span className="num text-foreground text-[11px]">
            <Live select={(f) => missionClock(f.t_s)} placeholder="T+00:00:00" />
          </span>
        </span>
      </div>

      {[0, 1, 2].map((row) => (
        <div
          key={row}
          className="border-border flex min-h-0 min-w-0 flex-1 items-stretch border-b last:border-b-0"
        >
          {specs.slice(row * 2, row * 2 + 2).map((spec) => (
            <Strip key={spec.id} spec={spec} />
          ))}
        </div>
      ))}
    </div>
  );
}

function Strip({ spec }: { spec: StripSpec }) {
  const ch = channel(spec.id);
  const head = useRef<SVGLineElement>(null);
  const frames = useReplay((s) => s.frames);
  const paths = useMemo(() => traces(frames, spec), [frames, spec]);

  useLiveSink(() => {
    const x = ((session.t / (session.duration || 1)) * BOX).toFixed(3);
    head.current?.setAttribute("x1", x);
    head.current?.setAttribute("x2", x);
  });

  return (
    <div className="border-border flex min-h-0 min-w-0 flex-1 flex-col border-r px-4 py-[10px] last:border-r-0">
      <div className="flex min-w-0 items-baseline justify-between gap-[10px]">
        <span className="label-micro whitespace-nowrap">{ch.label}</span>
        <span
          className={`num text-[16px] whitespace-nowrap ${
            spec.context ? "text-muted-foreground" : "text-foreground"
          }`}
        >
          <Live select={(f) => fmt(ch.get(f), ch.dp)} />
          <span className="text-muted-foreground ml-1 text-[10px]">{ch.unit}</span>
        </span>
      </div>

      <svg
        width="100%"
        height="100%"
        viewBox={`0 0 ${BOX} ${BOX}`}
        preserveAspectRatio="none"
        className="mt-[6px] block min-h-0 flex-1"
      >
        {[25, 50, 75].map((y) => (
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
        {paths.ghosts.map((d, i) => (
          <path
            // eslint-disable-next-line react/no-array-index-key
            key={i}
            d={d}
            className="trace-ghost"
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {paths.predicted && (
          <path d={paths.predicted} className="trace-predicted" vectorEffect="non-scaling-stroke" />
        )}
        <path
          d={paths.main}
          fill="none"
          stroke={spec.context ? "var(--predicted)" : "var(--measured)"}
          strokeWidth="2"
          vectorEffect="non-scaling-stroke"
        />
        <line
          ref={head}
          x1="0"
          y1="0"
          x2="0"
          y2={BOX}
          stroke="var(--foreground)"
          strokeWidth="1"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
    </div>
  );
}

/**
 * Which six channels, and which cylinder the two per-cylinder cells follow.
 *
 * Ranked on the largest exhaust residual seen anywhere in the recording, which
 * is the same quantity OPS accents a cylinder on. A recording with no twin in it
 * falls back to cylinder 1 rather than drawing nothing.
 */
function layout(frames: Frame[]): StripSpec[] {
  const worst = worstCylinder(frames);
  const siblings = [1, 2, 3, 4].filter((c) => c !== worst);
  return [
    { id: "rpm", ghosts: [], context: true },
    { id: "map", ghosts: [], context: true },
    {
      id: `egt${worst}`,
      ghosts: siblings.map((c) => `egt${c}`),
      predicted: `egt${worst}_twin`,
      context: false,
    },
    {
      id: `cht${worst}`,
      ghosts: siblings.map((c) => `cht${c}`),
      predicted: `cht${worst}_twin`,
      context: false,
    },
    { id: "oil_p", ghosts: [], context: true },
    { id: "vib", ghosts: [], context: true },
  ];
}

function worstCylinder(frames: Frame[]): number {
  let best = 1;
  let largest = 0;
  for (const frame of frames) {
    const normalised = frame.twin?.normalised;
    if (!normalised) continue;
    for (let c = 0; c < 4; c += 1) {
      const magnitude = Math.abs(normalised[TWIN.EGT + c] ?? 0);
      if (magnitude > largest) {
        largest = magnitude;
        best = c + 1;
      }
    }
  }
  return best;
}

/**
 * Every path in one cell, on one scale.
 *
 * The siblings share the featured channel's scale on purpose: four exhaust
 * temperatures each normalised to its own extremes would show every cylinder
 * diverging by the same amount, which is the opposite of what happened.
 */
function traces(
  frames: Frame[],
  spec: StripSpec,
): { main: string; ghosts: string[]; predicted?: string } {
  const ids = [spec.id, ...spec.ghosts, ...(spec.predicted ? [spec.predicted] : [])];
  const all = ids.map((id) => series(frames, channel(id).get, MAX_POINTS));
  const scale = span(all);
  const paths = all.map((one) => path(one.values, scale));
  return {
    main: paths[0] ?? "",
    ghosts: paths.slice(1, 1 + spec.ghosts.length),
    predicted: spec.predicted ? paths.at(-1) : undefined,
  };
}
