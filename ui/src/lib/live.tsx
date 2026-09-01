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
 *
 * # Two cadences, and why a readout is not on the fast one
 *
 * Traces are redrawn as often as the display can show them. **Text is not.** A
 * four-decimal value rewritten on every 20 Hz frame is not a readable number, it
 * is a flicker with a mean, and `design.md` 5's tabular figures fix the horizontal
 * shiver without touching the rate. Readouts are therefore written at
 * {@link READOUT_HZ}, which is the cadence a mechanical instrument settles at and
 * roughly the fastest a person reads a digit.
 *
 * The cost is bounded and it is stated: a displayed digit is at most one readout
 * period old, 200 ms, which is inside the 250 ms at which the app calls a source
 * stale in the first place. So nothing can be shown as current that the link
 * status would already be calling frozen.
 */

import { type RefObject, useLayoutEffect, useRef } from "react";

import { NO_VALUE } from "@/lib/fmt";
import type { Frame } from "@/lib/telemetry";
import { telemetry } from "@/store/telemetry";

type Sink = (frame: Frame) => void;

/**
 * Rate at which text readouts are rewritten.
 *
 * Five, not the 20 Hz the frames arrive at. Both halves of that matter: slower
 * and a value visibly lags the trace beside it, faster and the digits cannot be
 * read. The period must stay below the 250 ms staleness threshold in
 * `telemetry.isFresh` or a stale value could be displayed as current.
 */
export const READOUT_HZ = 5;

const READOUT_PERIOD_MS = 1000 / READOUT_HZ;

const sinks = new Set<Sink>();
const readouts = new Set<Sink>();
let handle = 0;
let lastReadout = 0;

function tick(now: number): void {
  const frame = telemetry.latest;
  if (frame) {
    for (const sink of sinks) sink(frame);
    if (now - lastReadout >= READOUT_PERIOD_MS) {
      lastReadout = now;
      for (const sink of readouts) sink(frame);
    }
  }
  handle = requestAnimationFrame(tick);
}

function start(): void {
  if (handle === 0) {
    // Zero rather than `now`, so the first readout lands on the first frame
    // instead of a fifth of a second after a screen appears.
    lastReadout = 0;
    handle = requestAnimationFrame(tick);
  }
}

function stop(): void {
  if (sinks.size === 0 && readouts.size === 0) {
    cancelAnimationFrame(handle);
    handle = 0;
  }
}

/**
 * Run `sink` once per animation frame with the most recent telemetry frame.
 *
 * For traces and anything else that is drawn rather than read. Text belongs on
 * {@link subscribeReadout}.
 *
 * The loop only runs while something is subscribed, so a screen with no live
 * elements costs nothing.
 */
export function subscribe(sink: Sink): () => void {
  sinks.add(sink);
  start();
  return () => {
    sinks.delete(sink);
    stop();
  };
}

/** Run `sink` at {@link READOUT_HZ}, for anything that writes text. */
export function subscribeReadout(sink: Sink): () => void {
  readouts.add(sink);
  start();
  return () => {
    readouts.delete(sink);
    stop();
  };
}

/**
 * Subscribe for the lifetime of a component, at the readout rate.
 *
 * Every component that uses this writes text, so there is no full-rate option
 * here. One that needs the frame rate wants a trace, and a trace calls
 * {@link subscribe} from its own effect the way `Strip` does.
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
  useLayoutEffect(() => subscribeReadout((frame) => ref.current(frame)), []);
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
