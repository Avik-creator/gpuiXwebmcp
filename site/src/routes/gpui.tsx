import { Link, createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/gpui")({ component: GpuiPage });

function GpuiPage() {
  return (
    <main id="main" className="mx-auto max-w-3xl px-4 py-14 sm:px-6">
      <p className="text-xs font-medium uppercase tracking-[0.08em] text-muted-foreground">
        Plain language
      </p>
      <h1 className="mt-3 text-4xl font-semibold tracking-tight">What GPUI is</h1>
      <p className="mt-4 text-lg text-muted-foreground">
        GPUI is the UI toolkit behind the Zed editor. It draws windows, text,
        and clicks on the GPU. Our debugger is one of those windows.
      </p>

      <section className="mt-10 space-y-4 text-muted-foreground">
        <h2 className="text-xl font-medium text-foreground">What it is not</h2>
        <p>
          It is not a web browser. It does not load localhost:5173 inside
          itself. The SITE field in the header only tells Chrome which tab to
          inspect. It does not sandbox you. The binary runs as you, on your
          machine, same as any other native app.
        </p>
        <p>
          It also cannot see WebMCP. Tools live on the Chrome tab. GPUI talks
          to a small protocol: pages, tools, execute, log. A Chrome extension
          is the only piece that touches the page.
        </p>
      </section>

      <section className="mt-10 rounded-xl border border-border bg-card p-6">
        <h2 className="text-xl font-medium">Why we used it here</h2>
        <p className="mt-3 text-muted-foreground">
          We wanted the inspector to read as a native tool rather than a web page
          in a frame: near-black ground, warm bone text, one rust accent reserved
          for faults, and a recorded trace of everything you ran. GPUI gives us
          that surface. Chrome’s own inspector stays a side panel. Different job.
        </p>
      </section>

      <p className="mt-10">
        <Link
          to="/"
          className="inline-flex min-h-11 items-center text-accent hover:underline cursor-pointer"
        >
          Back to the overview
        </Link>
      </p>
    </main>
  );
}
