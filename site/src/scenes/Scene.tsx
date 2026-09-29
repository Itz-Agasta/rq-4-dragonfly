import { useLayoutEffect, useRef, type ReactNode } from "react";

import { scrub, type FrameMap } from "../film/scroll";
import type { SeqId } from "../film/sequences";

type Props = {
  /** Scroll length in viewport heights. Longer means slower footage per wheel tick. */
  vh: number;
  map: FrameMap | null;
  preload?: SeqId[];
  stage?: string;
  id?: string;
  /**
   * The stage paints a solid background (dither, cards) rather than letting the film
   * show through. Decides how the neighbouring handovers dissolve; see scrub().
   */
  opaque?: boolean;
  /** Per-tick hook for overlays CSS cannot drive, like a counting readout. */
  onP?: (p: number) => void;
  children: ReactNode;
};

/** A scroll span with a fixed stage that crossfades in and out; see scrub(). */
export function Scene({
  vh,
  map,
  preload = [],
  stage = "",
  id,
  opaque = false,
  onP,
  children,
}: Props) {
  const ref = useRef<HTMLElement>(null);
  // map, preload and onP are fixed per scene; binding once is intentional, and
  // re-binding on every render would recreate the ScrollTriggers mid-scroll.
  // oxlint-disable-next-line react-hooks/exhaustive-deps
  useLayoutEffect(() => scrub(ref.current!, map, preload, onP), []);
  return (
    <section
      ref={ref}
      id={id}
      className="scene"
      style={{ height: `calc(${vh}vh * var(--scroll-scale, 1))` }}
      data-opaque={opaque ? "1" : undefined}
    >
      <div className={`stage ${stage}`}>
        <div className="stage-inner">{children}</div>
      </div>
    </section>
  );
}
