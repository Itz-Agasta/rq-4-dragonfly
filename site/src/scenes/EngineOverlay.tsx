import { useLayoutEffect, useRef } from "react";

import { useFootageViewBox } from "../film/crop";

// Overlays drawn in the footage's own pixel space. Coordinates were measured on the
// last frame of site/gen/masters/S2.mp4 at 1600x900; re-measure if S2 is re-rendered.
// preserveAspectRatio "xMidYMid slice" is the SVG spelling of the canvas cover fit,
// so these stay registered on the engine at any window aspect.

type Box = { x: number; y: number; w: number; h: number; label: string; hot?: boolean };

/**
 * Focal x of the engine bay in footage space. On narrow screens the crop centres
 * here instead of on the frame, so the bay stays in view; scenes pass the same value
 * to the canvas (Frame.fx) or the overlays drift off the metal.
 */
export const ENGINE_FX = 665;

// Bore rims measured at 2x on a 10 px grid: (cx, cy, rx, ry, depth to the block).
// The bores step up toward the prop because the camera looks slightly down the bay.
const BORES = [
  [462, 499, 40, 14, 76],
  [537, 490, 37, 14, 77],
  [605, 480, 37, 13, 77],
  [675, 471, 37, 13, 76],
] as const;

// Cylinders numbered left to right as the reader sees them. The fault story is
// cylinder 3; the ordinal is ours, not the E4P firing order.
const BOXES: Box[] = [
  ...BORES.map(([cx, cy, rx, ry, d], k) => ({
    x: cx - rx - 4,
    y: cy - ry - 6,
    w: 2 * rx + 8,
    h: d + ry + 14,
    label: `CYL ${k + 1}`,
    hot: k === 2,
  })),
  { x: 726, y: 426, w: 204, h: 134, label: "TURBO · MAP · MAT" },
];

function Corners({ x, y, w, h, hot }: Box) {
  const k = 10;
  const d = `M${x},${y + k}V${y}H${x + k} M${x + w - k},${y}H${x + w}V${y + k} M${x + w},${y + h - k}V${y + h}H${x + w - k} M${x + k},${y + h}H${x}V${y + h - k}`;
  return (
    <path
      d={d}
      fill="none"
      stroke={hot ? "var(--primary)" : "var(--fg)"}
      strokeWidth={1.25}
      vectorEffect="non-scaling-stroke"
    />
  );
}

/**
 * Armory-grammar brackets on each instrumented part, revealed one by one inside the
 * scene window [from, to]. They clear before the camera dives into cylinder 3.
 */
export function ChannelBoxes({ from, to }: { from: number; to: number }) {
  const vb = useFootageViewBox(ENGINE_FX);
  return (
    <svg className="frame-svg" viewBox={vb} preserveAspectRatio="xMidYMid slice" aria-hidden>
      {BOXES.map((b, i) => (
        <g key={b.label} data-at={`${from + i * 0.025},${to}`}>
          <rect
            x={b.x}
            y={b.y}
            width={b.w}
            height={b.h}
            fill="none"
            stroke="var(--fg)"
            strokeOpacity={0.14}
            vectorEffect="non-scaling-stroke"
          />
          <Corners {...b} />
          <text x={b.x} y={b.y - 8} style={b.hot ? { fill: "var(--primary)" } : undefined}>
            {b.label}
          </text>
        </g>
      ))}
      <text
        className="caption"
        x={418}
        y={612}
        data-at={`${from + 0.14},${to}`}
        style={{ fill: "var(--muted)" }}
      >
        CHT · EGT · lambda every cylinder · oil P · oil T · fuel flow · crank rpm
      </text>
    </svg>
  );
}

// One bore: open top ellipse, walls down to the block, lower arc.
const bore = (cx: number, cy: number, rx: number, ry: number, depth: number) =>
  `M${cx - rx},${cy} a${rx},${ry} 0 1,0 ${2 * rx},0 a${rx},${ry} 0 1,0 ${-2 * rx},0 ` +
  `M${cx - rx},${cy} V${cy + depth} M${cx + rx},${cy} V${cy + depth} ` +
  `M${cx - rx},${cy + depth} a${rx},${ry} 0 0,0 ${2 * rx},0`;

const TWIN = [
  ...BORES.map(([cx, cy, rx, ry, d]) => bore(cx, cy, rx, ry, d)),
  // turbocharger housing and the outlet elbow
  "M730,492 a80,62 0 1,0 160,0 a80,62 0 1,0 -160,0",
  "M884,462 C908,450 926,470 923,500 C921,522 906,532 890,534",
  // injector rail across the post tops
  "M425,445 L687,414",
];

/**
 * The twin, in the GCS grammar: dashed hairline over the solid measured world. It
 * wipes on left to right with the section's `--p` (see .twin in styles.css).
 */
export function TwinTrace() {
  const vb = useFootageViewBox(ENGINE_FX);
  return (
    <svg className="frame-svg twin" viewBox={vb} preserveAspectRatio="xMidYMid slice" aria-hidden>
      {TWIN.map((d, i) => (
        <path
          key={i}
          d={d}
          fill="none"
          stroke="#f0f0f2"
          strokeWidth={1.5}
          vectorEffect="non-scaling-stroke"
        />
      ))}
    </svg>
  );
}

// The reveal's part labels, armory's product-label grammar: a dark chip with a
// leader line onto the part it names. Anchors are the measured geometry above.
const PARTS = [
  // Chips stay above y 600: below that, at common aspects, is where the scene's
  // lower copy sits once the cover crop is applied.
  { label: "Engine model · RK4 at 200 Hz", to: [560, 470], at: [300, 300] },
  { label: "28-state unscented filter", to: [810, 470], at: [900, 300] },
  { label: "22 channels", to: [470, 575], at: [130, 560] },
  { label: "Residual · measured minus predicted", to: [700, 560], at: [960, 590] },
] as const;

/** Names what DRAGONFLY adds to the engine, one label at a time inside [from, 1]. */
export function PartLabels({ from }: { from: number }) {
  const vb = useFootageViewBox(ENGINE_FX);
  const svg = useRef<SVGSVGElement>(null);
  // Chips are sized from the rendered text, not a per-character guess: letter
  // spacing and the late-loading web font both change the width.
  useLayoutEffect(() => {
    const fit = () =>
      svg.current?.querySelectorAll("g").forEach((g) => {
        const t = g.querySelector("text");
        const r = g.querySelector("rect");
        if (t && r) r.setAttribute("width", String(t.getComputedTextLength() + 30));
      });
    fit();
    document.fonts?.ready.then(fit);
  }, []);
  return (
    <svg
      ref={svg}
      className="frame-svg parts"
      viewBox={vb}
      preserveAspectRatio="xMidYMid slice"
      aria-hidden
    >
      {PARTS.map((p, i) => {
        const w = p.label.length * 9.6 + 30;
        return (
          <g key={p.label} data-at={`${from + i * 0.05},1.01`}>
            <line x1={p.to[0]} y1={p.to[1]} x2={p.at[0] + w / 2} y2={p.at[1]} />
            <circle cx={p.to[0]} cy={p.to[1]} r={3} />
            <rect x={p.at[0]} y={p.at[1] - 16} width={w} height={32} />
            <text x={p.at[0] + 14} y={p.at[1] + 4}>
              {p.label}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
