/**
 * The guided tour: driver.js driven from {@link TOUR}, bridged to the router.
 *
 * Vanilla DOM on purpose, not a React overlay. Telemetry is read from the store
 * inside the render loop rather than through React (`docs/conventions.md` 3), and
 * a tour that re-rendered the tree on every step would put that invariant at
 * risk for nothing. driver.js mutates its own detached nodes and never touches
 * ours, so the uPlot rings keep filling at 20 Hz underneath the spotlight and an
 * engineer watches live data through the cutout.
 *
 * # Short tours, not one marathon
 *
 * The catalogue is one ordered list but it runs as eight: a five step `intro`
 * and one per screen. First visit opens the intro alone; the GUIDE cell and `?`
 * open the tour for whichever screen is showing. A tour past seven steps
 * completes at 16% against roughly 72% at three to five (Chameleon, over 550M
 * in-app interactions), and the twenty-three step run this started as was the
 * shape that fails. The whole sequence is still reachable, from a button on the
 * intro's first step, because a demonstration sometimes does want the full walk.
 *
 * <https://www.chameleon.io/blog/onboarding-ux-patterns>
 *
 * # Routing
 *
 * `navigate` is passed in rather than importing the router, which would close a
 * cycle: `routes.tsx` imports `Shell`, which imports `NavRail`, which starts the
 * tour. Steps declare the screen they live on; when that changes the next screen
 * is pushed before `moveNext`, and `WAIT_MS` covers the mount. A step with
 * `screen: null` targets shell chrome that exists on every screen and forces no
 * navigation, so a tour can be started from anywhere.
 *
 * The tour does not restore the screen it was started from. Someone who stops
 * halfway is looking at the screen the last step put them on, which is the one
 * they were just reading about.
 */

import { driver, type AllowedButtons, type Config, type DriveStep, type Driver } from "driver.js";

import "driver.js/dist/driver.css";

import { TOUR, type TourGroup, type TourStep } from "@/lib/tour/steps";

// After the library's own stylesheet, which opens with `all: unset` and would
// otherwise land on top of the reskin.
import "@/lib/tour/tour.css";
import type { ScreenId } from "@/store/app";

/** Where the first-visit flag lives. */
const SEEN_KEY = "dragonfly.tour.seen";

/**
 * How long a step waits for its target after a screen change.
 *
 * Generous because the screens behind the slowest steps fetch before they
 * render: ANALYSIS holds at a placeholder until `/api/signatures` lands, and
 * REPLAY until a mission is listed and read. Overshooting costs nothing, since
 * the wait ends the moment the element appears; undershooting drops the tour to
 * a centred popover with no target, which reads as a bug.
 *
 * **A long wait is the fallback, not the plan.** While one runs, the previous
 * step stays on screen and further NEXT presses are swallowed, so a step behind
 * a slow fetch looks frozen however high this is set. At 4000 SIMULATE ate six
 * presses waiting on its projection. The fix was to point that step at the
 * profile bar, which mounts with the route, and let the panel that waits on the
 * request come one step later. Any new step whose target renders after a fetch
 * wants the same treatment.
 */
const WAIT_MS = 8000;

/**
 * Whether this window is the kiosk demonstration.
 *
 * `just kiosk` runs chromium on a throwaway `--user-data-dir`, so `localStorage`
 * is empty on every boot and the first-visit gate would open the tour in front
 * of the room each time. The justfile appends `?kiosk=1` for this check.
 *
 * Read once at module load and cached: this is a single-page application, the
 * router drops the query string on the first navigation, and nothing may make
 * the answer change mid-session.
 */
const IS_KIOSK = new URLSearchParams(window.location.search).has("kiosk");

/** Live instance, so a second start replaces the first rather than stacking. */
let active: Driver | null = null;

/** The tours a step belongs to. Defaults to the screen it lives on. */
function groupsOf(step: TourStep): readonly TourGroup[] {
  if (step.groups) return step.groups;
  return step.screen ? [step.screen] : [];
}

/** Steps belonging to one tour, in catalogue order. */
function group(name: TourGroup): TourStep[] {
  return TOUR.filter((step) => groupsOf(step).includes(name));
}

/** Current screen id, from the URL rather than from React. */
function currentScreen(): string {
  return window.location.pathname.replace("/", "");
}

/**
 * Put the router on the screen a step needs.
 *
 * Returns without navigating when the screen already matches, so stepping
 * between two panels on one screen never pushes a history entry.
 */
function goTo(step: TourStep | undefined, navigate: (to: string) => void): void {
  if (step?.screen && step.screen !== currentScreen()) navigate(`/${step.screen}`);
}

/**
 * A footer button that is not one of driver.js's three.
 *
 * Built here rather than configured because driver.js takes button text
 * globally and these belong to one step. `driver-popover-footer-btn` is the
 * library's own class, which the reskin styles alongside the built-ins.
 */
function addButton(footer: HTMLElement, label: string, extra: string, onClick: () => void): void {
  const button = document.createElement("button");
  button.type = "button";
  button.className = `driver-popover-footer-btn ${extra}`;
  button.textContent = label;
  button.addEventListener("click", onClick);
  footer.prepend(button);
}

