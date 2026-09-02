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
import { useNavigate } from "react-router";

import { useFaultCylinder } from "@/components/ops/fault";
import { SchematicDrawing } from "@/components/ops/SchematicDrawing";
import { Button } from "@/components/ui/button";
import { fmt, grouped, NO_VALUE } from "@/lib/fmt";
import { useLiveSink, useLiveText } from "@/lib/live";
import { type Frame, isFresh } from "@/lib/telemetry";
import { useApp } from "@/store/app";
import type { SourceKey } from "@/store/frame";

// The MAF box shows lambda alongside, which arrives on a different message, so
// it answers to both. See the channel registry for who carries what.
const ENGINE: readonly SourceKey[] = ["engine_ms"];
const AUX: readonly SourceKey[] = ["auxiliary_ms"];
const AUX_AND_ENGINE: readonly SourceKey[] = ["auxiliary_ms", "engine_ms"];

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
  /**
   * Bus sources feeding this callout. The box goes stale when any one of them
   * goes quiet, not when all of them do.
   */
  sources: readonly SourceKey[];
  accent?: boolean;
  /**
   * Channel this box opens on TWIN, if it has one.
   *
   * A box without one is not a dead link, it is a reading with no twin behind
   * it, and the two must not look the same: only the ones that resolve are
   * given a hover state and a focus ring.
   */
  channel?: string;
  children: React.ReactNode;
}

function Callout({ x, y, width, label, sources, accent = false, channel, children }: CalloutProps) {
  const value = useRef<SVGTextElement>(null);
  const flag = useRef<SVGTSpanElement>(null);

  const navigate = useNavigate();
  const select = useApp((s) => s.select);

  useLiveSink((f) => {
    // Any rather than all, deliberately. Marking a live value stale understates
    // its health; leaving a frozen one bright overstates it, and only one of
    // those two errors gets an operator to trust a number that stopped moving.
    const stale = sources.some((s) => !isFresh(f.ages[s]));
    const el = value.current;
    if (el && stale !== el.hasAttribute("data-stale")) el.toggleAttribute("data-stale", stale);
    // Dimming alone cannot be read as anything in particular, so the box also
    // says the word, in the label rather than over the value.
    const mark = flag.current;
    const text = stale ? " · STALE" : "";
    if (mark && mark.textContent !== text) mark.textContent = text;
  });

  const follow = () => {
    if (!channel) return;
    select({ channel });
    void navigate("/twin");
  };

  return (
    <g
      // Screens are drill-downs: this is the click that follows a fault from a
      // reading to the disagreement behind it, rather than navigating to TWIN
      // and hunting for the channel again.
      {...(channel
        ? {
            role: "link",
            tabIndex: 0,
            "aria-label": `${label}, open on TWIN`,
            onClick: follow,
            onKeyDown: (e: React.KeyboardEvent) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                follow();
              }
            },
            className:
              "pointer-events-auto cursor-pointer [&>rect]:hover:fill-[var(--accent)] focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-[var(--ring)]",
          }
        : { "aria-hidden": true })}
    >
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
        <tspan ref={flag} fill="var(--foreground-dim)" />
      </text>
      <text
        ref={value}
        x={x + 12}
        y={y + 40}
        fontSize="14"
        fill={accent ? ACCENT : "var(--foreground)"}
      >
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
function Overlay({ fault }: { fault: number }) {
  return (
    <svg
      viewBox="0 0 1120 610"
      preserveAspectRatio="xMidYMid meet"
      className="pointer-events-none absolute inset-0"
      width="100%"
      height="100%"
      style={{ fontFamily: "var(--font-mono)", fontVariantNumeric: "tabular-nums" }}
    >
      <g stroke={KEY_EDGE} strokeWidth="1" fill="none" aria-hidden="true">
        {[0, 1, 2, 3].map((i) => (
          <line
            key={`egt-leader-${i}`}
            x1={352 + i * 100}
            y1="54"
            x2={352 + i * 100}
            y2="62"
            stroke={i + 1 === fault ? ACCENT : KEY_EDGE}
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
          sources={ENGINE}
          channel={`egt${i + 1}`}
          accent={i + 1 === fault}
        >
          <LiveSpan select={(f) => fmt(f.egt_k[i] ?? Number.NaN, 0)} />
          <Unit>K</Unit>
        </Callout>
      ))}

      <Callout x={20} y={6} width={188} label="MAP · INTAKE PLENUM" sources={ENGINE} channel="map">
        <LiveSpan select={(f) => grouped(f.map_pa / 100)} />
        <Unit>hPa</Unit>
        <tspan fill="var(--foreground-dim)" fontSize="11" dx="10">
          <LiveSpan select={(f) => `${fmt(f.boost_pa / 1e5, 2)} bar`} />
        </tspan>
      </Callout>

      <Callout x={920} y={20} width={188} label="TURBO SPEED" sources={AUX} channel="tc_rpm">
        <LiveSpan select={(f) => grouped(f.tc_rpm)} />
        <Unit>rpm</Unit>
      </Callout>

      <Callout
        x={920}
        y={150}
        width={188}
        label="MAF · COMPRESSOR IN"
        sources={AUX_AND_ENGINE}
        channel="maf"
      >
        <LiveSpan select={(f) => fmt(f.maf_kgs, 3)} />
        <Unit>kg/s</Unit>
        <tspan fill="var(--foreground-dim)" fontSize="11" dx="10">
          <LiveSpan select={(f) => `λ ${fmt(f.lambda, 2)}`} />
        </tspan>
      </Callout>

      <Callout x={120} y={480} width={188} label="OIL · SUMP" sources={ENGINE} channel="oil_p">
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
  const fault = useFaultCylinder();

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
        <SchematicDrawing faultCylinder={fault} showDotGrid={showDotGrid} />
        <Overlay fault={fault} />
      </div>
    </section>
  );
}
