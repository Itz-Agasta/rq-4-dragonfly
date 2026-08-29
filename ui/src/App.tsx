/**
 * Application shell.
 *
 * Structure only. The real shell is the 64px nav rail plus a 40px top bar, with six
 * screens routed beneath it (design.md sec 3). Nothing here is built yet: the D4
 * physics gate in docs/mvp.md comes first, and the twin has to produce data before a
 * screen has anything honest to render.
 */
export function App() {
  return (
    <div className="bg-background text-foreground flex h-full w-full items-center justify-center">
      <span className="label-micro">RQ-4 DRAGONFLY</span>
    </div>
  );
}
