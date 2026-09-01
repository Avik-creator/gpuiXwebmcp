const GITHUB = "https://github.com/Avik-creator/gpuiXwebmcp";

export function SiteFooter() {
  return (
    <footer className="mx-auto flex w-full max-w-5xl flex-col gap-2 px-6 pb-12 pt-16 sm:flex-row sm:items-center sm:justify-between sm:px-10">
      <p className="t-label m-0">Local-dev inspector · not an agent · not for untrusted sites</p>
      <a href={GITHUB} className="t-label inline-flex min-h-11 items-center text-mute hover:text-ink transition-colors duration-150">
        github.com/Avik-creator/gpuiXwebmcp
      </a>
    </footer>
  );
}