/**
 * Translate a selection of the catalogue into driver.js steps.
 *
 * `decorated` says the caller is adding its own SKIP to the first step, which
 * is then the only step that may drop CLOSE. Without it a screen tour's first
 * step offers no way out but Escape.
 */
function build(
  steps: TourStep[],
  navigate: (to: string) => void,
  get: () => Driver,
  decorated: boolean,
): DriveStep[] {
  return steps.map((step, i) => ({
    element: step.target ? `[data-tour="${step.target}"]` : undefined,
    // Only steps that follow a screen change need the wait, but a step whose
    // panel is behind a fetch needs it on a direct jump too, so every step gets
    // it. It costs nothing when the element is already mounted.
    waitForElement: WAIT_MS,
    popover: {
      title: step.title,
      description: step.body,
      side: step.side,
      align: step.align,
      // BACK goes nowhere from step zero. CLOSE stays unless a SKIP is being
      // added beside it, which would make two buttons for one action.
      showButtons: (i === 0
        ? decorated
          ? ["next"]
          : ["next", "close"]
        : ["next", "previous", "close"]) satisfies AllowedButtons[],
      onNextClick: () => {
        const obj = get();
        goTo(steps[i + 1], navigate);
        obj.moveNext();
      },
      onPrevClick: () => {
        const obj = get();
        goTo(steps[i - 1], navigate);
        obj.movePrevious();
      },
    },
  }));
}

const CONFIG: Config = {
  showProgress: true,
  progressText: "{{current}} / {{total}}",
  nextBtnText: "NEXT",
  prevBtnText: "BACK",
  doneBtnText: "DONE",
  popoverClass: "dragonfly-tour",
  // Square, tight to the panel. `--radius` is 0 across this application and a
  // 5px rounded cutout against 0px panel corners reads as a rendering fault.
  stageRadius: 0,
  stagePadding: 4,
  overlayColor: "#000000",
  overlayOpacity: 0.72,
  // The highlighted panel keeps drawing at 20 Hz but stops taking clicks. The
  // rail step spotlights the navigation, and a click there would move the
  // screen out from under the tour.
  disableActiveInteraction: true,
};

/**
 * Run a selection of the catalogue.
 *
 * `decorate` hangs extra buttons on the opening step's footer. Safe to call
 * while a tour is running; the running instance is destroyed first.
 */
function run(
  steps: TourStep[],
  navigate: (to: string) => void,
  decorate?: (footer: HTMLElement, obj: Driver) => void,
): void {
  active?.destroy();
  if (steps.length === 0) return;

  const obj = driver({
    ...CONFIG,
    steps: build(steps, navigate, () => obj, decorate !== undefined),
    onPopoverRender: (popover, { state }) => {
      if (state.activeIndex === 0) decorate?.(popover.footerButtons, obj);
    },
    onDestroyed: () => {
      markSeen();
      active = null;
    },
  });

  active = obj;
  goTo(steps[0], navigate);
  obj.drive();
}

/**
 * The introduction: what the product is, and how to move around it.
 *
 * Two extra buttons on its first step. SKIP because an engineer meeting an
 * unrequested overlay should not have to hunt for a corner icon to decline it,
 * and ALL SCREENS because the full run is worth one click to whoever wants it
 * and worth nothing to whoever does not.
 */
export function startIntro(navigate: (to: string) => void): void {
  run(group("intro"), navigate, (footer, obj) => {
    addButton(footer, "ALL SCREENS", "driver-alt-btn", () => {
      obj.destroy();
      startFullTour(navigate);
    });
    addButton(footer, "SKIP", "driver-skip-btn", () => obj.destroy());
  });
}

/** Every step, in catalogue order. The demonstration walk. */
export function startFullTour(navigate: (to: string) => void): void {
  run([...TOUR], navigate, (footer, obj) => {
    addButton(footer, "SKIP", "driver-skip-btn", () => obj.destroy());
  });
}

/**
 * The tour for whichever screen is showing. What GUIDE and `?` open.
 *
 * Falls back to the intro from a path with no tour of its own, which today is
 * only `/` before the index redirect settles.
 */
export function startScreenTour(navigate: (to: string) => void): void {
  const steps = group(currentScreen() as ScreenId);
  if (steps.length === 0) startIntro(navigate);
  else run(steps, navigate);
}

/** Record that this browser has been offered the tour. */
function markSeen(): void {
  // A browser refusing storage (private mode, a locked-down kiosk profile) must
  // not take the tour down with it. The cost of a throw here is the tour being
  // offered again next session, which is the harmless direction to fail in.
  try {
    window.localStorage.setItem(SEEN_KEY, "1");
  } catch {
    /* storage unavailable */
  }
}

/**
 * Open the intro if this browser has not seen it, and never in the kiosk.
 *
 * Marks the browser as seen when it opens rather than when it finishes, so a
 * tour abandoned at step two does not reappear on the next load.
 */
export function startTourOnFirstVisit(navigate: (to: string) => void): void {
  if (IS_KIOSK) return;
  try {
    if (window.localStorage.getItem(SEEN_KEY)) return;
  } catch {
    // Storage unreadable means the flag can never be written either, so an
    // auto-start would fire on every load. Stay quiet and leave the GUIDE cell.
    return;
  }
  markSeen();
  startIntro(navigate);
}
