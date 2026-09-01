/**
 * The fault signature matrix, fetched once.
 *
 * It is constant for a given engine, so it arrives over HTTP rather than in
 * every frame. `dragonfly-core` generates it at startup by perturbing the engine
 * model and settling it, which is why the client keeps no copy of the numbers and
 * cannot drift from them.
 */

import { useEffect, useState } from "react";

/** One hypothesis row and its axes, mirroring `/api/signatures`. */
export interface Matrix {
  /** Hypothesis names in row order. Index 0 is the null hypothesis. */
  hypotheses: string[];
  /** Subsystem each hypothesis belongs to, indexing `SUBSYSTEMS`. */
  subsystem: number[];
  /** Whether the fault is in the instrument rather than the engine. */
  instrument: boolean[];
  /** Channel names in column order. */
  channels: string[];
  /** Unit-length direction each fault pushes the residual, per channel. */
  rows: number[][];
  /** Health parameters in `theta` order, with the span a trajectory spans. */
  parameters: Parameter[];
}

/**
 * One health parameter's identity and the two values its trajectory runs between.
 *
 * `failure` is the value `prognostics::rul` projects to, served rather than
 * mirrored so a chart cannot draw a threshold that disagrees with the remaining
 * life printed beside it.
 */
export interface Parameter {
  name: string;
  nominal: number;
  failure: number;
}

/**
 * Fetch the matrix, retrying while the core is still starting.
 *
 * Returns null until it arrives. The screen draws its axes from this, so there
 * is nothing to render before it lands and a spinner would be the only honest
 * alternative to waiting.
 */
export function useMatrix(): Matrix | null {
  const [matrix, setMatrix] = useState<Matrix | null>(null);

  useEffect(() => {
    let live = true;
    let timer: number | undefined;

    const load = async () => {
      try {
        const response = await fetch("/api/signatures");
        if (!response.ok) throw new Error(String(response.status));
        const body = (await response.json()) as Matrix;
        if (live) setMatrix(body);
      } catch {
        // The core may not be up yet, or the bus may be down while it starts.
        // Retrying quietly is right: this is a constant, so there is no stale
        // value to worry about and nothing to tell the operator until it fails
        // for long enough that the whole screen is obviously empty.
        if (live) timer = window.setTimeout(() => void load(), 2000);
      }
    };
    void load();

    return () => {
      live = false;
      if (timer !== undefined) window.clearTimeout(timer);
    };
  }, []);

  return matrix;
}

/**
 * Greyscale luminance for a signature component.
 *
 * `design.md` fixes the ramp at `10 + v^contrast * 230`. The exponent below one
 * lifts the middle of the range: most components of a unit vector are small, and
 * a linear ramp leaves a row that genuinely moves five channels looking like a
 * row that moves none.
 */
export function ramp(value: number): string {
  const CONTRAST = 0.62;
  const v = Math.min(Math.abs(value), 1);
  const level = Math.round(10 + Math.pow(v, CONTRAST) * 230);
  return `rgb(${level} ${level} ${level})`;
}

/**
 * Where the signed edge cap sits on a cell, or null when there is nothing to say.
 *
 * The luminance ramp carries magnitude and throws away sign, and sign is the
 * strongest argument on this screen: a coked injector runs its cylinder **cool**
 * where a drifting probe reads it **hot**, so the two rows are opposite rather
 * than merely different. A 2px cap on the top edge for positive and the bottom
 * edge for negative restores that without spending colour, which is reserved.
 */
export function sign(value: number): "top" | "bottom" | null {
  const VISIBLE = 0.08;
  if (Math.abs(value) < VISIBLE) return null;
  return value > 0 ? "top" : "bottom";
}
