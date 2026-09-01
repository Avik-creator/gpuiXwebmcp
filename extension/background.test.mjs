/**
 * Load background.js the way Chrome does and drive its socket by hand.
 *
 *   node extension/background.test.mjs
 */

import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));

function listenerBucket() {
  const fns = [];
  return { addListener: (fn) => fns.push(fn), fns };
}

/** A service-worker world: a fake WebSocket, a fake chrome, captured timers. */
function makeEnvironment() {
  const sockets = [];
  const timers = [];
  const alarms = [];

  class FakeSocket {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;
    constructor(url) {
      this.url = url;
      this.readyState = FakeSocket.CONNECTING;
      this.listeners = {};
      this.sent = [];
      sockets.push(this);
    }
    addEventListener(name, fn) {
      (this.listeners[name] ||= []).push(fn);
    }
    send(data) {
      this.sent.push(JSON.parse(data));
    }
    close() {
      this.readyState = FakeSocket.CLOSED;
    }
    fire(name, event = {}) {
      for (const fn of this.listeners[name] || []) fn(event);
    }
  }

  const chrome = {
    runtime: {
      onMessage: listenerBucket(),
      onStartup: listenerBucket(),
      onInstalled: listenerBucket(),
    },
    tabs: {
      query: async () => [],
      sendMessage: async () => ({ ok: true }),
      update: async () => {},
      create: async () => {},
      onRemoved: listenerBucket(),
      onActivated: listenerBucket(),
      onUpdated: listenerBucket(),
    },
    windows: { update: async () => {} },
    alarms: {
      create: (name, info) => alarms.push({ name, info }),
      onAlarm: listenerBucket(),
    },
  };

  const context = createContext({
    WebSocket: FakeSocket,
    chrome,
    console,
    URL,
    setTimeout: (fn, ms) => {
      timers.push({ fn, ms });
      return timers.length;
    },
    clearTimeout() {},
  });

  const source = readFileSync(join(here, "background.js"), "utf8");
  runInContext(source, context, { filename: "background.js" });
  return { sockets, timers, alarms, chrome };
}

let failures = 0;
async function test(name, body) {
  try {
    await body();
    console.log(`  ok   ${name}`);
  } catch (error) {
    failures += 1;
    console.error(`  FAIL ${name}\n       ${error.message}`);
  }
}

console.log("background.js");

await test("loads, opens a socket and arms the keepalive alarm", async () => {
  const { sockets, alarms, chrome } = makeEnvironment();
  assert.equal(sockets.length, 1, "one socket on load");
  assert.equal(alarms.length, 1, "the keepalive alarm must be registered at top level");
  assert.equal(chrome.alarms.onAlarm.fns.length, 1, "and something must listen for it");
});

await test("a closed socket schedules a reconnect instead of throwing", async () => {
  // The regression: the close handler called an identifier that did not
  // exist, so the first drop threw and the bridge never came back.
  const { sockets, timers } = makeEnvironment();
  sockets[0].fire("close");
  const reconnect = timers.find((timer) => timer.ms === 1000);
  assert.ok(reconnect, "no reconnect timer was scheduled");
  reconnect.fn();
  assert.equal(sockets.length, 2, "firing the timer must open a fresh socket");
});

await test("a tab close is reported with a timestamp", async () => {
  // Without one the debugger could not parse it, and its page list only grew.
  const { sockets, chrome } = makeEnvironment();
  const socket = sockets[0];
  socket.readyState = 1;
  chrome.tabs.onRemoved.fns[0](7);
  const event = socket.sent.find((item) => item.type === "page_closed");
  assert.ok(event, "page_closed was never sent");
  assert.equal(event.page_id, "tab:7");
  assert.ok(!Number.isNaN(Date.parse(event.timestamp)), "timestamp must be a date");
});

await test("a command for an unknown page fails rather than hanging", async () => {
  const { sockets } = makeEnvironment();
  const socket = sockets[0];
  socket.readyState = 1;
  socket.fire("message", {
    data: JSON.stringify({
      type: "execute_tool",
      page_id: "nope",
      tool: "get_user",
      arguments: {},
      execution_id: "exec_1",
    }),
  });
  await new Promise((resolve) => setImmediate(resolve));
  const failed = socket.sent.find((item) => item.type === "tool_execution_failed");
  assert.ok(failed, "no failure was reported");
  assert.equal(failed.execution_id, "exec_1");
});

if (failures > 0) {
  console.error(`\n${failures} failing`);
  process.exit(1);
}
console.log("\nall passing");
