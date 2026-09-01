import { Link, createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/gpui")({ component: GpuiPage });

function GpuiPage() {
  return (
    <main id="main" className="mx-auto max-w-[640px] px-6 pb-16 pt-24 sm:px-0">
      <p className="t-label m-0">Plain language</p>
      <h1 className="t-focus m-0 mt-2 font-normal">What GPUI is</h1>
      <p className="mt-6 text-mute">
        GPUI is the UI toolkit behind the Zed editor. It draws windows, text,
        and clicks on the GPU. Our debugger is one of those windows.
      </p>

      <section className="mt-12">
        <p className="t-label m-0 border-b border-hair pb-2">What it is not</p>
        <p className="mt-4 text-mute">
          It is not a web browser. It does not load localhost:5173 inside
          itself. The site field only tells Chrome which tab to inspect. It
          does not sandbox you. The binary runs as you, on your machine, same
          as any other native app.
        </p>
        <p className="mt-4 text-mute">
          It also cannot see WebMCP. Tools live on the Chrome tab. GPUI talks
          to a small protocol: pages, tools, execute, log. A Chrome extension
          is the only piece that touches the page.
        </p>
      </section>

      <section className="mt-12">
        <p className="t-label m-0 border-b border-hair pb-2">Why we used it here</p>
        <p className="mt-4 text-mute">
          We wanted the inspector to read as a native tool rather than a web
          page in a frame: near-black ground, warm bone text, one rust accent
          reserved for faults, and a recorded trace of everything you ran. GPUI
          gives us that surface. Chrome’s own inspector stays a side panel.
          Different job.
        </p>
        <p className="mt-4 text-mute">
          This site is drawn with the same five tokens and the same three
          voices, so what you see here is what the window looks like.
        </p>
      </section>

      <p className="mt-12">
        <Link to="/" className="act">
          ‹ Back to the overview
        </Link>
      </p>
    </main>
  );
}
