const BOOKS = [
  { id: "book-1", title: "Programming GPUI", author: "Zed Industries" },
  { id: "book-2", title: "WebMCP in Practice", author: "Chrome Team" },
];

const notes = [];

function supportsWebMCP() {
  return "modelContext" in document;
}

function $(id) {
  const node = document.getElementById(id);
  if (!node) {
    throw new Error(`missing element #${id}`);
  }
  return node;
}

function setStatus(state, text) {
  const status = $("webmcp-status");
  status.dataset.state = state;
  status.textContent = text;
}

function showResult(value) {
  $("last-result").textContent = JSON.stringify(value, null, 2);
}

function renderProducts(results) {
  const list = $("product-list");
  list.replaceChildren();
  for (const book of results) {
    const item = document.createElement("li");
    item.textContent = `${book.title} — ${book.author}`;
    list.append(item);
  }
}

function renderNotes() {
  const list = $("note-list");
  list.replaceChildren();
  if (notes.length === 0) {
    const item = document.createElement("li");
    item.className = "muted";
    item.textContent = "No notes yet. Call create_note.";
    list.append(item);
    return;
  }
  for (const note of notes) {
    const item = document.createElement("li");
    item.textContent = note;
    list.append(item);
  }
}

function renderToolNames(names) {
  const list = $("tool-list");
  list.replaceChildren();
  for (const name of names) {
    const item = document.createElement("li");
    item.textContent = name;
    list.append(item);
  }
}

export async function getUser() {
  const profile = {
    id: "user_1",
    name: "Ada Lovelace",
    email: "ada@localhost",
  };
  $("user-output").textContent = JSON.stringify(profile, null, 2);
  showResult(profile);
  return profile;
}

export async function searchProducts({ query } = {}) {
  const trimmed = typeof query === "string" ? query.trim() : "";
  if (!trimmed) {
    throw new Error("query is required");
  }
  const payload = {
    query: trimmed,
    results: BOOKS,
  };
  renderProducts(payload.results);
  showResult(payload);
  return payload;
}

export async function createNote({ text } = {}) {
  const trimmed = typeof text === "string" ? text.trim() : "";
  if (!trimmed) {
    throw new Error("text is required");
  }
  notes.push(trimmed);
  renderNotes();
  const payload = { ok: true, text: trimmed };
  showResult(payload);
  return payload;
}

const TOOLS = [
  {
    name: "get_user",
    title: "Get user",
    description: "Return the current demo user profile",
    inputSchema: {
      type: "object",
      properties: {},
    },
    annotations: {
      readOnlyHint: true,
    },
    execute: getUser,
  },
  {
    name: "search_products",
    title: "Search products",
    description: "Search products by query",
    inputSchema: {
      type: "object",
      properties: {
        query: { type: "string" },
      },
      required: ["query"],
    },
    annotations: {
      readOnlyHint: true,
    },
    execute: searchProducts,
  },
  {
    name: "create_note",
    title: "Create note",
    description: "Create a note from text",
    inputSchema: {
      type: "object",
      properties: {
        text: { type: "string" },
      },
      required: ["text"],
    },
    annotations: {
      readOnlyHint: false,
      untrustedContentHint: true,
    },
    execute: createNote,
  },
];

async function registerTools() {
  const banner = $("flag-banner");
  if (!supportsWebMCP()) {
    banner.hidden = false;
    setStatus("missing", "WebMCP unavailable");
    renderToolNames(TOOLS.map((tool) => tool.name));
    return;
  }

  banner.hidden = true;

  for (const tool of TOOLS) {
    await document.modelContext.registerTool(tool);
  }

  let names = TOOLS.map((tool) => tool.name);
  try {
    const discovered = await document.modelContext.getTools();
    names = discovered.map((tool) => tool.name);
  } catch (error) {
    console.warn("getTools failed after registerTool", error);
  }

  renderToolNames(names);
  setStatus("ready", `${names.length} tools registered`);
}

renderProducts(BOOKS);
renderNotes();
registerTools().catch((error) => {
  console.error(error);
  setStatus("missing", "Registration failed");
  $("flag-banner").hidden = false;
  $("flag-banner").textContent =
    "document.modelContext.registerTool failed. See the console for the error.";
});
