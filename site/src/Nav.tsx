import { useEffect, useRef, useState } from "react";

import { OPS, REPO } from "./links";

const MENU = [
  { label: "Dashboard", href: OPS },
  { label: "The problem", href: "#top" },
  { label: "The product", href: "#product" },
  { label: "The proof", href: "#proof" },
  { label: "Read the model", href: `${REPO}#readme` },
  { label: "Source", href: REPO },
];

// The ground station's own mark (ui/index.html favicon), so site and GCS share one.
function Mark() {
  return (
    <svg
      viewBox="0 0 20 20"
      width="20"
      height="20"
      aria-hidden
      fill="none"
      stroke="#ff6b35"
      strokeWidth="1.5"
    >
      <path d="M10 2.2V17.9" />
      <path d="M3 4.9 10 7.1 17 4.9" />
      <path d="M2.2 13.1 10 10.3 17.8 13.1" />
    </svg>
  );
}

/**
 * Armory's navbar grammar: a dark inset bar, two small links with orange squares,
 * a menu that opens over a blurred page. It slides away while the reader scrolls
 * down, so it never sits on the film, and comes back the moment they scroll up.
 */
export function Nav() {
  const bar = useRef<HTMLElement>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let last = scrollY;
    const onScroll = () => {
      const y = scrollY;
      // 8 px of hysteresis: a trackpad's settle wobble must not flicker the bar.
      if (Math.abs(y - last) < 8) return;
      bar.current?.classList.toggle("nav-hidden", y > last && y > 120);
      last = y;
    };
    addEventListener("scroll", onScroll, { passive: true });
    return () => removeEventListener("scroll", onScroll);
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("menu-open", open);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  }, [open]);

  return (
    <header className="nav" ref={bar}>
      <div className="nav-bar">
        <a className="nav-logo" href="#top" onClick={() => setOpen(false)}>
          <Mark />
          <span>DRAGONFLY</span>
        </a>
        <nav className="nav-links" aria-label="Primary">
          <a href={OPS}>Dashboard</a>
          <a href={`${REPO}#readme`}>Read the model</a>
        </nav>
        <button
          className={`nav-menu ${open ? "is-open" : ""}`}
          aria-expanded={open}
          aria-label="Menu"
          onClick={() => setOpen((o) => !o)}
        >
          <i />
          <i />
          <i />
        </button>
      </div>
      <div className={`nav-panel ${open ? "is-open" : ""}`} aria-hidden={!open}>
        <ul>
          {MENU.map((m, i) => (
            <li key={m.label}>
              <a
                href={m.href}
                onClick={() => setOpen(false)}
                tabIndex={open ? 0 : -1}
                style={{ transitionDelay: open ? `${60 + i * 40}ms` : "0ms" }}
              >
                {m.label}
              </a>
            </li>
          ))}
        </ul>
      </div>
    </header>
  );
}
