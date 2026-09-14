/**
 * The 48px navigation rail: leftmost column of the lattice, full viewport height.
 *
 * Icon-only, with the screen name on hover. 48px square cells around a 20px
 * glyph: wide enough to breathe, narrow enough that the rail reads as a margin
 * rather than a panel. It was 64 when each cell also carried a text label, and
 * that width left a bare glyph swimming in void. Quiet by default so it reads as
 * instrument chrome rather than a web app's sidebar: dim line-art on the void,
 * one weight, no fills.
 *
 * **The active cell does not take the accent.** The accent means one thing in
 * this product, which is that something needs attention; spending it on "you are
 * here" would leave a real alarm with no colour of its own. The active cell is
 * marked instead by bringing its glyph to full luminance and framing it with the
 * same corner crosshairs used elsewhere in the interface. Luminance and framing
 * carry the state; colour stays reserved.
 *
 * The one accent the rail may show is a 4px corner square meaning the screen has
 * an unacknowledged alert, which is still the alarm meaning.
 *
 * Keys 1 to 7 switch screens, because a rehearsed demo should never depend on
 * hitting a click target.
 */

import { useEffect } from "react";
import { NavLink, useLocation, useNavigate } from "react-router";

import {
  AboutGlyph,
  GuideGlyph,
  Mark,
  SCREEN_GLYPHS,
  SettingsGlyph,
} from "@/components/app/glyphs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { startScreenTour } from "@/lib/tour";
import { type ScreenId, SCREENS, screenHasUnacknowledged, useApp } from "@/store/app";

const LABELS: Record<ScreenId, string> = {
  ops: "OPS",
  twin: "TWIN",
  analysis: "ANALYSIS",
  simulate: "SIMULATE",
  replay: "REPLAY",
  fleet: "FLEET",
  evidence: "EVIDENCE",
};

/** Where ABOUT goes. */
const SOURCE = "https://github.com/Itz-Agasta/rq-4-dragonfly";

const CELL =
  "relative flex h-12 shrink-0 items-center justify-center border-b border-border outline-none " +
  "transition-colors duration-150 focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-inset";

/** `+` crosshairs at the cell's corners, marking the active screen. */
function Marks() {
  const arm = "absolute bg-structure-hi";
  return (
    <span aria-hidden="true">
      <span className={`${arm} top-[5px] left-[5px] h-px w-[5px]`} />
      <span className={`${arm} top-[3px] left-[7px] h-[5px] w-px`} />
      <span className={`${arm} top-[5px] right-[5px] h-px w-[5px]`} />
      <span className={`${arm} top-[3px] right-[7px] h-[5px] w-px`} />
      <span className={`${arm} bottom-[5px] left-[5px] h-px w-[5px]`} />
      <span className={`${arm} bottom-[3px] left-[7px] h-[5px] w-px`} />
      <span className={`${arm} right-[5px] bottom-[5px] h-px w-[5px]`} />
      <span className={`${arm} right-[7px] bottom-[3px] h-[5px] w-px`} />
    </span>
  );
}

function Cell({ id, index }: { id: ScreenId; index: number }) {
  const Glyph = SCREEN_GLYPHS[id];
  const alerted = useApp(screenHasUnacknowledged(id));
  // Active state is computed here rather than taken from NavLink's render props.
  // The tooltip trigger clones its child through a slot, which does not carry a
  // function `className` or a function child across, so the cell silently lost
  // its height and collapsed to the glyph.
  const active = useLocation().pathname === `/${id}`;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <NavLink
          to={`/${id}`}
          className={`${CELL} ${
            active
              ? "bg-popover text-foreground"
              : "text-structure hover:bg-popover hover:text-muted-foreground"
          }`}
          aria-label={LABELS[id]}
        >
          {active ? <Marks /> : null}
          {alerted ? (
            <span
              className="bg-primary absolute top-[6px] right-[6px] size-1"
              aria-label="unacknowledged alert"
            />
          ) : null}
          <Glyph />
        </NavLink>
      </TooltipTrigger>
      <TooltipContent side="right" sideOffset={6}>
        {LABELS[id]}
        <span className="text-foreground-dim ml-2">{index}</span>
      </TooltipContent>
    </Tooltip>
  );
}

