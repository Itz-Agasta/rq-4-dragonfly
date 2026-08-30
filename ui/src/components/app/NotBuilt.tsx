/**
 * Placeholder for a screen that does not exist yet.
 *
 * Says so plainly rather than rendering an empty dashboard. An empty dashboard is
 * indistinguishable from a broken one, and this way the rail is demonstrable
 * before the screens behind it are.
 */

export function NotBuilt({ title, note }: { title: string; note: string }) {
  return (
    <div className="marks relative flex min-h-0 flex-1 flex-col items-center justify-center gap-3">
      <span className="t-section text-foreground-dim tracking-[0.12em]">{title}</span>
      <span className="label-micro">{note}</span>
    </div>
  );
}
