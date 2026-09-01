/**
 * Isolated-world content script. Reads document.modelContext the same way
 * Chrome's model-context-tool-inspector does. Do not wrap registerTool.
 */

const LIST_DEBOUNCE_MS = 200;
let listTimer = 0;

function hasModelContext() {
  return typeof document.modelContext === "object" && document.modelContext !== null;
}

function normalizeInputSchema(inputSchema) {
  if (typeof inputSchema === "string") {
    try {
      return JSON.parse(inputSchema);
    } catch {
      return {};
    }
  }
  if (inputSchema && typeof inputSchema === "object") {
    return inputSchema;
  }
  return {};
}

function normalizeTool(tool) {
  const annotations = tool.annotations && typeof tool.annotations === "object"
    ? tool.annotations
    : {};
  return {
    name: String(tool.name ?? ""),
    title: tool.title == null ? undefined : String(tool.title),
    description: String(tool.description ?? ""),
    input_schema: normalizeInputSchema(tool.inputSchema),
    annotations: {
      read_only_hint:
        typeof annotations.readOnlyHint === "boolean"
          ? annotations.readOnlyHint
          : undefined,
      untrusted_content_hint:
        typeof annotations.untrustedContentHint === "boolean"
          ? annotations.untrustedContentHint
          : undefined,
    },
  };
}

function pageFields() {
  return {
    url: window.location.href,
    title: document.title || window.location.href,
    origin: window.location.origin,
  };
}

function toJsonValue(result) {
  if (result == null) {
    return null;
  }
  if (typeof result === "string") {
    try {
      const once = JSON.parse(result);
      if (typeof once === "string") {
        try {
          return JSON.parse(once);
        } catch {
          return once;
        }
      }
      return once;
    } catch {
      return result;
    }
  }
  return result;
}

async function listTools() {
  if (window !== window.top) {
    return;
  }
  if (!hasModelContext()) {
    chrome.runtime.sendMessage({
      type: "tools_unavailable",
      ...pageFields(),
    });
    return;
  }

  const discovered = await document.modelContext.getTools();
  const tools = [];
  for (const tool of discovered) {
    tools.push(normalizeTool(tool));
  }
  chrome.runtime.sendMessage({
    type: "tools_changed",
    tools,
    ...pageFields(),
  });
}

function debouncedListTools() {
  clearTimeout(listTimer);
  listTimer = setTimeout(() => {
    listTools().catch((error) => {
      console.warn("[WebMCP bridge] listTools failed", error);
    });
  }, LIST_DEBOUNCE_MS);
}

function watchTools() {
  if (!hasModelContext() || window !== window.top) {
    return;
  }
  document.modelContext.ontoolchange = debouncedListTools;
}

async function executeNamedTool(name, inputArgs) {
  if (!hasModelContext()) {
    throw new Error(
      'You must run Chrome with the "WebMCP for testing" flag enabled.'
    );
  }
  const tools = await document.modelContext.getTools();
  const tool = tools.find((candidate) => candidate.name === name);
  if (!tool) {
    throw new Error(`tool not found: ${name}`);
  }

  const args = inputArgs == null ? {} : inputArgs;
  try {
    return await document.modelContext.executeTool(tool, args);
  } catch (error) {
    const message = error && error.message ? String(error.message) : String(error);
    if (message.startsWith("Failed to parse input")) {
      return await document.modelContext.executeTool(tool, JSON.stringify(args));
    }
    throw error;
  }
}

chrome.runtime.onMessage.addListener((message, _sender, reply) => {
  if (window !== window.top) {
    return;
  }
  if (message.action === "LIST_TOOLS") {
    watchTools();
    debouncedListTools();
    reply({ ok: true });
    return;
  }
  if (message.action === "EXECUTE_TOOL") {
    executeNamedTool(message.name, message.arguments)
      .then((result) => reply({ ok: true, result: toJsonValue(result) }))
      .catch((error) =>
        reply({ ok: false, error: String(error && error.message ? error.message : error) })
      );
    return true;
  }
  return undefined;
});

function boot() {
  if (window !== window.top) {
    return;
  }
  watchTools();
  debouncedListTools();
  chrome.runtime.sendMessage({ type: "content_script_ready", ...pageFields() }).catch(
    () => {}
  );
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot, { once: true });
} else {
  boot();
}
