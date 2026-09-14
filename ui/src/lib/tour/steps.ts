/**
 * The guided tour's content: what each panel is, in one fact.
 *
 * Written for a propulsion engineer meeting this station for the first time,
 * not for a reviewer watching a demonstration. A panel earns a step only when
 * reading it wrong is plausible: innovation mistaken for residual, a subsystem
 * score that is a limit check rather than an estimate, a remaining life whose
 * hours come from a compressed ramp. Panels that explain themselves are
 * skipped.
 *
 * # One fact per step, 25 words, and no second sentence of context
 *
 * Tour tooltips are scanned, not read. Chameleon's benchmark over 550M
 * in-app interactions puts the working limits at ~25 words and ~180 characters
 * a step; past that, dismissal rises rather than comprehension. The first draft
 * of this file averaged 55 words and ran to 90, which is a help article wearing
 * a popover. Every step now carries the single thing an operator cannot infer
 * from the panel itself, and the rationale behind it lives in `design.md`,
 * `handover.md` and the EVIDENCE screen where a reader can take their time.
 *
 * `node ui/scripts/tour-lint.mjs` fails `just check` on a step over either limit.
 * <https://www.chameleon.io/blog/onboarding-ux-patterns>
 *
 * # Every figure here is verified against the code, not recalled
 *
 * `PARAMS = 10` and `HYPOTHESES = 9` in `twin-core`, 22 channels and 7
 * subsystems, `HISTORY_SECONDS = 90` at 20 Hz, and the -0.69 sigma torque bias
 * from `SyncQuality`. A wrong number here is worse than no tour, because it is
 * the one surface a new user has no way to check. When a count changes in
 * `twin-core`, it changes here.
 */

import type { ScreenId } from "@/store/app";

/** A tour: the introduction, or one screen's own. */
export type TourGroup = "intro" | ScreenId;

/** One stop on the tour. */
export interface TourStep {
  /**
   * Screen this step lives on, or null for a centred step belonging to no
   * screen. The tour navigates when this changes from the previous step.
   */
  screen: ScreenId | null;
  /**
   * Which tours this step belongs to, when that is not just its `screen`.
   *
   * A list because membership is not exclusive. The shell steps live on OPS
   * because the mission bar is only guaranteed populated there, but they
   * introduce the product rather than that screen, so they carry `["intro"]`
   * alone and OPS's own tour starts at the flight conditions. Fault injection
   * is in both: a new operator has to be told the control exists, and someone
   * pressing `?` on OPS is standing in front of the button.
   */
  groups?: readonly TourGroup[];
  /** Value of the target's `data-tour` attribute. Absent means centred. */
  target?: string;
  /** Eight words at most. The panel's name, not a sentence about it. */
  title: string;
  /**
   * Popover body, 25 words and 180 characters at most.
   *
   * driver.js assigns this to `innerHTML`, so markup works and is used for the
   * one legend in the catalogue. Safe because every string here is authored in
   * this file: nothing user-supplied or fetched ever reaches it, and nothing
   * may start.
   */
  body: string;
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
}

