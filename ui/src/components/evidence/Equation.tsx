/**
 * A display equation, rendered with KaTeX.
 *
 * KaTeX and not hand-marked HTML: sub- and superscripts alone can carry these
 * five formulas, and that was tried, but a radical written as a fractional
 * exponent and a hat built from a combining character both read as improvised.
 * A reviewer who works with these equations daily sees the difference.
 *
 * It renders in Computer Modern, so the maths sits in a serif on a page set in
 * Geist Mono. That contrast is accepted: an equation is a quotation from the
 * literature, and it reads as one.
 *
 * Rendered once into `innerHTML` at mount. `throwOnError` is off so a malformed
 * formula degrades to its own source text rather than taking the screen down
 * during a demonstration.
 */

import katex from "katex";
import { useEffect, useRef } from "react";

export function Equation({ tex, note }: { tex: string; note?: string }) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (host.current) {
      katex.render(tex, host.current, { displayMode: true, throwOnError: false });
    }
  }, [tex]);

  return (
    <div className="border-border bg-card flex min-w-0 flex-col gap-2 overflow-x-auto border border-l-2 border-l-[var(--primary)] px-5 py-4">
      <div ref={host} className="text-foreground text-[16px]" />
      {note ? (
        <span className="text-muted-foreground text-[12.5px] leading-[1.6]">{note}</span>
      ) : null}
    </div>
  );
}
