/**
 * Resolve a theme token to a concrete colour.
 *
 * uPlot hands `stroke` straight to the canvas 2D context, and `strokeStyle` does
 * not understand `var(--x)` — it silently ignores the assignment and keeps the
 * previous value, so a trace styled with a CSS variable draws in black on black
 * and looks like no data at all. SVG resolves variables and canvas does not; this
 * is the seam between them.
 */
export function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}
