import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { SplitText } from "gsap/SplitText";
import Lenis from "lenis";

import { show, type Frame } from "./canvas";
import { load, SEQUENCES, type SeqId } from "./sequences";

gsap.registerPlugin(ScrollTrigger, SplitText);

let lenis: Lenis | null = null;

/** Lenis driven by GSAP's ticker, the integration the Lenis README documents. Call once. */
export function startScroll() {
  // A phone's address bar showing and hiding fires resize; refreshing every trigger
  // on it is visible jank mid-scroll.
  ScrollTrigger.config({ ignoreMobileResize: true });
  // Geist Mono arriving late reflows every line of copy; positions must follow it.
  document.fonts?.ready.then(() => ScrollTrigger.refresh());
  // Reduced motion: native scrolling, no inertia. The film still follows the
  // scrollbar, because it is the content, not decoration.
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  lenis = new Lenis({ autoRaf: false, anchors: true });
  lenis.on("scroll", ScrollTrigger.update);
  gsap.ticker.add((t) => lenis?.raf(t * 1000));
  gsap.ticker.lagSmoothing(0);
  // Dev contract: ?jump=<px> lands pre-scrolled so a screenshot harness can capture
  // any beat without scripting a scroll.
  const jump = Number(new URLSearchParams(location.search).get("jump"));
  // A film starts at frame 0. Restoring a mid-page offset would open on whatever
  // scene the reader left, before its frames are loaded.
  history.scrollRestoration = "manual";
  requestAnimationFrame(() => lenis?.scrollTo(jump || 0, { immediate: true }));
}

/**
 * Holds the page still while the loader is up. Without Lenis (reduced motion) the
 * document itself stops scrolling.
 */
export function holdScroll(hold: boolean) {
  if (lenis && hold) lenis.stop();
  else if (lenis) lenis.start();
  document.documentElement.style.overflow = hold ? "hidden" : "";
}

/**
 * Recording mode (`?record`): one constant-speed pass from top to bottom, for a
 * screen capture. Linear on purpose: an eased scroll lingers at the ends and rushes
 * the middle, which is where the story is.
 */
