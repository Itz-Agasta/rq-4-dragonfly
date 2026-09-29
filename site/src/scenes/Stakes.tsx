import coking from "../data/coking.json";
import { linear } from "../film/scroll";
import { NoiseBg } from "./NoiseBg";
import { Scene } from "./Scene";

const REPO = "https://github.com/Itz-Agasta/rq-4-dragonfly";
const OPS = "https://dragonfly.vyse.site/ops";
const lead = (coking.alarms.cusum_s! - coking.onset_s).toFixed(1);

// Limit status, not values. That no certified limit trips on this fault is true
// (README, TCDS E.200, and coking.json's redline flag); invented numbers would not be.
const CHANNELS = ["CHT 1", "CHT 2", "CHT 3", "CHT 4", "EGT 3", "OIL P", "OIL T", "COOLANT"];

/** Without it: the nose-camera dive, and the page goes black before impact. */
export function Crash() {
  return (
    <Scene vh={420} map={linear("S4")} preload={["S4"]}>
      <div className="center" data-at="0.03,0.28">
        <h2 className="h1 xl" data-words>
          Without it
        </h2>
      </div>
      <div className="gauges" data-at="0.2,0.86">
        {CHANNELS.map((c) => (
          <div key={c} className="gauge">
            <span className="label">{c}</span>
            <span className="gauge-state">Within limits</span>
          </div>
        ))}
      </div>
      <div className="blackout" />
      <div className="center" data-at="0.9,1.01">
        <h2 className="h2" data-words>
          Threshold monitoring finds this fault
          <br />
          in the wreckage
        </h2>
      </div>
    </Scene>
  );
}

/**
 * With it: the numbers from the same recording the cards draw, over signal noise,
 * because what the reader just watched was a link going dead.
 */
export function Verdict() {
  return (
    <Scene vh={260} map={null} opaque stage="black">
      <NoiseBg />
      <div className="lower" data-at="0.1,0.5">
        <h2 className="h1 xl" data-words>
          DRAGONFLY caught it
          <br />
          <span className="accent">{lead} seconds after onset</span>
        </h2>
      </div>
      <div className="center" data-at="0.55,1.01" style={{ paddingBottom: "14vh" }}>
        <h2 className="h1" data-words>
          Every gauge was still green
        </h2>
        <p className="label">Measurement minus physics</p>
        <div className="cta">
          <a className="btn primary" href={OPS}>
            Open the dashboard
          </a>
          <a className="btn" href={`${REPO}#readme`}>
            Read the model
          </a>
        </div>
      </div>
    </Scene>
  );
}

export function Footer() {
  return (
    <footer className="footer">
      <div className="footer-row">
        <div className="footer-brand">
          <svg
            viewBox="0 0 20 20"
            width="18"
            height="18"
            aria-hidden
            fill="none"
            stroke="#ff6b35"
            strokeWidth="1.5"
          >
            <path d="M10 2.2V17.9" />
            <path d="M3 4.9 10 7.1 17 4.9" />
            <path d="M2.2 13.1 10 10.3 17.8 13.1" />
          </svg>
          <b>DRAGONFLY</b>
        </div>
        <nav className="footer-links">
          <a href={OPS}>Dashboard</a>
          <a href={`${REPO}#readme`}>Read the model</a>
          <a href={REPO}>Source</a>
        </nav>
      </div>
      <p className="label disclosure">
        Every number on this page comes from the model and its recordings.
      </p>
    </footer>
  );
}
