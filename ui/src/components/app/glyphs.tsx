/**
 * Line-art glyphs for the navigation rail.
 *
 * Drawn here rather than pulled from an icon library because they have to read as
 * the same hand as the engine schematic: 1.5px strokes on a 20px box, no fills,
 * no rounded joins, no pictograms. An icon set's house style is the fastest way to
 * make an instrument panel look like a web app.
 *
 * Every glyph strokes `currentColor` so the active rail cell can invert by setting
 * a text colour and nothing else.
 */

import type { ScreenId } from "@/store/app";

interface GlyphProps {
  className?: string;
}

function Frame({
  children,
  className,
  size = 20,
}: {
  children: React.ReactNode;
  className?: string;
  size?: number;
}) {
  return (
    <svg
      viewBox="0 0 20 20"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="butt"
      strokeLinejoin="miter"
      aria-hidden="true"
      className={className}
    >
      {children}
    </svg>
  );
}

/**
 * The product mark: a dragonfly in plan, which is also an inline-4 on its crank
 * axis. Spine is the crank, the four wings are the four cylinders.
 *
 * Each wing *pair* is one continuous stroke through the body rather than two
 * wings meeting it, and nothing here is dashed. Four separate wings, a head dot
 * and a dashed twin-edge were all drawn and all dissolve below 24px, which is
 * the only size that matters: the rail cell and the 16px favicon. The head dot
 * additionally made it read as a stick figure.
 *
 * **The hind pair is wider than the fore pair on purpose.** A wide-top,
 * narrow-bottom silhouette is an arrowhead, and every symmetric or fore-wide
 * variant read as an arrow or an asterisk instead of an insect. Do not
 * "correct" the asymmetry.
 *
 * Stays `currentColor`, and the rail renders it in the foreground grey, not the
 * accent: a permanently orange element in the chrome competes with a real alarm
 * for the one colour that means attention. The favicon is the exception and
 * takes the accent, because a browser tab has no alarm semantics to protect.
 */
function Mark({ className, size }: GlyphProps & { size?: number }) {
  return (
    <Frame className={className} size={size}>
      <path d="M10 2.2V17.9" />
      <path d="M3 4.9 10 7.1 17 4.9" />
      <path d="M2.2 13.1 10 10.3 17.8 13.1" />
    </Frame>
  );
}

/** A piston in its bore, on a rod, over the crank. */
function OpsGlyph({ className }: GlyphProps) {
  return (
    <Frame className={className}>
      <path d="M4.5 2.5v7M15.5 2.5v7" />
      <path d="M5.5 3.5h9v5h-9z" />
      <path d="M10 8.5v5" />
      <circle cx="10" cy="15.5" r="2.5" />
    </Frame>
  );
}

/** Two traces that start together and separate: measured against predicted. */
function TwinGlyph({ className }: GlyphProps) {
  return (
    <Frame className={className}>
      <path d="M2.5 10.5h4c3 0 3.5-6 11-7.5" />
      <path d="M2.5 10.5h4c3 0 3.5 5 11 6.5" strokeDasharray="2.5 2" />
    </Frame>
  );
}

/** Residual matrix. */
function AnalysisGlyph({ className }: GlyphProps) {
  return (
    <Frame className={className}>
      <path d="M3 3h14v14H3z" />
      <path d="M10 3v14M3 10h14" />
    </Frame>
  );
}

/** A commanded step. */
function SimulateGlyph({ className }: GlyphProps) {
  return (
    <Frame className={className}>
      <path d="M2.5 15.5h6v-11h9" />
      <path d="M2.5 15.5h6c5 0 3-7 9-7" strokeDasharray="2.5 2" />
    </Frame>
  );
}

/** Timeline with a playhead. */
function ReplayGlyph({ className }: GlyphProps) {
  return (
    <Frame className={className}>
      <path d="M3 6.5h14v7H3z" />
      <path d="M7.5 3.5v13" />
    </Frame>
  );
}

/** Three airframes, ranked. */
function FleetGlyph({ className }: GlyphProps) {
  return (
    <Frame className={className}>
      <path d="M4 6.5 10 2.5l6 4M4 12 10 8l6 4M4 17.5 10 13.5l6 4" />
    </Frame>
  );
}

/** Aperture, adjusted. */
function SettingsGlyph({ className }: GlyphProps) {
  return (
    <Frame className={className}>
      <circle cx="10" cy="10" r="4" />
      <path d="M10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2" />
    </Frame>
  );
}

/** Information. */
function AboutGlyph({ className }: GlyphProps) {
  return (
    <Frame className={className}>
      <circle cx="10" cy="10" r="7.5" />
      <path d="M10 9.5v4.5M10 6.2v1.3" />
    </Frame>
  );
}

export const SCREEN_GLYPHS: Record<ScreenId, (props: GlyphProps) => React.ReactElement> = {
  ops: OpsGlyph,
  twin: TwinGlyph,
  analysis: AnalysisGlyph,
  simulate: SimulateGlyph,
  replay: ReplayGlyph,
  fleet: FleetGlyph,
};

export { AboutGlyph, Mark, SettingsGlyph };
