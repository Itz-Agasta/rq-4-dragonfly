import { useEffect, useRef, useState } from "react";

import { Cards } from "./cards/Cards";
import { mountFilm, show } from "./film/canvas";
import { autoplay, startScroll } from "./film/scroll";
import { complete, load, SEQUENCES, type SeqId } from "./film/sequences";
import { Loader } from "./Loader";
import { Nav } from "./Nav";
import { Engine, Fault, Hero } from "./scenes/Problem";
import { FirstPrinciples, Reveal } from "./scenes/Reveal";
import { Crash, Footer, Verdict } from "./scenes/Stakes";

export function App() {
  const film = useRef<HTMLCanvasElement>(null);
  const rail = useRef<HTMLDivElement>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    mountFilm(film.current!);
    startScroll();
    show({ a: { id: "S1", f: 0 } });
    // ?record: every frame of every scene loaded first, so the capture never shows
    // a coarse pass, then one constant-speed pass. for me to record demo :)
    if (new URLSearchParams(location.search).has("record")) {
      document.documentElement.classList.add("recording");
      const ids = Object.keys(SEQUENCES) as SeqId[];
      ids.forEach((id) => void load(id));
      const wait = setInterval(() => {
        if (ids.every((id) => complete(id))) {
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
    return () => removeEventListener("scroll", onScroll);
  }, []);

  return (
    <>
      <Loader onDone={() => setReady(true)} />
      <canvas id="film" ref={film} aria-hidden />
      {/* One vignette for the whole film. Per-scene scrims popped at every handover:
          the outgoing one vanished with its scene before the incoming one faded in. */}
      <div className="vignette" aria-hidden />
      <Nav />
      <div className="rail" ref={rail} aria-hidden>
        <i />
      </div>
      <main aria-busy={!ready}>
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
