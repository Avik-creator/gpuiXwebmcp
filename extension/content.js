/**
 * Isolated-world content script. Reads the page's modelContext the same way
 * Chrome's model-context-tool-inspector does. Do not wrap registerTool.
 */

const LIST_DEBOUNCE_MS = 100;
let listTimer = 0;

/// True once the toolchange listener is attached, so it is attached only once.
let watching = false;

/// Runs we can still abort, keyed by the debugger's execution id.
const inFlight = new Map();

// The spec hangs the API on navigator, and real sites register there; early
// Chrome builds hung it on document. Both are checked, navigator first.
function modelContext() {
  if (
    typeof navigator !== "undefined" &&
    typeof navigator.modelContext === "object" &&
    navigator.modelContext !== null
  ) {
    return navigator.modelContext;
  }
  if (typeof document.modelContext === "object" && document.modelContext !== null) {
    return document.modelContext;
  }
  return null;
}

function hasModelContext() {
  return modelContext() !== null;
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

  const discovered = await modelContext().getTools();
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
  const context = modelContext();
  if (typeof context.addEventListener === "function") {
    // The documented API. Also non-destructive: the page keeps its own handler.
    if (!watching) {
      context.addEventListener("toolchange", debouncedListTools);
      watching = true;
    }
    return;
  }
  // Older Chrome only exposed the property. Assigning it clobbers whatever the
  // page installed, so it is the fallback, not the first choice.
  context.ontoolchange = debouncedListTools;
}

async function executeNamedTool(name, inputArgs, executionId) {
  if (!hasModelContext()) {
    throw new Error(
      'You must run Chrome with the "WebMCP for testing" flag enabled.'
    );
  }
  const context = modelContext();
  const tools = await context.getTools();
  const tool = tools.find((candidate) => candidate.name === name);
  if (!tool) {
    throw new Error("tool not found: " + name);
  }
  const args = inputArgs == null ? {} : inputArgs;

  // WebMCP passes an AbortSignal as execute's second argument. Where that is
  // supported this is a real abort; where it is not, cancelling only stops us
  // waiting, and the debugger says exactly that.
  const controller =
    typeof AbortController === "function" ? new AbortController() : null;
  if (executionId && controller) {
    inFlight.set(executionId, controller);
  }
  const options = controller ? { signal: controller.signal } : undefined;

  try {
    try {
      return await context.executeTool(tool, args, options);
    } catch (error) {
      const message = error && error.message ? String(error.message) : "";
      if (message.startsWith("Failed to parse input")) {
        return await context.executeTool(
          tool,
          JSON.stringify(args),
          options
        );
      }
      throw error;
    }
  } finally {
    if (executionId) {
      inFlight.delete(executionId);
    }
  }
}

function cancelExecution(executionId) {
  const controller = inFlight.get(executionId);
  if (!controller) {
    return false;
  }
  controller.abort();
  inFlight.delete(executionId);
  return true;
}

chrome.runtime.onMessage.addListener((message, _sender, reply) => {
  if (window !== window.top) {
    return;
  }
  if (message.action === "LIST_TOOLS") {
    watchTools();
    listTools()
      .then(() => reply({ ok: true }))
      .catch((error) => reply({ ok: false, error: String(error.message || error) }));
    return true;
  }
  if (message.action === "CANCEL_TOOL") {
    reply({ ok: cancelExecution(message.executionId) });
    return true;
  }
  if (message.action === "EXECUTE_TOOL") {
    executeNamedTool(message.name, message.arguments, message.executionId)
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
