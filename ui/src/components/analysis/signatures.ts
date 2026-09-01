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
 * Runs between {@link FLOOR} and {@link CEILING}, which carry why they sit where
 * they do.
 */
export function ramp(value: number): string {
  const v = Math.min(Math.abs(value), 1);
  const level = Math.round(FLOOR + Math.pow(v, CONTRAST) * (CEILING - FLOOR));
  return `rgb(${level} ${level} ${level})`;
}

/**
 * Luminance of a cell whose channel the fault does not move.
 *
 * The panel colour, so an unmoved channel is not drawn at all. **145 of the 198
 * cells are zero**, so every point of floor is paid 145 times: a floor of 28 was
 * tried and turned the panel into a field of grey tiles that the few channels a
 * fault actually moves had to compete with. Lit cells against black is the
 * stronger reading, and it is the one chosen after seeing both on the bus.
 *
 * The cost is real and accepted: an empty cell and the gap between two cells look
 * the same, so columns cannot be counted along a row and the observed row cannot
 * be tracked up a column by eye. If that ever has to be recovered, the way is a
 * 1px `--border` outline per cell, which restores the grid without lifting the
 * fill, rather than raising this number.
 */
const FLOOR = 10;

/**
 * Luminance of a channel carrying the whole of a unit signature.
 *
 * Reachable rather than theoretical: a fault expressing in exactly one channel
 * puts its whole unit length there. Two in the catalogue do, EGT 3 sensor drift
 * and oil supply loss, and they are the rows a reader picks out first.
 */
const CEILING = 240;

/**
 * Gamma on the magnitude.
 *
 * Reads backwards, so it is worth stating: **lowering this brightens the panel by
 * brightening the small values hardest.** With a median non-zero component of 0.27
 * that is a uniform mid-grey wash which buries the few channels a fault actually
 * moves, so reaching for a lower exponent to make the matrix brighter makes it
 * less legible. Raising it pushes weak components down and leaves the peak, which
 * is what carries the signature, standing alone.
 */
const CONTRAST = 0.62;

/**
 * Channel name shortened for the matrix's rotated column labels.
 *
 * Rotated text is as tall as the name is long, and the band has to fit the
 * longest one, so a single eight-character channel costs **every** column that
 * height as empty space above a three-character name. `LAMBDA 3` is the
 * offender; the greek letter is what the instrument panel uses anyway.
 *
 * Only the matrix header abbreviates. Attribution names a channel in a sentence
 * an operator reads once, where the full word is worth its width.
 */
export function short(channel: string): string {
  if (channel.startsWith("LAMBDA ")) return `\u03bb ${channel.slice(7)}`;
  if (channel === "TORQUE") return "TRQ";
  if (channel === "COOLANT") return "COOL";
  return channel;
}

/**
 * The ramp itself, as a gradient, for the legend.
 *
 * Generated rather than hand-picked so a reader can match a cell against it. A
 * legend whose stops were chosen by eye is a second encoding that disagrees with
 * the first one everywhere except at its ends.
 */
export function rampGradient(): string {
  const stops = [0, 0.25, 0.5, 0.75, 1].map(ramp).join(", ");
  return `linear-gradient(90deg, ${stops})`;
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
