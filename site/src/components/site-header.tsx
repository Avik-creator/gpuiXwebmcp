import { Link } from "@tanstack/react-router";

const GITHUB = "https://github.com/Avik-creator/gpuiXwebmcp";

const PLACES = [
  { to: "/", label: "OVERVIEW" },
  { to: "/webmcp", label: "WEBMCP" },
  { to: "/gpui", label: "GPUI" },
  { to: "/try", label: "TRY IT" },
] as const;

const placeClass = "t-label inline-flex min-h-11 items-center text-mute hover:text-ink transition-colors duration-150";

/** The bar: places on the left with the current one lit, a report on the right. */
export function SiteHeader() {
  return (
    <header className="h-14 shrink-0">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:bg-paper focus:px-3 focus:py-2 focus:text-ink"
      >
        Skip to content
      </a>
      <div className="mx-auto flex h-full max-w-5xl items-center justify-between gap-6 px-6 sm:px-10">
        <nav aria-label="Primary" className="flex items-center gap-6 overflow-x-auto">
          <Link to="/" className="t-label inline-flex min-h-11 shrink-0 items-center text-mute hover:text-ink transition-colors duration-150">
            ‹ gpuiXwebmcp
          </Link>
          {PLACES.map((place) => (
            <Link
              key={place.to}
              to={place.to}
              className={`${placeClass} shrink-0`}
              activeOptions={{ exact: place.to === "/" }}
              activeProps={{ className: "text-ink" }}
            >
              {place.label}
            </Link>
          ))}
        </nav>
        <a href={GITHUB} className="t-label inline-flex min-h-11 shrink-0 items-center text-mute hover:text-ink transition-colors duration-150">
          GitHub ›
        </a>
      </div>
    </header>
  );
}
