import { Link, createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/webmcp")({ component: WebmcpPage });

function WebmcpPage() {
  return (
    <main id="main" className="mx-auto max-w-3xl px-4 py-14 sm:px-6">
      <p className="text-xs font-medium uppercase tracking-[0.08em] text-muted-foreground">
        Plain language
      </p>
      <h1 className="mt-3 text-4xl font-semibold tracking-tight">What WebMCP is</h1>
      <p className="mt-4 text-lg text-muted-foreground">
        WebMCP lets a website offer a list of actions other software can call.
        Think of it as a labeled remote for that page: “get the current user”,
        “search the catalog”, “save a note”.
      </p>

      <section className="mt-10 space-y-4 text-muted-foreground">
        <h2 className="text-xl font-medium text-foreground">Why that matters</h2>
        <p>
          Without it, a helper has to click around like a person, or scrape the
          HTML. With it, the page says “here are my tools” and “here is the
          shape of the arguments”. You call a tool. The page does the work in
          the user’s session.
        </p>
        <p>
          That last part is the sharp edge. If you are logged into mail or a
          bank, a mutating tool is you acting. A “read-only” hint is only a
          hint. That is why this project is a debugger you drive by hand, not
          an agent that fires tools on its own.
        </p>
      </section>

      <section className="mt-10 rounded-xl border border-border bg-card p-6">
        <h2 className="text-xl font-medium">In this repo</h2>
        <p className="mt-3 text-muted-foreground">
          Chrome exposes the list on the document (experimental; you turn on a
          testing flag). Our demo page registers three tools that match the
          GPUI fixture: get_user, search_products, create_note. The native app
          never names that document API. A content script does, then talks to
          GPUI over a localhost socket.
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
