import { useEffect, useRef, useState } from "react";

import { Cards } from "./cards/Cards";
import { mountFilm, show } from "./film/canvas";
import { autoplay, startScroll } from "./film/scroll";
import { load, progress, SEQUENCES, type SeqId } from "./film/sequences";
import { Nav } from "./Nav";
import { Engine, Fault, Hero } from "./scenes/Problem";
import { FirstPrinciples, Reveal } from "./scenes/Reveal";
import { Crash, Footer, Verdict } from "./scenes/Stakes";

export function App() {
  const film = useRef<HTMLCanvasElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  const rail = useRef<HTMLDivElement>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    mountFilm(film.current!);
    startScroll();
    show({ a: { id: "S1", f: 0 } });
    // The preloader waits for the hero's coarse pass only (every 8th frame, 15 of
    // 120). The fine pass keeps streaming while the reader reads the headline.
    const tick = setInterval(
      () => bar.current?.style.setProperty("--l", String(Math.min(1, progress("S1") * 8))),
      100,
    );
    load("S1").then(() => {
      clearInterval(tick);
      setReady(true);
    });
    // ?record: every frame of every scene loaded first, so the capture never shows
    // a coarse pass, then one constant-speed pass. for me to record demo :)
    if (new URLSearchParams(location.search).has("record")) {
      document.documentElement.classList.add("recording");
      const ids = Object.keys(SEQUENCES) as SeqId[];
      ids.forEach((id) => void load(id));
      const wait = setInterval(() => {
        if (ids.every((id) => progress(id) >= 1)) {
          clearInterval(wait);
          setTimeout(() => autoplay(110), 2500);
        }
      }, 250);
    }
    const onScroll = () => {
      const max = document.documentElement.scrollHeight - innerHeight;
      rail.current?.style.setProperty("--page", String(max > 0 ? scrollY / max : 0));
    };
    addEventListener("scroll", onScroll, { passive: true });
    return () => {
      clearInterval(tick);
      removeEventListener("scroll", onScroll);
    };
  }, []);

  return (
    <>
      <div className={`preloader ${ready ? "done" : ""}`} aria-hidden={ready}>
        <span className="wordmark">DRAGONFLY</span>
        <div className="bar" ref={bar}>
          <i />
        </div>
      </div>
      <canvas id="film" ref={film} aria-hidden />
      {/* One vignette for the whole film. Per-scene scrims popped at every handover:
          the outgoing one vanished with its scene before the incoming one faded in. */}
      <div className="vignette" aria-hidden />
      <Nav />
      <div className="rail" ref={rail} aria-hidden>
        <i />
      </div>
      <main>
        <Hero />
        <Engine />
        <Fault />
        <Reveal />
        <FirstPrinciples />
        <Cards />
        <Crash />
        <Verdict />
      </main>
      <Footer />
    </>
  );
}
