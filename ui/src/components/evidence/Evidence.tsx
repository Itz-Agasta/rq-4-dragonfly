/**
 * EVIDENCE: the screen a reviewer reads rather than watches.
 *
 * Every other screen answers "what is the engine doing". This one answers "why
 * should I believe any of it", which is a different question with a different
 * shape: it is a document, it is read at arm's length rather than across a room,
 * and it is the one screen where a scroll is the right interaction.
 *
 * **The pane scrolls, the page never does.** `min-h-0` on the flex child and
 * `overflow-y-auto` on the pane; the no-page-scroll rule holds at every viewport
 * this ships to.
 *
 * **Nothing reserves horizontal space.** A 1180px document cap and then a 72
 * character prose measure were both tried and both removed: on a wall display the
 * empty third reads as a broken page, and this screen is read across a room as
 * often as at a desk.
 *
 * The index column is a scroll spy rather than a router: five sections of one
 * document, so splitting them across five routes would put a page load between
 * two halves of an argument. The active entry is marked by luminance, never by
 * the accent, which is reserved for alarms everywhere in this application.
 *
 * **This screen is scheduled for deletion.** When the landing page exists it
 * becomes its /docs section, which is why the content lives in `data/` and the
 * sections take no props: moving it is a copy of one folder, not a rewrite.
 */

import { useEffect, useRef, useState } from "react";

import { Engine } from "@/components/evidence/Engine";
import { Equation } from "@/components/evidence/Equation";
import { Literature } from "@/components/evidence/Literature";
import { Parameters } from "@/components/evidence/Parameters";
import { Provenance } from "@/components/evidence/Provenance";
import { Validation } from "@/components/evidence/Validation";

const SECTIONS: [string, string, string][] = [
  ["engine", "01", "THE ENGINE"],
  ["parameters", "02", "PARAMETERS"],
  ["validation", "03", "VALIDATION"],
  ["provenance", "04", "PROVENANCE"],
  ["literature", "05", "LITERATURE"],
];

/**
 * Which section is being read.
 *
 * Topmost intersecting section wins, rather than the largest: with a long
 * section under a short one, "largest visible" leaves the index pointing at a
 * heading that has already scrolled off, which reads as a broken index.
 */
function useActiveSection(root: React.RefObject<HTMLDivElement | null>) {
  const [active, setActive] = useState(SECTIONS[0][0]);

  useEffect(() => {
    const pane = root.current;
    if (!pane) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((e) => e.isIntersecting)
          .toSorted((a, b) => a.boundingClientRect.top - b.boundingClientRect.top);
        if (visible[0]) setActive(visible[0].target.id);
      },
      { root: pane, rootMargin: "0px 0px -70% 0px", threshold: 0 },
    );
    for (const [id] of SECTIONS) {
      const node = pane.querySelector(`#${id}`);
      if (node) observer.observe(node);
    }
    return () => observer.disconnect();
  }, [root]);

  return active;
}

function Index({ active, onJump }: { active: string; onJump: (id: string) => void }) {
  return (
    <nav
      className="border-border flex w-[208px] shrink-0 flex-col border-r"
      aria-label="Document sections"
    >
      {SECTIONS.map(([id, num, label]) => (
        <button
          key={id}
          type="button"
          onClick={() => onJump(id)}
          className={`border-border flex items-baseline gap-3 border-b px-4 py-3 text-left transition-colors ${
            active === id
              ? "bg-popover text-foreground"
              : "text-structure hover:bg-popover hover:text-muted-foreground"
          }`}
        >
          <span className="label-micro num">{num}</span>
          <span className="label-micro">{label}</span>
        </button>
      ))}
      <div className="min-h-6 flex-1" />
    </nav>
  );
}

export function Evidence() {
  const pane = useRef<HTMLDivElement>(null);
  const active = useActiveSection(pane);

  const jump = (id: string) => {
    pane.current?.querySelector(`#${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  return (
    <div className="flex min-h-0 min-w-0 flex-1">
      <Index active={active} onJump={jump} />
      <div ref={pane} className="min-h-0 min-w-0 flex-1 overflow-y-auto">
        <div className="flex min-w-0 flex-col gap-14 px-8 py-8 pb-28">
          <header className="flex flex-col gap-3">
            <div className="mb-1 flex items-center gap-4">
              <span className="label-micro text-[var(--primary)]">RQ-4 DRAGONFLY</span>
              <span className="h-px flex-1 bg-[var(--primary)]/35" />
            </div>
            <h1 className="t-hero text-foreground tracking-tight">Evidence</h1>
            <p className="text-foreground text-[17px] leading-[1.6]">
              A thermodynamic model of the engine runs alongside the real engine, and the two are
              compared every 50 ms. The difference between them is what the system acts on. A
              threshold monitor reports a temperature that has already gone too high. The difference
              reports an engine that has stopped behaving the way the physics predicts, which
              happens first. That claim rests on the model being correct, so this page documents it:
              the modelled engine and its type certificate, all 88 constants and their sources, the
              generated validation, the path a number takes from the bus to the screen, and the
              literature behind each part.
            </p>
          </header>

          <Equation
            tex={String.raw`r_k \;=\; y_k \;-\; h\!\left(\hat{x}_k,\; \theta_{\mathrm{nom}}\right)`}
            note="The residual. y is the 22 measured channels, h is the model prediction from the estimated state, and theta-nom pins every health parameter at its healthy value. The prediction runs against a healthy engine deliberately. An estimator free to fit the degraded engine absorbs the fault and then reports a clean result."
          />

          <Engine />
          <Parameters />
          <Validation />
          <Provenance />
          <Literature />
        </div>
      </div>
    </div>
  );
}
