import { Link } from "@tanstack/react-router";

const GITHUB = "https://github.com/Avik-creator/gpuiXwebmcp";

const navClass = "min-h-11 inline-flex items-center px-2 text-sm text-muted-foreground hover:text-foreground transition-colors duration-200 cursor-pointer";
const activeNavClass = "text-foreground";

export function SiteHeader() {
  return (
    <header className="border-b border-border bg-card">
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-accent focus:px-3 focus:py-2 focus:text-on-accent"
      >
        Skip to content
      </a>
      <div className="mx-auto flex max-w-6xl items-center justify-between gap-4 px-4 py-3 sm:px-6">
        <Link
          to="/"
          className="min-h-11 inline-flex items-center gap-2 text-sm font-medium tracking-[0.08em] uppercase text-muted-foreground hover:text-foreground transition-colors duration-200 cursor-pointer"
        >
          <span className="size-2 rounded-full bg-accent" aria-hidden="true" />
          gpuiXwebmcp
        </Link>
        <nav aria-label="Primary" className="flex flex-wrap items-center justify-end gap-1">
          <Link to="/" hash="what" className={navClass} activeProps={{ className: activeNavClass }}>
            What it is
          </Link>
          <Link to="/webmcp" className={navClass} activeProps={{ className: activeNavClass }}>
            WebMCP
          </Link>
          <Link to="/gpui" className={navClass} activeProps={{ className: activeNavClass }}>
            GPUI
          </Link>
          <a
            href={GITHUB}
            className="min-h-11 inline-flex items-center rounded-md bg-accent px-3 text-sm font-medium text-on-accent hover:opacity-90 transition-opacity duration-200 cursor-pointer"
          >
            View repo
          </a>
        </nav>
      </div>
    </header>
  );
}
