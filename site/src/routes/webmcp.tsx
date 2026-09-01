import { Link, createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/webmcp")({ component: WebmcpPage });

function WebmcpPage() {
  return (
    <main id="main" className="mx-auto max-w-[640px] px-6 pb-16 pt-24 sm:px-0">
      <p className="t-label m-0">Plain language</p>
      <h1 className="t-focus m-0 mt-2 font-normal">What WebMCP is</h1>
      <p className="mt-6 text-mute">
        WebMCP lets a website offer a list of actions other software can call.
        Think of it as a labeled remote for that page: “get the current user”,
        “search the catalog”, “save a note”.
      </p>

      <section className="mt-12">
        <p className="t-label m-0 border-b border-hair pb-2">Why that matters</p>
        <p className="mt-4 text-mute">
          Without it, a helper has to click around like a person, or scrape the
          HTML. With it, the page says “here are my tools” and “here is the
          shape of the arguments”. You call a tool. The page does the work in
          the user’s session.
        </p>
        <p className="mt-4 text-mute">
          That last part is the sharp edge. If you are logged into mail or a
          bank, a mutating tool is you acting. A “read-only” hint is only a
          hint. That is why this project is a debugger you drive by hand, not
          an agent that fires tools on its own.
        </p>
      </section>

      <section className="mt-12">
        <p className="t-label m-0 border-b border-hair pb-2">In this repo</p>
        <p className="mt-4 text-mute">
          Chrome exposes the list on the document behind a testing flag. The
          bundled demo page registers three tools that match the playground:
          get_user, search_products, create_note. The native window never names
          that document API. A content script does, then talks to GPUI over a
          localhost socket.
        </p>
        <p className="mt-4 text-mute">
          You can register one yourself, right here, and run it.
        </p>
        <p className="mt-6">
          <Link to="/try" className="act">
            Try it ›
          </Link>
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
