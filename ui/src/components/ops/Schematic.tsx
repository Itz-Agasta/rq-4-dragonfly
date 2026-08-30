/**
 * The engine schematic panel: header, drawing, and live callouts on the drawing.
 *
 * Callouts are `<text>` inside the drawing's own coordinate space rather than
 * HTML overlaid on it, so a leader line and the box it points at can never drift
 * apart when the panel resizes. Every leader terminates on the component it
 * labels; a line that connects nothing is worse than no line.
 *
 * The values are live off the bus. Only the accented cylinder is authored, and
 * only until faults exist.
 */

import { useRef } from "react";

import { FAULT_CYLINDER } from "@/components/ops/data";
import { SchematicDrawing } from "@/components/ops/SchematicDrawing";
import { Button } from "@/components/ui/button";
import { fmt, grouped, NO_VALUE } from "@/lib/fmt";
import { useLiveText } from "@/lib/live";
import type { Frame } from "@/lib/telemetry";
import { useApp } from "@/store/app";

const STRUCTURE = "var(--structure)";
const KEY_EDGE = "var(--structure-hi)";
const ACCENT = "var(--primary)";

/** A `<tspan>` whose text is rewritten from the render loop. */
function LiveSpan({ select, ...rest }: { select: (f: Frame) => string } & Record<string, unknown>) {
  const ref = useRef<SVGTSpanElement>(null);
  useLiveText(ref, select);
  return (
    <tspan ref={ref} {...rest}>
      {NO_VALUE}
    </tspan>
  );
}

interface CalloutProps {
  x: number;
  y: number;
  width: number;
  label: string;
  accent?: boolean;
  children: React.ReactNode;
}

function Callout({ x, y, width, label, accent = false, children }: CalloutProps) {
  return (
    <g>
      <rect
        x={x}
        y={y}
        width={width}
        height="46"
        fill="var(--card)"
        stroke={accent ? ACCENT : STRUCTURE}
        strokeWidth="1"
      />
      <text
        x={x + 12}
        y={y + 20}
        fill={accent ? ACCENT : "var(--muted-foreground)"}
        fontSize="10"
        letterSpacing="0.8"
      >
        {label}
      </text>
      <text x={x + 12} y={y + 40} fontSize="14" fill={accent ? ACCENT : "var(--foreground)"}>
        {children}
      </text>
    </g>
  );
}

function Unit({ children }: { children: string }) {
  return (
    <tspan fill="var(--muted-foreground)" fontSize="10" dx="3">
      {children}
    </tspan>
  );
}

/** Callouts and their leader lines, in the drawing's 1120x610 space. */
function Overlay() {
  return (
    <svg
      viewBox="0 0 1120 610"
      preserveAspectRatio="xMidYMid meet"
      className="pointer-events-none absolute inset-0"
      width="100%"
      height="100%"
      aria-hidden="true"
      style={{ fontFamily: "var(--font-mono)", fontVariantNumeric: "tabular-nums" }}
    >
      <g stroke={KEY_EDGE} strokeWidth="1" fill="none">
        {[0, 1, 2, 3].map((i) => (
          <line
            key={`egt-leader-${i}`}
            x1={352 + i * 100}
            y1="54"
            x2={352 + i * 100}
            y2="62"
            stroke={i + 1 === FAULT_CYLINDER ? ACCENT : KEY_EDGE}
          />
        ))}
        <line x1="114" y1="54" x2="114" y2="118" />
        <path d="M960 66 L900 78" />
        <path d="M1014 196 L1014 250 L910 250" />
        <path d="M308 503 L324 505" />
      </g>

      {[0, 1, 2, 3].map((i) => (
        <Callout
          key={`egt-${i}`}
          x={308 + i * 100}
          y={6}
          width={88}
          label={`EGT ${i + 1}`}
          accent={i + 1 === FAULT_CYLINDER}
        >
          <LiveSpan select={(f) => fmt(f.egt_k[i] ?? Number.NaN, 0)} />
          <Unit>K</Unit>
        </Callout>
      ))}

      <Callout x={20} y={6} width={188} label="MAP · INTAKE PLENUM">
        <LiveSpan select={(f) => grouped(f.map_pa / 100)} />
        <Unit>hPa</Unit>
        <tspan fill="var(--foreground-dim)" fontSize="11" dx="10">
          <LiveSpan select={(f) => `${fmt(f.boost_pa / 1e5, 2)} bar`} />
        </tspan>
      </Callout>

      <Callout x={920} y={20} width={188} label="TURBO SPEED">
        <LiveSpan select={(f) => grouped(f.tc_rpm)} />
        <Unit>rpm</Unit>
      </Callout>

      <Callout x={920} y={150} width={188} label="MAF · COMPRESSOR IN">
        <LiveSpan select={(f) => fmt(f.maf_kgs, 3)} />
        <Unit>kg/s</Unit>
        <tspan fill="var(--foreground-dim)" fontSize="11" dx="10">
          <LiveSpan select={(f) => `λ ${fmt(f.lambda, 2)}`} />
        </tspan>
      </Callout>

      <Callout x={120} y={480} width={188} label="OIL · SUMP">
        <LiveSpan select={(f) => fmt(f.oil_p_pa / 1e5, 2)} />
        <Unit>bar</Unit>
        <tspan fill="var(--foreground-dim)" fontSize="11" dx="10">
          <LiveSpan select={(f) => `${fmt(f.oil_t_k, 0)} K`} />
        </tspan>
      </Callout>
    </svg>
  );
}

export function Schematic() {
  const showDotGrid = useApp((s) => s.showDotGrid);

  return (
    <section className="bg-card marks relative flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex h-10 shrink-0 items-center justify-between gap-5 px-[18px]">
        <div className="flex min-w-0 items-baseline gap-[14px]">
          <span className="t-section tracking-[0.02em] whitespace-nowrap">ENGINE SCHEMATIC</span>
          <span className="label-micro truncate">
            Inline-4 · heavy-fuel CI · turbo · side elevation
          </span>
        </div>
        {/*
          Lives here rather than in the strip grid below. In the reference layout
          it occupies a seventh cell in a six-cell grid, which breaks the column
          alignment the lattice depends on. Near-white outline, never accent: the
          accent means alarm, and a button is not one.
        */}
        <Button size="sm" className="shrink-0" disabled>
          INJECT FAULT
        </Button>
      </div>

      <div className="relative min-h-0 min-w-0 flex-1">
        <SchematicDrawing faultCylinder={FAULT_CYLINDER} showDotGrid={showDotGrid} />
        <Overlay />
      </div>
    </section>
  );
}
