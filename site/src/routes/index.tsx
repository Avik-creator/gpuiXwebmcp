import { Link, createFileRoute } from "@tanstack/react-router";
import { DebuggerPreview } from "../components/debugger-preview";

export const Route = createFileRoute("/")({ component: Home });

const GITHUB = "https://github.com/Avik-creator/gpuiXwebmcp";

const pain = [
  {
    title: "The page has tools. You cannot see them.",
    body: "A WebMCP site publishes actions. Until you inspect them, they are invisible. Guessing from the DOM is the wrong job.",
  },
  {
    title: "Chrome already ships an inspector.",
    body: "It lives in a side panel and still executes through a JSON box. Fine for a flag. Awkward as a daily native tool.",
  },
  {
    title: "Agents hide the round-trip.",
    body: "We are not building a chat that calls tools for you. We want you to pick the tool, fill the form, and watch the log.",
  },
] as const;

const steps = [
  {
    n: "1",
    title: "Native window first",
    body: "GPUI lists pages and tools against a fixture. Execute works with no Chrome. If the window is wrong, the extension cannot save it.",
  },
  {
    n: "2",
    title: "Same three tools on a page",
    body: "A vanilla demo site registers get_user, search_products, and create_note. Same JSON the fixture returns.",
  },
  {
    n: "3",
    title: "Then the live tab",
    body: "A small Chrome extension pipes document.modelContext over localhost WebSocket. The same inspector Execute hits the real page.",
  },
] as const;

const faqs = [
  {
    q: "Is this an AI agent?",
    a: "No. It is a debugger. You choose the tool and click Execute. There is no model in the loop, and no allow/reject gate.",
  },
  {
    q: "Can I leave the extension on while I browse?",
    a: "No. Treat it like Chrome’s own inspector: local-dev only. Tool execution runs in the page as you, cookies and all.",
  },
  {
    q: "Why GPUI instead of another Chrome panel?",
    a: "We want a native window, a stable protocol, and a recorded trace we can grow into replay later. GPUI draws that window. It does not talk to the page.",
  },
] as const;

