import { useEffect, useRef, useState } from "react";

import { holdScroll } from "./film/scroll";
import { load, progress } from "./film/sequences";

// The boot screen. The log is theatre: a pre-flight sequence that scrolls on its own
// clock, in the grammar of promo.emotion-agency.com. Every line still names something
// the system really does, so it reads as the ground station powering up rather than
// sci-fi filler. The percentage and the exit are NOT theatre: they follow the hero's
// frames, because a number that lies is the one thing a slow connection exposes.

// Past this, open on the hero's coarse pass and keep streaming. Measured: ~7.8 s on
// throttled 4G for the light tier, well under this.
const GIVE_UP_MS = 8000;
// A loader that appears and vanishes inside this reads as a glitch, so a cached
// visit never shows it (Nielsen's ~1 s threshold, less a margin).
const SHOW_AFTER_MS = 800;
// Once it has spoken, let the sequence play; cutting it at 0.3 s reads as a bug.
const MIN_SHOWN_MS = 1800;
// Log cadence. Loops until the load finishes, like the reference.
const LINE_MS = 420;

const SEQUENCE = [
  "POWERING GROUND STATION",
  "OPENING DRONECAN BUS · 20 HZ",
  "LOADING ENGINE MODEL · AUSTRO E4P",
  "SPOOLING TWIN · RK4 AT 200 HZ",
  "SEEDING 28-STATE FILTER",
  "ARMING CUSUM · MAHALANOBIS",
  "READING 22 CHANNELS",
  "SYNCING TWIN TO ENGINE",
];
const FINAL = "LINK ESTABLISHED";
const LINE = "The twin is spinning up. Hang tight.";
const LINE_DONE = "Engine and twin in lockstep.";
const GLYPHS = "▓▒░<>/\\|#%&$@*+=";
const VISIBLE = 6;

/** Scrambled type-on: each character settles after a few frames of noise. */
function useScramble(text: string, run: boolean) {
  const [out, setOut] = useState("");
  // Reduced motion gets the line whole, with no scramble.
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
  useEffect(() => {
    if (!run || still) return;
    let i = 0;
    const id = setInterval(() => {
      i += 0.5;
      const settled = text.slice(0, Math.floor(i));
      const noise = Array.from({ length: Math.min(4, text.length - settled.length) }, () =>
        text[settled.length] === " " ? " " : GLYPHS[(Math.random() * GLYPHS.length) | 0],
      ).join("");
      setOut(settled + noise);
      if (settled.length >= text.length) clearInterval(id);
    }, 28);
    return () => clearInterval(id);
  }, [text, run, still]);
  return still ? (run ? text : "") : out;
}

export function Loader({ onDone }: { onDone: () => void }) {
  const [shown, setShown] = useState(false);
  const [leaving, setLeaving] = useState(false);
  const [linked, setLinked] = useState(false);
  const [gone, setGone] = useState(false);
  const [pct, setPct] = useState(0);
  const [step, setStep] = useState(0);
  const ready = useRef(false);
  const typed = useScramble(linked ? LINE_DONE : LINE, shown);

  useEffect(() => {
    holdScroll(true);
    const t0 = performance.now();
    let shownAt = 0;
    let fonts = false;
    document.fonts?.ready.then(() => (fonts = true));
    // The hero gets the connection to itself; the engine bay starts once the hero's
    // light tier is in and streams while the reader reads the headline. Gating the
    // exit on it measured +2.5 s on 4G for a scene three scrolls away.
    void load("S1");
    let bayStarted = false;
    const showTimer = setTimeout(() => {
      if (ready.current) return;
      shownAt = performance.now();
      setShown(true);
    }, SHOW_AFTER_MS);
    const lines = setInterval(() => setStep((s) => s + 1), LINE_MS);

    const exit = (fast: boolean) => {
      clearInterval(lines);
      setLinked(true);
      setLeaving(true);
      // Hold LINK ESTABLISHED long enough to land, then dissolve into the hero.
      setTimeout(
        () => {
          holdScroll(false);
          onDone();
          setTimeout(() => setGone(true), 900);
        },
        fast ? 0 : 900,
      );
    };

    const tick = setInterval(() => {
      const hero = progress("S1");
      const now = performance.now();
      const late = now - t0 > GIVE_UP_MS;
      const heroDone = hero >= 1;
      if ((heroDone || late) && !bayStarted) {
        bayStarted = true;
        void load("S2");
      }
      setPct(Math.min(100, Math.round(5 * +fonts + 95 * hero)));
      const loaded = fonts && (heroDone || late);
      const playedEnough = !shownAt || now - shownAt >= MIN_SHOWN_MS;
      if (!ready.current && loaded && playedEnough) {
        ready.current = true;
        clearInterval(tick);
        clearTimeout(showTimer);
        exit(!shownAt);
      }
    }, 100);
    return () => {
      clearInterval(tick);
      clearInterval(lines);
      clearTimeout(showTimer);
    };
    // onDone is App's stable setter; the loader runs exactly once.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (gone) return null;
  // A rolling window over the looping sequence; the newest line is highlighted.
  const log = Array.from({ length: Math.min(step + 1, VISIBLE) }, (_, k) => {
    const n = step - (Math.min(step + 1, VISIBLE) - 1) + k;
    return { n, text: SEQUENCE[n % SEQUENCE.length] };
  });
  if (linked) log.push({ n: step + 1, text: FINAL });
  const last = log.length - 1;
  return (
    <div
      className={`boot ${shown ? "is-shown" : ""} ${leaving ? "is-leaving" : ""}`}
      aria-hidden={!shown}
    >
      <div role="status" aria-live="polite" className="sr-only">
        {linked ? "Loaded" : `Loading, ${pct} percent`}
      </div>
      <ol className="boot-log" aria-hidden>
        {log.map((l, i) => (
          <li
            key={l.n}
            className={i === last ? "is-current" : "is-done"}
            style={{ opacity: i === last ? 1 : 0.35 + (0.55 * (i + 1)) / log.length }}
          >
            <span>
              // {l.text}
              {l.text === FINAL ? "" : "..."}
            </span>
          </li>
        ))}
      </ol>
      <div className="boot-status num">
        {linked ? "( LINKED )" : `( ${String(pct).padStart(2, "0")}% )`}
      </div>
      <p className="boot-line" aria-hidden>
        // {typed}
      </p>
    </div>
  );
}
