import { nearest, onDecoded, type SeqId } from "./sequences";

// One fixed full-viewport canvas for the whole film. Scenes do not own canvases:
// handing the frame between sections by changing (sequence, index) is what makes
// the page read as one continuous shot instead of a stack of videos.

/** A frame position. `f` is fractional: the eased index lives between frames. */
export type Shot = { id: SeqId; f: number };

/**
 * What a scene asks the film to show. `b` crossfades a second shot over `a` at
 * `mix`; `zoom` scales about a point given in the footage's 1600x900 space, the same
 * space the frame-locked SVG overlays use.
 */
export type Frame = { a: Shot; b?: Shot; mix?: number; zoom?: { x: number; y: number; s: number } };

let canvas: HTMLCanvasElement;
let ctx: CanvasRenderingContext2D;
let target: Frame | null = null;
// Eased copies of target.a.f and target.b.f. The drawn index chases the scroll
// rather than snapping to it; this is most of what makes a scrubbed sequence
// read as footage instead of a flipbook.
let easeA: Shot | null = null;
let easeB: Shot | null = null;
let queued = false;
let dirty = true;

// Per-frame fraction of the remaining distance covered. 0.2 settles a jump in
// ~12 frames (200 ms at 60 Hz): slow enough to smooth a wheel's steps, quick
// enough that the film never feels late behind the copy.
const EASE = 0.2;

export function mountFilm(el: HTMLCanvasElement) {
  canvas = el;
  ctx = el.getContext("2d", { alpha: false })!;
  const fit = () => {
    // Capped at 2x: a 3x phone would allocate a 1170x2532 backbuffer to show
    // 960w frames, spending memory on pixels that carry no extra detail.
    const dpr = Math.min(devicePixelRatio, 2);
    canvas.width = Math.round(innerWidth * dpr);
    canvas.height = Math.round(innerHeight * dpr);
    dirty = true;
    schedule();
  };
  addEventListener("resize", fit);
  onDecoded(() => {
    dirty = true;
    schedule();
  });
  fit();
}

/** Requests a frame. Cheap to call every scroll tick; drawing happens once per rAF. */
export function show(frame: Frame) {
  target = frame;
  schedule();
}

function schedule() {
  if (queued) return;
  queued = true;
  requestAnimationFrame(tick);
}

// Chase within a sequence; jump when the sequence changes, because easing from
// frame 140 of one clip to frame 3 of another would play the first clip backwards.
function chase(cur: Shot | null, want: Shot): { s: Shot; moving: boolean } {
  if (!cur || cur.id !== want.id) return { s: { ...want }, moving: true };
  const d = want.f - cur.f;
  if (Math.abs(d) < 0.02) return { s: { ...want }, moving: Math.abs(d) > 0 };
  return { s: { id: cur.id, f: cur.f + d * EASE }, moving: true };
}

function tick() {
  queued = false;
  if (!target || !ctx) return;
  const a = chase(easeA, target.a);
  easeA = a.s;
  let moving = a.moving;
  if (target.b) {
    const b = chase(easeB, target.b);
    easeB = b.s;
    moving ||= b.moving;
  } else easeB = null;
  if (moving || dirty) draw();
  if (moving) schedule();
}

function draw() {
  dirty = false;
  const t = target!;
  const cw = canvas.width;
  const ch = canvas.height;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  if (t.zoom && t.zoom.s !== 1) {
    // Map the zoom point from footage space to canvas space with the same cover
    // arithmetic paint() uses, then scale about it.
    const s = Math.max(cw / 1600, ch / 900);
    const px = (cw - 1600 * s) / 2 + t.zoom.x * s;
    const py = (ch - 900 * s) / 2 + t.zoom.y * s;
    ctx.setTransform(t.zoom.s, 0, 0, t.zoom.s, px * (1 - t.zoom.s), py * (1 - t.zoom.s));
  }
  paint(easeA!, 1);
  // The incoming shot is never zoomed: it lands at 1x so the next scene, which
  // starts it unzoomed, continues without a jump.
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  if (easeB && t.mix) paint(easeB, t.mix);
}

function paint(shot: Shot, alpha: number) {
  const img = nearest(shot.id, Math.round(shot.f));
  // Not loaded yet: keep what is on screen rather than flash black. onDecoded
  // marks the canvas dirty when something lands.
  if (!img) return;
  const iw = img.naturalWidth;
  const ih = img.naturalHeight;
  const cw = canvas.width;
  const ch = canvas.height;
  const s = Math.max(cw / iw, ch / ih);
  const w = iw * s;
  const h = ih * s;
  ctx.globalAlpha = alpha;
  ctx.drawImage(img, (cw - w) / 2, (ch - h) / 2, w, h);
  ctx.globalAlpha = 1;
}