function Home() {
  return (
    <main id="main">
      <section className="mx-auto grid max-w-6xl gap-12 px-4 py-14 sm:px-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)] lg:items-center lg:py-20">
        <div>
          <p className="text-xs font-medium uppercase tracking-[0.08em] text-muted-foreground">
            Native inspector · local-dev
          </p>
          <h1 className="mt-3 max-w-xl text-4xl font-semibold tracking-tight text-balance sm:text-5xl">
            See what a page can do — in a native window.
          </h1>
          <p className="mt-4 max-w-xl text-lg text-muted-foreground">
            gpuiXwebmcp lists WebMCP tools, lets you fill a form, click Execute,
            and read the result plus an event log. It is a developer tool, not
            an autonomous browser agent.
          </p>
          <div className="mt-8 flex flex-wrap gap-3">
            <a
              href={GITHUB}
              className="inline-flex min-h-11 items-center rounded-md bg-accent px-4 text-sm font-medium text-on-accent hover:opacity-90 transition-opacity duration-200 cursor-pointer"
            >
              View the repo
            </a>
            <Link
              to="/"
              hash="what"
              className="inline-flex min-h-11 items-center rounded-md border border-border bg-muted px-4 text-sm font-medium text-foreground hover:bg-selected transition-colors duration-200 cursor-pointer"
            >
              What we are building
            </Link>
          </div>
        </div>
        <DebuggerPreview />
      </section>

      <section className="border-t border-border bg-card py-16" aria-labelledby="pain-heading">
        <div className="mx-auto max-w-6xl px-4 sm:px-6">
          <h2 id="pain-heading" className="text-2xl font-semibold">
            The pain is simple
          </h2>
          <p className="mt-2 max-w-2xl text-muted-foreground">
            Web pages are starting to expose tools. You still need a place to
            look at them that is not a chatbot.
          </p>
          <ul className="mt-8 grid gap-4 md:grid-cols-3">
            {pain.map((item) => (
              <li key={item.title} className="rounded-xl border border-border bg-background p-5">
                <h3 className="font-medium">{item.title}</h3>
                <p className="mt-2 text-sm text-muted-foreground">{item.body}</p>
              </li>
            ))}
          </ul>
        </div>
      </section>

      <section id="what" className="scroll-mt-8 py-16" aria-labelledby="what-heading">
        <div className="mx-auto max-w-6xl px-4 sm:px-6">
          <h2 id="what-heading" className="text-2xl font-semibold">
            What we are trying to do
          </h2>
          <p className="mt-3 max-w-2xl text-muted-foreground">
            Open one GPUI window. See the page. See the tools. Fill the schema.
            Execute. Watch the log. First against fake tools, then against a
            real Chrome tab. Stop before LLM, replay, or OS sidecars.
          </p>
          <div className="mt-10 grid gap-4 md:grid-cols-2">
            <article className="rounded-xl border border-border bg-card p-6">
              <h3 className="text-lg font-medium">
                <Link
                  to="/webmcp"
                  className="text-foreground hover:text-accent transition-colors duration-200 cursor-pointer"
                >
                  WebMCP, in one sentence
                </Link>
              </h3>
              <p className="mt-3 text-muted-foreground">
                A website can publish a short menu of actions — search products,
                create a note — that a debugger or assistant can call on that
                page. Chrome is still experimental. The API lives on the
                document, not in our Rust window.
              </p>
              <Link
                to="/webmcp"
                className="mt-4 inline-flex min-h-11 items-center text-sm text-accent hover:underline cursor-pointer"
              >
                Read the simple version
              </Link>
            </article>
            <article className="rounded-xl border border-border bg-card p-6">
              <h3 className="text-lg font-medium">
                <Link
                  to="/gpui"
                  className="text-foreground hover:text-accent transition-colors duration-200 cursor-pointer"
                >
                  GPUI, in one sentence
                </Link>
              </h3>
              <p className="mt-3 text-muted-foreground">
                Zed’s toolkit for drawing native app windows on the GPU. It is
                not a browser. It cannot see page tools by itself. That is why
                a tiny extension exists: a pipe, not a second UI.
              </p>
              <Link
                to="/gpui"
                className="mt-4 inline-flex min-h-11 items-center text-sm text-accent hover:underline cursor-pointer"
              >
                Read the simple version
              </Link>
            </article>
          </div>
        </div>
      </section>

      <section className="border-t border-border bg-card py-16" aria-labelledby="how-heading">
        <div className="mx-auto max-w-6xl px-4 sm:px-6">
          <h2 id="how-heading" className="text-2xl font-semibold">
            How it works
          </h2>
          <p className="mt-2 max-w-2xl text-muted-foreground">
            GPUI first, Chrome second. The UI never imports Chrome types.
          </p>
          <ol className="mt-8 grid gap-4 md:grid-cols-3">
            {steps.map((step) => (
              <li key={step.n} className="rounded-xl border border-border bg-background p-5">
                <p className="font-mono text-sm text-accent">{step.n}</p>
                <h3 className="mt-2 font-medium">{step.title}</h3>
                <p className="mt-2 text-sm text-muted-foreground">{step.body}</p>
              </li>
            ))}
          </ol>
          <p className="mt-8 font-mono text-sm text-muted-foreground">
            Chrome tab → WebMCP → extension → localhost socket → GPUI
          </p>
        </div>
      </section>

      <section className="py-16" aria-labelledby="warn-heading">
        <div className="mx-auto max-w-6xl px-4 sm:px-6">
          <div
            className="rounded-xl border border-destructive bg-muted p-5"
            role="note"
            aria-labelledby="warn-heading"
          >
            <h2 id="warn-heading" className="font-medium text-destructive">
              Local-dev only
            </h2>
            <p className="mt-2 max-w-3xl text-sm text-muted-foreground">
              Do not leave the extension loaded on untrusted sites. Execute runs
              in the tab as the logged-in user. The socket binds 127.0.0.1 and
              only accepts the extension origin — that is hardening, not a
              production security boundary.
            </p>
          </div>
        </div>
      </section>

      <section className="border-t border-border bg-card py-16" aria-labelledby="faq-heading">
        <div className="mx-auto max-w-3xl px-4 sm:px-6">
          <h2 id="faq-heading" className="text-2xl font-semibold">
            Questions
          </h2>
          <div className="mt-6 divide-y divide-border rounded-xl border border-border bg-background">
            {faqs.map((item) => (
              <details key={item.q} className="group p-4">
                <summary className="min-h-11 cursor-pointer list-none font-medium after:float-right after:text-muted-foreground after:content-['+'] group-open:after:content-['–']">
                  {item.q}
                </summary>
                <p className="mt-3 text-sm text-muted-foreground">{item.a}</p>
              </details>
            ))}
          </div>
        </div>
      </section>

      <section className="py-16" aria-labelledby="cta-heading">
        <div className="mx-auto max-w-6xl px-4 text-center sm:px-6">
          <h2 id="cta-heading" className="text-2xl font-semibold">
            Run the window
          </h2>
          <p className="mx-auto mt-3 max-w-xl text-muted-foreground">
            Clone the repo, then{" "}
            <code className="font-mono text-foreground">cargo run -p debugger</code>
            . Click the status pill for Demo. Load the extension when you
            want a Chrome tab.
          </p>
          <a
            href={GITHUB}
            className="mt-8 inline-flex min-h-11 items-center rounded-md bg-accent px-5 text-sm font-medium text-on-accent hover:opacity-90 transition-opacity duration-200 cursor-pointer"
          >
            Open GitHub
          </a>
        </div>
      </section>
    </main>
  );
}
