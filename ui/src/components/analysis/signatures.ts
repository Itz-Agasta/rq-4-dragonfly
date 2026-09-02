/**
 * How the signature matrix is drawn.
 *
 * The luminance ramp, the sign cap and the abbreviations the rotated column
 * labels need. Separate from `lib/signatures`, which is the wire contract: TWIN
 * wants the parameter descriptors from that endpoint and has no matrix to paint.
 */

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
