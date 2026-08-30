/**
 * The render loop.
 *
 * One `requestAnimationFrame` loop drives every live element in the app. Values
 * are written straight to the DOM and charts are fed straight from the ring
 * buffers; React never re-renders for a telemetry frame.
 *
 * Two reasons it is one loop rather than one per component. It coalesces: frames
 * arrive at 20 Hz and the display refreshes at 60, so a per-component timer would
 * do the same work three times for the same data. And it orders: every readout on
 * screen is written from the same frame, so two numbers that should agree cannot
 * be caught disagreeing.
 */

import { type RefObject, useLayoutEffect, useRef } from "react";

import { NO_VALUE } from "@/lib/fmt";
import type { Frame } from "@/lib/telemetry";
import { telemetry } from "@/store/telemetry";

type Sink = (frame: Frame) => void;

const sinks = new Set<Sink>();
let handle = 0;

function tick(): void {
  const frame = telemetry.latest;
  if (frame) {
    for (const sink of sinks) sink(frame);
  }
  handle = requestAnimationFrame(tick);
}

/**
 * Run `sink` once per animation frame with the most recent telemetry frame.
 *
 * The loop only runs while something is subscribed, so a screen with no live
 * elements costs nothing.
 */
export function subscribe(sink: Sink): () => void {
  sinks.add(sink);
  if (handle === 0) handle = requestAnimationFrame(tick);
  return () => {
    sinks.delete(sink);
    if (sinks.size === 0) {
      cancelAnimationFrame(handle);
      handle = 0;
    }
  };
}

/**
 * Subscribe for the lifetime of a component.
 *
 * The callback is held in a ref and the subscription is never torn down on a
 * changed closure. Without that, an inline arrow function would resubscribe on
 * every render, which is both wasteful and a way to miss frames.
 */
export function useLiveSink(sink: Sink): void {
  const ref = useRef(sink);
  useLayoutEffect(() => {
    ref.current = sink;
  });
  useLayoutEffect(() => subscribe((frame) => ref.current(frame)), []);
}

/**
 * Drive an element's text from the render loop.
 *
 * Works on SVG `<text>` and `<tspan>` as well as HTML, which is why it takes a
 * ref rather than rendering an element: the schematic's callouts live inside the
 * drawing's coordinate space and cannot be HTML overlaid on top of it.
 */
export function useLiveText(
  ref: RefObject<{ textContent: string | null } | null>,
  select: (frame: Frame) => string,
): void {
  useLiveSink((frame) => {
    const el = ref.current;
    if (!el) return;
    const next = select(frame);
    if (el.textContent !== next) el.textContent = next;
  });
}

export interface LiveProps {
  /** Produce the text to display from the current frame. */
  select: (frame: Frame) => string;
  /**
   * Whether the value is currently trustworthy. When this returns false the
   * element carries `data-stale`, which the stylesheet dims.
   */
  fresh?: (frame: Frame) => boolean;
  className?: string;
  /** Shown until the first frame arrives. */
  placeholder?: string;
}

/**
 * A span whose text is rewritten from the render loop.
 *
 * Writes only on change, so a value sitting still costs one string comparison per
 * frame and no layout.
 */
export function Live({ select, fresh, className, placeholder = NO_VALUE }: LiveProps) {
  const ref = useRef<HTMLSpanElement>(null);

  useLiveSink((frame) => {
    const el = ref.current;
    if (!el) return;
    const next = select(frame);
    if (el.textContent !== next) el.textContent = next;
    if (fresh) {
      const stale = !fresh(frame);
      if (stale !== el.hasAttribute("data-stale")) {
        el.toggleAttribute("data-stale", stale);
      }
    }
  });

  return (
    <span ref={ref} className={className}>
      {placeholder}
    </span>
  );
}
