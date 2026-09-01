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
    body: "GPUI lists pages and tools against built-in sample data. Run works with no Chrome. If the window is wrong, the extension cannot save it.",
  },
  {
    n: "2",
    title: "Same three tools on a page",
    body: "A vanilla demo site registers get_user, search_products, and create_note. Same JSON the playground returns.",
  },
  {
    n: "3",
    title: "Then the live tab",
    body: "A small Chrome extension pipes navigator.modelContext over a localhost socket. The same Run hits the real page.",
  },
] as const;

const faqs = [
  {
    q: "Is this an AI agent?",
    a: "No. It is a debugger. You choose the tool and press Run. There is no model in the loop, and no allow/reject gate.",
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

function Rule({ children }: { children: string }) {
  return <p className="t-label m-0 border-b border-hair pb-2">{children}</p>;
}

function Home() {
  return (
    <main id="main" className="mx-auto max-w-[640px] px-6 pb-16 pt-24 sm:px-0">
      <p className="t-label m-0">Native inspector · local-dev</p>
      <h1 className="t-hero m-0 mt-2 font-normal text-balance">
        See what a page can do, in a native window.
      </h1>
      <p className="mt-6 text-mute">
        gpuiXwebmcp lists a site’s WebMCP tools, lets you fill in one, press
        Run, and read the result plus a record of everything that happened. It
        is a developer tool, not an autonomous browser agent.
      </p>
      <p className="mt-6 flex flex-wrap items-center gap-8">
        <Link to="/try" className="act">
          Try WebMCP in this browser ›
        </Link>
        <a href={GITHUB} className="act">
          View the repo ›
        </a>
      </p>

      <div className="mt-14">
        <DebuggerPreview />
      </div>

      <section className="mt-16" aria-labelledby="pain-heading">
        <h2 id="pain-heading" className="t-label m-0 border-b border-hair pb-2 font-normal">
          The pain is simple
        </h2>
        <p className="mt-4 text-mute">
          Web pages are starting to expose tools. You still need a place to look
          at them that is not a chatbot.
        </p>
        <ul className="m-0 mt-6 list-none p-0">
          {pain.map((item) => (
            <li key={item.title} className="mb-6">
              <p className="m-0">{item.title}</p>
              <p className="m-0 text-mute">{item.body}</p>
            </li>
          ))}
        </ul>
      </section>

      <section id="what" className="mt-16 scroll-mt-8" aria-labelledby="what-heading">
        <h2 id="what-heading" className="t-label m-0 border-b border-hair pb-2 font-normal">
          What we are trying to do
        </h2>
        <p className="mt-4 text-mute">
          Open one GPUI window. See the page. See the tools. Fill the schema.
          Run. Read the history. First against sample data, then against a
          real Chrome tab. Stop before LLM, replay, or OS sidecars.
        </p>
        <div className="mt-6 flex flex-col gap-6">
          <article>
            <p className="m-0">WebMCP, in one sentence</p>
            <p className="m-0 text-mute">
              A website can publish a short menu of actions, search products,
              create a note, that a debugger or assistant can call on that
              page. Chrome is still experimental. The API lives on the
              document, not in our Rust window.
            </p>
            <Link to="/webmcp" className="act">
              Read the simple version ›
            </Link>
          </article>
          <article>
            <p className="m-0">GPUI, in one sentence</p>
            <p className="m-0 text-mute">
              Zed’s toolkit for drawing native app windows on the GPU. It is
              not a browser. It cannot see page tools by itself. That is why a
              tiny extension exists: a pipe, not a second UI.
            </p>
            <Link to="/gpui" className="act">
              Read the simple version ›
            </Link>
          </article>
        </div>
      </section>

      <section className="mt-16" aria-labelledby="how-heading">
        <h2 id="how-heading" className="t-label m-0 border-b border-hair pb-2 font-normal">
          How it works
        </h2>
        <p className="mt-4 text-mute">GPUI first, Chrome second. The UI never imports Chrome types.</p>
        <ol className="m-0 mt-6 list-none p-0">
          {steps.map((step) => (
            <li key={step.n} className="mb-6 flex gap-4">
              <span className="t-label w-4 shrink-0 pt-[5px]">{step.n}</span>
              <div>
                <p className="m-0">{step.title}</p>
                <p className="m-0 text-mute">{step.body}</p>
              </div>
            </li>
          ))}
        </ol>
        <p className="t-label m-0 mt-2">Chrome tab → WebMCP → extension → localhost socket → GPUI</p>
      </section>

      <section className="mt-16" aria-labelledby="warn-heading">
        <h2 id="warn-heading" className="t-label m-0 border-b border-hair pb-2 font-normal text-accent">
          Local-dev only
        </h2>
        <p className="mt-4 text-mute">
          Do not leave the extension loaded on untrusted sites. Run executes in
          the tab as the logged-in user. The socket binds 127.0.0.1 and only
          accepts the extension origin. That is hardening, not a production
          security boundary.
        </p>
      </section>

      <section className="mt-16" aria-labelledby="faq-heading">
        <h2 id="faq-heading" className="t-label m-0 border-b border-hair pb-2 font-normal">
          Questions
        </h2>
        <div className="mt-2">
          {faqs.map((item) => (
            <details key={item.q} className="group border-b border-hair py-2">
              <summary className="flex min-h-11 cursor-pointer list-none items-center justify-between gap-4 text-ink">
                {item.q}
                <span className="t-label" aria-hidden="true">
                  <span className="group-open:hidden">show</span>
                  <span className="hidden group-open:inline">hide</span>
                </span>
              </summary>
              <p className="m-0 pb-2 text-mute">{item.a}</p>
            </details>
          ))}
        </div>
      </section>

      <section className="mt-16" aria-labelledby="cta-heading">
        <Rule>Run the window</Rule>
        <p className="mt-4 text-mute">
          Clone the repo, then <span className="text-ink">cargo run -p debugger</span>. Press ⌃T for the
          playground. Load the extension when you want a live tab.
        </p>
        <p className="mt-2 flex flex-wrap items-center gap-8">
          <a href={GITHUB} className="act">
            Open GitHub ›
          </a>
          <Link to="/try" className="act">
            Or try WebMCP here first ›
          </Link>
        </p>
      </section>
    </main>
  );
}
