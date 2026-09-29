import { useEffect, useState } from "react";

// The footage is authored at 16:9 and drawn with a cover fit. On anything narrower
// (a phone in portrait shows ~26% of the width) a centred crop cuts off subjects
// that sit off-centre, so a scene can name a focal x in the 1600x900 footage space.
// The canvas and the frame-locked SVG overlays both crop through here, which is the
// only thing keeping the overlays registered on the engine.
export const FOOTAGE_W = 1600;
export const FOOTAGE_H = 900;

/** Left edge of the cover-fitted footage in canvas pixels, centred on `fx`. */
export function coverLeft(cw: number, ch: number, fx = FOOTAGE_W / 2) {
  const s = Math.max(cw / FOOTAGE_W, ch / FOOTAGE_H);
  const w = FOOTAGE_W * s;
  return Math.min(0, Math.max(cw - w, cw / 2 - fx * s));
}

/** The viewBox an overlay needs to match coverLeft at the current window size. */
export function useFootageViewBox(fx = FOOTAGE_W / 2) {
  const calc = () => {
    const a = innerWidth / innerHeight;
    if (a >= FOOTAGE_W / FOOTAGE_H) return `0 0 ${FOOTAGE_W} ${FOOTAGE_H}`;
    const vw = FOOTAGE_H * a;
    const vx = Math.min(FOOTAGE_W - vw, Math.max(0, fx - vw / 2));
    return `${vx.toFixed(1)} 0 ${vw.toFixed(1)} ${FOOTAGE_H}`;
  };
  const [vb, setVb] = useState(calc);
  useEffect(() => {
    const on = () => setVb(calc());
    addEventListener("resize", on);
    return () => removeEventListener("resize", on);
    // calc closes over fx only, which is a per-scene constant.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [fx]);
  return vb;
}