export const TOUR: readonly TourStep[] = [
  {
    screen: null,
    groups: ["intro"],
    title: "RQ-4 DRAGONFLY",
    body:
      "A physics model runs beside the real engine. The station watches the gap between them, " +
      "not the sensors. That gap is the residual.",
  },

  {
    screen: null,
    groups: ["intro"],
    target: "rail",
    side: "right",
    align: "start",
    title: "The rail",
    body:
      "Keys 1 to 7 switch screens. An orange square on a cell means an unacknowledged alert." +
      '<ul class="tour-legend">' +
      "<li><b>OPS</b><span>live engine, alerts, strips</span></li>" +
      "<li><b>TWIN</b><span>measured against physics</span></li>" +
      "<li><b>ANALYSIS</b><span>what is wrong, how long left</span></li>" +
      "<li><b>SIMULATE</b><span>fly the rest of the mission</span></li>" +
      "<li><b>REPLAY</b><span>a recorded mission, scrubbed</span></li>" +
      "<li><b>FLEET</b><span>one row per airframe</span></li>" +
      "<li><b>EVIDENCE</b><span>what was validated, against what</span></li>" +
      "</ul>",
  },
  {
    screen: "ops",
    target: "topbar-inputs",
    side: "bottom",
    align: "end",
    title: "Flight conditions",
    body:
      "The three inputs the model takes from outside itself. If these go stale, so do the " +
      "twin's predictions.",
  },
  {
    screen: "ops",
    groups: ["intro"],
    target: "topbar-twin",
    side: "bottom",
    align: "end",
    title: "Twin lock and innovation",
    body:
      "Innovation is how far the model missed the last measurement. It stays small as the " +
      "engine degrades: it tracks the twin, not the engine's health.",
  },

  {
    screen: "ops",
    target: "ops-health",
    side: "right",
    align: "start",
    title: "Subsystem health",
    body:
      "Seven subsystems, 0 to 100. Five come from the estimator. Electrical and Mechanical are " +
      "plain limit checks, and the row says so.",
  },
  {
    screen: "ops",
    target: "ops-schematic",
    side: "left",
    align: "center",
    title: "Engine section",
    body:
      "Live values at the place each is measured. A dashed hairline and a diamond INFERRED tag " +
      "mark a value the estimator solved for.",
  },
  {
    screen: "ops",
    groups: ["intro", "ops"],
    target: "ops-inject",
    side: "bottom",
    align: "end",
    title: "Break the engine on purpose",
    body:
      "Press F or INJECT FAULT, pick a preset, confirm. It commands the simulator, never an " +
      "engine. Stay on cylinder 3: only those faults get named.",
  },
  {
    screen: "ops",
    target: "ops-alerts",
    side: "left",
    align: "start",
    title: "Alerts and advisory",
    body:
      "Raised by residual rules, not limit crossings, so an alert can arrive before any " +
      "exceedance. The advisory below carries the action.",
  },
  {
    screen: "ops",
    target: "ops-strips",
    side: "top",
    align: "center",
    title: "Live strips",
    body:
      "Ninety seconds at 20 Hz. Solid white is measured, dashed grey is the twin. The gap " +
      "between them is the residual.",
  },

  {
    screen: "twin",
    target: "twin-pair",
    side: "right",
    align: "center",
    title: "Measured against physics",
    body:
      "Residual beneath in sigma. The shaded band is \u00b13\u03c3: it fills the strip on a quiet " +
      "channel and shrinks as the residual grows.",
  },
  {
    screen: "twin",
    target: "twin-rail",
    side: "right",
    align: "start",
    title: "Channel rail",
    body:
      "All 22 channels, loudest disagreement first. Ranked on a five second mean, so the order " +
      "settles instead of flickering.",
  },
  {
    screen: "twin",
    target: "twin-params",
    side: "left",
    align: "start",
    title: "What the estimator solved for",
    body:
      "Ten parameters the filter carries, none of them measured. Each sparkline runs against " +
      "its own excursion, so a slow decline is visible.",
  },
  {
    screen: "twin",
    target: "twin-sync",
    side: "left",
    align: "end",
    title: "Twin sync quality",
    body:
      "A channel's mean residual is the twin's standing bias, and it is not zero: torque sits " +
      "at -0.69 sigma. Detectors baseline against it first.",
  },

  {
    screen: "analysis",
    target: "analysis-detection",
    side: "bottom",
    align: "center",
    title: "Detection, against the redline",
    body:
      "The redline never trips on a coked injector, because that cylinder runs cool and every " +
      "limit that could see it is an upper bound.",
  },
  {
    screen: "analysis",
    target: "analysis-matrix",
    side: "top",
    align: "center",
    title: "Signature matrix",
    body:
      "Nine hypotheses by 22 channels, each row generated by perturbing the model. The " +
      "outlined row is the residual now. Find the row it matches.",
  },
  {
    screen: "analysis",
    target: "analysis-causes",
    side: "right",
    align: "start",
    title: "Ranked causes",
    body:
      "The winner with its posterior, then the alternatives, each labelled with the channel " +
      "that rejected it. Computed per frame, not templated.",
  },
  {
    screen: "analysis",
    target: "analysis-prognosis",
    side: "left",
    align: "start",
    title: "Remaining useful life",
    body:
      "Read the rate before the hours. On a seeded fault the ramp is compressed, so the hours " +
      "extrapolate a rate no injector cokes at.",
  },

  {
    screen: "simulate",
    target: "simulate-profile",
    side: "bottom",
    align: "start",
    title: "SIMULATE: fly the mission out",
    body:
      "Five profiles, one press each. The model runs at 500 times realtime, so a two hour leg " +
      "settles in seconds.",
  },
  {
    screen: "simulate",
    target: "simulate-outlook",
    side: "left",
    align: "start",
    title: "Seeded from the live estimate",
    body:
      "Seeded from the estimate held right now, so this projects the degraded engine, not a " +
      "healthy one flying the same profile.",
  },

  {
    screen: "replay",
    target: "replay-timeline",
    side: "top",
    align: "center",
    title: "REPLAY: a recorded mission",
    body:
      "Scrubbed on mission time. No remaining life and no isolation: a recorded frame does not " +
      "carry the span either one needs.",
  },

  {
    screen: "fleet",
    target: "fleet-roster",
    side: "right",
    align: "start",
    title: "FLEET: one row per airframe",
    body:
      "The same twin against a roster. This roster is static, and the screen says so rather " +
      "than letting plausible tails read as live.",
  },

  {
    screen: "evidence",
    target: "evidence-index",
    side: "right",
    align: "start",
    title: "EVIDENCE: the validation record",
    body:
      "Every claim this station makes, the reference it was checked against, and the error. " +
      "Start here before trusting a number elsewhere.",
  },

  {
    screen: null,
    groups: ["intro"],
    target: "rail-guide",
    side: "right",
    align: "end",
    title: "End of tour",
    body: "GUIDE or ? opens the tour for whichever screen you are on.",
  },
];
