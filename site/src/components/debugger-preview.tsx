const TOOLS = [
  { name: "get_user", access: "Only reads", mutates: false, blurb: "Return the current demo user profile", selected: false },
  { name: "search_products", access: "Only reads", mutates: false, blurb: "Search products by query", selected: true },
  { name: "create_note", access: "Can change things", mutates: true, blurb: "Create a note from text", selected: false },
] as const;

/** The window as it is: a bar, then the Tools screen in one quiet column. */
export function DebuggerPreview() {
  return (
    <figure className="m-0 border border-hair bg-paper">
      <figcaption className="sr-only">
        The debugger window: the bar with Tools, Run and History, then the list of tools the page offers.
      </figcaption>
      <div className="flex h-14 items-center justify-between gap-4 px-6">
        <p className="t-label m-0 flex items-center gap-5">
          <span className="text-hair">‹ BACK</span>
          <span className="text-hair">›</span>
          <span className="text-ink">TOOLS</span>
          <span>RUN</span>
          <span>HISTORY</span>
        </p>
        <p className="t-label m-0 hidden min-w-0 items-center gap-2 sm:flex">
          <span className="truncate">http://localhost:5173</span>
          <span>·</span>
          <span>Chrome connected</span>
        </p>
      </div>
      <div className="px-6 pb-10 pt-8 sm:px-12">
        <p className="t-label m-0">Open a site</p>
        <div className="mt-2 flex items-center justify-between gap-4 border-b border-hair pb-2">
          <span className="min-w-0 flex-1 truncate border border-dashed border-hair px-3 py-2">http://localhost:5173</span>
          <span className="t-label text-ink">OPEN</span>
        </div>
        <ul className="m-0 mt-10 list-none p-0">
          {TOOLS.map((tool) => (
            <li key={tool.name} className="mb-6">
              <p className="m-0">{tool.selected ? `${tool.name} ›` : tool.name}</p>
              <p className="m-0 text-mute">{tool.blurb}</p>
              <p className={`t-label m-0 mt-1 ${tool.mutates ? "text-accent" : ""}`}>{tool.access}</p>
            </li>
          ))}
        </ul>
        <p className="t-label m-0 mt-2">Try the playground &nbsp;⌃T</p>
      </div>
    </figure>
  );
}
