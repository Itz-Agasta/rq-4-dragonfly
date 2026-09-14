/**
 * Command confirmations, bottom centre of the screen.
 *
 * Rendered from the shell rather than from the panel that raises one, so a
 * confirmation outlives the drawer that issued it. The INJECT FAULT drawer
 * closes on confirm and its own footer line went with it, which is exactly the
 * case this exists for: the operator presses, the drawer gets out of the way,
 * and the answer is still on screen while they look back at the engine.
 *
 * Bottom centre, over the strips. Deliberately not the right column, which is
 * where alerts live: a transient confirmation appearing in the alarm surface
 * would read as an alarm.
 */

import { useToasts } from "@/store/toast";

export function Toasts() {
  const items = useToasts((s) => s.items);

  if (items.length === 0) return null;

  return (
    <div
      // Inert. Nothing here is clickable, so the stack cannot swallow a click
      // meant for the strips underneath it. That is also why there is no
      // dismiss button: the 7 s timeout is the only way one leaves, which costs
      // an impatient operator nothing and removes a whole class of fault.
      className="pointer-events-none fixed bottom-6 left-1/2 z-50 flex -translate-x-1/2 flex-col-reverse gap-2"
      role="status"
      aria-live="polite"
    >
      {items.map((item) => (
        <div
          key={item.id}
          className="bg-card border-border flex max-w-[440px] min-w-[300px] items-start gap-3 border py-[10px] pr-4 pl-[13px]"
        >
          <span
            aria-hidden="true"
            className={`mt-[2px] h-[26px] w-[2px] shrink-0 ${
              item.tone === "crit" ? "bg-crit" : "bg-foreground"
            }`}
          />
          <div className="flex min-w-0 flex-col gap-[3px]">
            <span className="t-small text-foreground">{item.text}</span>
            {item.detail ? (
              <span className="label-micro leading-[1.45] normal-case">{item.detail}</span>
            ) : null}
          </div>
        </div>
      ))}
    </div>
  );
}
