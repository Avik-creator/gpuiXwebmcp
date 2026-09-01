const TOOLS = [
  { name: "get_user", title: "Get user", badge: "read-only", selected: false },
  { name: "search_products", title: "Search products", badge: "read-only", selected: true },
  { name: "create_note", title: "Create note", badge: "untrusted", selected: false },
] as const;

const LOG = [
  "17:54:01  HELLO  fixture backend ready",
  "17:54:02  PAGE_CHANGED  http://localhost:5173/",
  "17:54:02  TOOLS_CHANGED  discovered 3 tools",
  "13:51:04  TOOL_EXECUTION_STARTED  search_products",
  "13:51:04  TOOL_EXECUTION_FINISHED  search_products 112ms",
] as const;

export function DebuggerPreview() {
  return (
    <figure className="overflow-hidden rounded-xl border border-border bg-background shadow-[0_0_40px_rgba(15,23,42,0.8)]">
      <figcaption className="sr-only">
        Mock of the WebMCP Debugger window: site field, pages, tools, inspector, and event log.
      </figcaption>
      <div className="border-b border-border bg-card">
        <div className="flex items-center justify-between px-4 py-3">
          <p className="text-sm">WebMCP Debugger</p>
          <p className="flex items-center gap-2 text-sm text-muted-foreground">
            <span className="size-2 rounded-full bg-accent" aria-hidden="true" />
            Demo
          </p>
        </div>
        <div className="flex items-center gap-2 border-t border-border px-4 py-2">
          <p className="text-sm text-muted-foreground">SITE</p>
          <p className="min-w-0 flex-1 truncate rounded-md border border-border bg-muted px-3 py-2 font-mono text-sm">
            http://localhost:5173
          </p>
          <p className="rounded-md bg-accent px-3 py-2 text-sm font-medium text-on-accent">GO</p>
        </div>
      </div>
      <div className="grid min-h-[22rem] grid-cols-1 md:grid-cols-[11rem_13rem_1fr]">
        <section className="border-b border-border md:border-b-0 md:border-r" aria-label="Pages">
          <h3 className="border-b border-border px-4 py-2 text-sm text-muted-foreground">Pages</h3>
          <div className="p-2">
            <div className="rounded-md bg-selected px-3 py-2">
              <p className="flex items-center gap-2 text-sm">
                <span className="size-1.5 rounded-full bg-accent" aria-hidden="true" />
                http://localhost:5173
              </p>
              <p className="pl-4 text-sm text-muted-foreground">WebMCP demo</p>
            </div>
          </div>
        </section>
        <section className="border-b border-border md:border-b-0 md:border-r" aria-label="Tools">
          <h3 className="border-b border-border px-4 py-2 text-sm text-muted-foreground">Tools</h3>
          <ul className="flex flex-col gap-1 p-2">
            {TOOLS.map((tool) => (
              <li
                key={tool.name}
                className={`rounded-md px-3 py-2 ${tool.selected ? "bg-selected" : "bg-card"}`}
              >
                <p className="font-mono text-sm">{tool.name}</p>
                <p className="text-sm text-muted-foreground">{tool.title}</p>
                <p className="text-sm text-muted-foreground">{tool.badge}</p>
              </li>
            ))}
          </ul>
        </section>
        <section aria-label="Inspector">
          <h3 className="border-b border-border px-4 py-2 text-sm text-muted-foreground">Inspector</h3>
          <div className="flex flex-col gap-3 p-4">
            <p className="font-mono text-sm">search_products</p>
            <p className="text-sm text-muted-foreground">Search products by query</p>
            <div>
              <p className="mb-1 text-sm text-muted-foreground">query *</p>
              <p className="rounded-md border border-border bg-muted px-3 py-2 font-mono text-sm">gpui</p>
            </div>
            <p className="flex min-h-11 items-center justify-center rounded-md bg-accent text-sm font-medium text-on-accent">
              Execute
            </p>
            <pre className="overflow-x-auto rounded-md bg-muted p-3 font-mono text-xs leading-5 text-foreground">
{`{
  "query": "gpui",
  "results": [
    { "title": "Programming GPUI" },
    { "title": "WebMCP in Practice" }
  ]
}`}
            </pre>
          </div>
        </section>
      </div>
      <section className="border-t border-border bg-card" aria-label="Event log">
        <h3 className="border-b border-border px-4 py-2 text-sm text-muted-foreground">Event Log</h3>
        <ul className="space-y-1 px-4 py-3 font-mono text-xs text-muted-foreground">
          {LOG.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      </section>
    </figure>
  );
}