export function autoplay(seconds: number) {
  const max = document.documentElement.scrollHeight - innerHeight;
  if (lenis) lenis.scrollTo(max, { duration: seconds, easing: (t) => t, lock: true, force: true });
  else {
    const t0 = performance.now();
    const step = (t: number) => {
      const u = Math.min(1, (t - t0) / (seconds * 1000));
      scrollTo(0, u * max);
      if (u < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }
}

/** Maps section progress 0..1 to what the film shows. Null leaves the canvas alone. */
export type FrameMap = (p: number) => Frame | null;

/** Fractional frame position `u` (0..1) through a sequence. */
export const at = (id: SeqId, u: number) => ({
  id,
  f: Math.max(0, Math.min(1, u)) * (SEQUENCES[id] - 1),
});

/** Linear map over one sequence. */
export const linear =
  (id: SeqId): FrameMap =>
  (p) => ({ a: at(id, p) });

const clamp01 = (x: number) => Math.max(0, Math.min(1, x));

type Beat = { el: HTMLElement; a: number; b: number; words: HTMLElement[] };

// Scroll over which one scene hands over to the next, in viewport heights. Long
// enough to read as a dissolve on a trackpad flick, short enough that two layers of
// copy never sit on screen together for long.
const HANDOVER_VH = 0.45;

/**
 * Binds a section to the film.
 *
 * Every scene's stage is a fixed full-viewport layer; the section is only a scroll
 * spacer. Nothing ever slides: a scene's layer fades in over the last HANDOVER_VH
 * of the scene before it, on top of it, which is a true crossfade on one continuous
 * stage (the sticky-per-section alternative drags each layer, scrim and all, up the
 * screen like a slide change). An opaque scene (`data-opaque`) followed by a film
 * scene fades out whole, so the footage dissolves back in instead of cutting.
 *
 * Inside its own span the section's progress `p` (0..1) picks the frame and drives
 * every `[data-at="a,b"]` beat. A beat carrying `data-words` reveals word by word,
 * each word flashing orange and settling to its own colour.
 */
export function scrub(
  section: HTMLElement,
  map: FrameMap | null,
  preload: SeqId[] = [],
  onP?: (p: number) => void,
): () => void {
  const splits: SplitText[] = [];
  const beats: Beat[] = [...section.querySelectorAll<HTMLElement>("[data-at]")].map((el) => {
    const [a, b] = el.dataset.at!.split(",").map(Number);
    const words: HTMLElement[] = [];
    for (const h of el.matches("[data-words]")
      ? [el]
      : [...el.querySelectorAll<HTMLElement>("[data-words]")]) {
      const s = new SplitText(h, { type: "words", wordsClass: "w" });
      splits.push(s);
      words.push(...(s.words as HTMLElement[]));
    }
    return { el, a, b, words };
  });
  const stage = section.querySelector<HTMLElement>(".stage")!;
  const inner = stage.querySelector<HTMLElement>(".stage-inner") ?? stage;
  const prev = section.previousElementSibling as HTMLElement | null;
  const next = section.nextElementSibling as HTMLElement | null;
  const opaque = (el: HTMLElement | null) => el?.dataset.opaque === "1";
  const isScene = (el: HTMLElement | null) => !!el?.classList.contains("scene");
  const F = () => (isScene(prev) ? innerHeight * HANDOVER_VH : 0);

  const beatsAt = (p: number) => {
    for (const { el, a, b, words } of beats) {
      // Fade over 3% of the section at each edge: long enough to read as a
      // dissolve, short enough that two beats never sit on screen together.
      // A beat ending at or past 1 runs to the end of its scene with no fade-out;
      // the scene's own handover takes it away.
      const e = 0.03;
      const o = clamp01(Math.min((p - a) / e, b >= 1 ? 1 : (b - p) / e));
      el.style.opacity = String(o);
      el.style.visibility = o > 0 ? "visible" : "hidden";
      if (!words.length) {
        el.style.transform = `translateY(${(1 - o) * 12}px)`;
        continue;
      }
      // Words arrive over the first third of the beat (capped at 12% of the
      // section) so the line is complete long before it leaves.
      const span = Math.min(0.12, (b - a) / 3);
      const u = (p - a) / span;
      const n = words.length;
      words.forEach((w, i) => {
        const k = u * n - i; // 0 when this word starts, 1 when it is fully in
        w.style.opacity = String(clamp01(k));
        // Orange while arriving, then the colour the markup gave it (white, or
        // orange for an .accent span). An empty string hands colour back to CSS.
        w.style.color = k > 0 && k < 2.2 ? "var(--primary)" : "";
      });
    }
  };

  const hide = () => {
    stage.style.visibility = "hidden";
    stage.style.opacity = "0";
  };
  const update = (st: ScrollTrigger) => {
    // A jump (an anchor link, a restored position) can carry the scroll clean over
    // a scene; ScrollTrigger still reports its progress change but never activates
    // it, so without this the skipped stage stays up, and an opaque one covers the
    // film. Only the last scene stays up past its end, behind the footer.
    if (!st.isActive && !(!isScene(next) && st.progress === 1)) {
      hide();
      return;
    }
    const f = F();
    const y = st.scroll();
    const top = st.start + f; // where this scene's own span begins
    const p = clamp01((y - top) / Math.max(1, st.end - top));
    const fadeIn = f ? clamp01((y - st.start) / f) : 1;
    // The next scene fades in over our last handover; decide what we do meanwhile.
    const tail = next && isScene(next) ? clamp01((st.end - y) / (innerHeight * HANDOVER_VH)) : 1;
    const wholeOut = opaque(section) && !opaque(next);
    stage.style.visibility = "visible";
    stage.style.opacity = String(fadeIn * (wholeOut ? tail : 1));
    inner.style.opacity = String(tail);
    stage.style.pointerEvents = fadeIn * tail > 0.5 ? "auto" : "none";
    section.style.setProperty("--p", p.toFixed(4));
    // During the fade-in the previous scene still owns the film, unless it has no
    // film (an opaque scene), in which case we start ours under the dissolve.
    if (map && (y >= top || opaque(prev))) {
      const fr = map(p);
      if (fr) show(fr);
    }
    beatsAt(p);
    onP?.(p);
  };

  const near = ScrollTrigger.create({
    trigger: section,
    start: "top bottom+=100%",
    end: "bottom top",
    onEnter: () => preload.forEach(load),
    onEnterBack: () => preload.forEach(load),
  });
  const span = ScrollTrigger.create({
    trigger: section,
    start: () => `top ${F()}px`,
    // The last scene ends when its bottom reaches the viewport's, not its top: the
    // page stops scrolling there, and an end it can never reach leaves the last
    // beats half revealed.
    end: isScene(next) ? "bottom top" : "bottom bottom",
    invalidateOnRefresh: true,
    onUpdate: update,
    // Past its end the last scene stays up behind the footer strip; every other
    // scene hands over and hides.
    onToggle: (st) => (st.isActive || (!isScene(next) && st.progress === 1) ? update(st) : hide()),
  });
  hide();
  beatsAt(0);
  // The first scene is on screen before anyone scrolls.
  if (!isScene(prev)) update(span);
  return () => {
    near.kill();
    span.kill();
    splits.forEach((s) => s.revert());
  };
}
