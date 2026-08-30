const WS_URL = "ws://127.0.0.1:17321";
const PROTOCOL_VERSION = 1;
const RECONNECT_MS = 1000;

let socket = null;
let reconnectTimer = 0;

function isoNow() {
  return new Date().toISOString();
}

function pageIdForTab(tabId) {
  return `tab:${tabId}`;
}

function parseTabId(pageId) {
  const match = /^tab:(\d+)$/.exec(String(pageId || ""));
  return match ? Number(match[1]) : null;
}

function sendEvent(event) {
  if (!socket || socket.readyState !== WebSocket.OPEN) {
    return;
  }
  socket.send(JSON.stringify(event));
}

async function requestTools(tabId) {
  try {
    await chrome.tabs.sendMessage(tabId, { action: "LIST_TOOLS" });
  } catch {
    // Content script may not be injected yet (chrome://, crashed tab, etc).
  }
}

async function emitActiveTab() {
  const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  if (!tab || tab.id == null) {
    return;
  }
  await requestTools(tab.id);
}

function connect() {
  if (
    socket &&
    (socket.readyState === WebSocket.CONNECTING || socket.readyState === WebSocket.OPEN)
  ) {
    return;
  }

  socket = new WebSocket(WS_URL);

  socket.addEventListener("open", () => {
    sendEvent({
      type: "hello",
      protocol_version: PROTOCOL_VERSION,
      timestamp: isoNow(),
    });
    emitActiveTab().catch(() => {});
  });

  socket.addEventListener("message", (event) => {
    let command;
    try {
      command = JSON.parse(event.data);
    } catch {
      return;
    }
    handleCommand(command).catch((error) => {
      console.warn("[WebMCP bridge] command failed", error);
    });
  });

  socket.addEventListener("close", () => {
    scheduleReconnect();
  });

  socket.addEventListener("error", () => {
    try {
      socket.close();
    } catch {
      // already closing
    }
  });
}

function scheduleReconnect() {
  socket = null;
  clearTimeout(reconnectTimer);
  reconnectTimer = setTimeout(connect, RECONNECT_MS);
}

async function handleCommand(command) {
  if (!command || typeof command !== "object") {
    return;
  }
  switch (command.type) {
    case "subscribe_page": {
      const tabId = parseTabId(command.page_id);
      if (tabId != null) {
        await requestTools(tabId);
      }
      return;
    }
    case "execute_tool": {
      await executeOnTab(command);
      return;
    }
    default:
      return;
  }
}

async function executeOnTab(command) {
  const tabId = parseTabId(command.page_id);
  const executionId = command.execution_id;
  const tool = command.tool;
  const args = command.arguments == null ? {} : command.arguments;
  const started = isoNow();
  const t0 = Date.now();

  sendEvent({
    type: "tool_execution_started",
    execution_id: executionId,
    tool,
    arguments: args,
    timestamp: started,
  });

  if (tabId == null) {
    sendEvent({
      type: "tool_execution_failed",
      execution_id: executionId,
      error: `unknown page_id: ${command.page_id}`,
      duration_ms: Date.now() - t0,
      timestamp: isoNow(),
    });
    return;
  }

  try {
    const response = await chrome.tabs.sendMessage(tabId, {
      action: "EXECUTE_TOOL",
      name: tool,
      arguments: args,
    });
    const durationMs = Date.now() - t0;
    if (response && response.ok) {
      sendEvent({
        type: "tool_execution_finished",
        execution_id: executionId,
        result: response.result == null ? null : response.result,
        duration_ms: durationMs,
        timestamp: isoNow(),
      });
      return;
    }
    sendEvent({
      type: "tool_execution_failed",
      execution_id: executionId,
      error: String((response && response.error) || "execute failed"),
      duration_ms: durationMs,
      timestamp: isoNow(),
    });
  } catch (error) {
    sendEvent({
      type: "tool_execution_failed",
      execution_id: executionId,
      error: String(error && error.message ? error.message : error),
      duration_ms: Date.now() - t0,
      timestamp: isoNow(),
    });
  }
}

chrome.runtime.onMessage.addListener((message, sender) => {
  const tab = sender.tab;
  if (!tab || tab.id == null) {
    return;
  }
  const page = {
    id: pageIdForTab(tab.id),
    url: message.url || tab.url || "",
    title: message.title || tab.title || "",
    origin: message.origin || "",
  };

  if (message.type === "content_script_ready" || message.type === "tools_unavailable") {
    sendEvent({
      type: "page_changed",
      page,
      timestamp: isoNow(),
    });
    if (message.type === "tools_unavailable") {
      sendEvent({
        type: "tools_changed",
        page_id: page.id,
        origin: page.origin,
        url: page.url,
        tools: [],
        timestamp: isoNow(),
      });
    }
    return;
  }

  if (message.type === "tools_changed") {
    sendEvent({
      type: "page_changed",
      page,
      timestamp: isoNow(),
    });
    sendEvent({
      type: "tools_changed",
      page_id: page.id,
      origin: page.origin,
      url: page.url,
      tools: Array.isArray(message.tools) ? message.tools : [],
      timestamp: isoNow(),
    });
  }
});

chrome.tabs.onActivated.addListener((info) => {
  requestTools(info.tabId).catch(() => {});
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.status === "complete") {
    requestTools(tabId).catch(() => {});
  }
});

connect();