/**
 * A cell with no screen behind it.
 *
 * `aria-disabled` rather than `disabled`: a disabled button receives no pointer
 * events, so its tooltip can never open, and the tooltip is the only thing
 * naming an icon-only cell. Present at all because the rail reads as unfinished
 * without them and an operator expects them.
 */
function UtilityCell({
  label,
  Glyph,
}: {
  label: string;
  Glyph: (props: { className?: string }) => React.ReactElement;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-disabled
          className={`${CELL} text-structure/70 cursor-default`}
          aria-label={label}
        >
          <Glyph />
        </button>
      </TooltipTrigger>
      <TooltipContent side="right" sideOffset={6}>
        {label}
        <span className="text-foreground-dim ml-2">not built</span>
      </TooltipContent>
    </Tooltip>
  );
}

/**
 * GUIDE runs the walkthrough for the screen that is showing.
 *
 * Per screen rather than the whole product: three to five steps complete at
 * roughly 72% where a twenty-three step run completes at 16%, and someone
 * pressing this on ANALYSIS wants ANALYSIS, not to be taken back to OPS. The
 * full walk is one button away on the first-visit intro.
 *
 * A real cell rather than a disabled one, and above ABOUT so the two ways of
 * asking "what is this" sit together: this one answers from inside the
 * application and ABOUT answers by leaving it.
 */
function GuideCell({ onStart }: { onStart: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onStart}
          data-tour="rail-guide"
          className={`${CELL} text-structure hover:bg-popover hover:text-muted-foreground`}
          aria-label="GUIDE, starts the walkthrough"
        >
          <GuideGlyph />
        </button>
      </TooltipTrigger>
      <TooltipContent side="right" sideOffset={6}>
        GUIDE
        <span className="text-foreground-dim ml-2">this screen · ?</span>
      </TooltipContent>
    </Tooltip>
  );
}

/**
 * ABOUT opens the source.
 *
 * A new tab, not a panel: nothing written here beats the repository at
 * answering "what is this", and a second copy is a second thing to keep in step.
 *
 * **It takes the demonstration out of the kiosk.** `just kiosk` is chromium
 * `--app` fullscreen and the tab lands on top of it. Hence the bottom of the
 * rail, where a mis-click is unlikely.
 */
function SourceCell() {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <a
          href={SOURCE}
          target="_blank"
          rel="noreferrer"
          className={`${CELL} text-structure hover:bg-popover hover:text-muted-foreground`}
          aria-label="ABOUT, opens the source repository"
        >
          <AboutGlyph />
        </a>
      </TooltipTrigger>
      <TooltipContent side="right" sideOffset={6}>
        ABOUT
        <span className="text-foreground-dim ml-2">source</span>
      </TooltipContent>
    </Tooltip>
  );
}

export function NavRail() {
  const navigate = useNavigate();

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      const target = event.target as HTMLElement | null;
      if (target?.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target?.tagName ?? "")) {
        return;
      }
      // `?` is shift-/ on most layouts, so the modifier guard above cannot be
      // extended to shift without losing it.
      if (event.key === "?") {
        startScreenTour((to) => void navigate(to));
        return;
      }
      const index = Number.parseInt(event.key, 10);
      if (index >= 1 && index <= SCREENS.length) {
        void navigate(`/${SCREENS[index - 1]}`);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate]);

  return (
    <nav
      data-tour="rail"
      className="border-border flex w-12 shrink-0 flex-col border-r"
      aria-label="Screens"
    >
      <div className="border-border text-foreground flex h-10 items-center justify-center border-b">
        <Mark size={22} />
        <span className="sr-only">RQ-4 DRAGONFLY</span>
      </div>

      {SCREENS.map((id, i) => (
        <Cell key={id} id={id} index={i + 1} />
      ))}

      <div className="min-h-6 flex-1" />

      <UtilityCell label="SETTINGS" Glyph={SettingsGlyph} />
      <GuideCell onStart={() => startScreenTour((to) => void navigate(to))} />
      <SourceCell />
    </nav>
  );
}
