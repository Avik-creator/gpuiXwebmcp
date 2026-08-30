const GITHUB = "https://github.com/Avik-creator/gpuiXwebmcp";

export function SiteFooter() {
  return (
    <footer className="border-t border-border bg-card">
      <div className="mx-auto flex max-w-6xl flex-col gap-2 px-4 py-8 text-sm text-muted-foreground sm:flex-row sm:items-center sm:justify-between sm:px-6">
        <p>Local-dev inspector. Not an agent. Not for untrusted sites.</p>
        <a
          href={GITHUB}
          className="min-h-11 inline-flex items-center text-foreground hover:text-accent transition-colors duration-200 cursor-pointer"
        >
          github.com/Avik-creator/gpuiXwebmcp
        </a>
      </div>
    </footer>
  );
}
