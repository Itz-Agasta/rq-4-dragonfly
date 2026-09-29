import { useEffect, useRef } from "react";

import { mountDither, type Dither } from "../film/dither";

/**
 * The dithered cloud, filling its stage. The scene drives it through `bind`, which
 * receives a setter for how far the cloud has lifted out of the black (0 to 1).
 */
export function DitherBg({ bind }: { bind: (set: (r: number) => void) => void }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const d: Dither = mountDither(ref.current!);
    bind(d.setReveal);
    return d.destroy;
    // bind is a stable scene-level registrar; mounting twice would leak a GL context.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return <canvas ref={ref} className="dither" aria-hidden />;
}
