import { useEffect, useRef } from "react";

// Film grain, the signal-lost texture armory puts behind "We are redefining the
// defence". One small random tile is regenerated a couple of dozen times a second
// and repeated across the stage at a random offset, which reads as full-screen
// static at a fraction of the cost of filling every pixel every frame.
const TILE = 160;
const ALPHA = 30; // of 255 per grain: texture, not snow
const EVERY_MS = 42; // ~24 Hz; faster reads as flicker, slower as a still

export function NoiseBg() {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current!;
    const ctx = canvas.getContext("2d")!;
    const tile = document.createElement("canvas");
    tile.width = tile.height = TILE;
    const tctx = tile.getContext("2d")!;
    const img = tctx.createImageData(TILE, TILE);
    const fit = () => {
      // Half resolution, upscaled by the browser: grain wants to be a little coarse.
      canvas.width = Math.ceil(canvas.clientWidth / 2);
      canvas.height = Math.ceil(canvas.clientHeight / 2);
    };
    // Fixed stages are always "in the viewport", so IntersectionObserver cannot
    // tell us the scene is off; the stage's own visibility can.
    const stage = canvas.closest<HTMLElement>(".stage");
    const paint = (force = false) => {
      if (!force && stage && stage.style.visibility === "hidden") return;
      const d = img.data;
      for (let i = 0; i < d.length; i += 4) {
        const v = (Math.random() * 255) | 0;
        d[i] = d[i + 1] = d[i + 2] = v;
        d[i + 3] = ALPHA;
      }
      tctx.putImageData(img, 0, 0);
      ctx.fillStyle = "#0b0b0c";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      const pat = ctx.createPattern(tile, "repeat")!;
      pat.setTransform(new DOMMatrix().translate(Math.random() * TILE, Math.random() * TILE));
      ctx.fillStyle = pat;
      ctx.fillRect(0, 0, canvas.width, canvas.height);
    };
    const ro = new ResizeObserver(() => (fit(), paint(true)));
    ro.observe(canvas);
    fit();
    paint(true);
    // Reduced motion gets one still frame of grain; everyone else, only while on screen.
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return () => ro.disconnect();
    let timer = 0;
    const io = new IntersectionObserver(([e]) => {
      clearInterval(timer);
      if (e.isIntersecting) timer = window.setInterval(() => paint(), EVERY_MS);
    });
    io.observe(canvas);
    return () => {
      clearInterval(timer);
      io.disconnect();
      ro.disconnect();
    };
  }, []);
  return <canvas ref={ref} className="dither" aria-hidden />;
}
