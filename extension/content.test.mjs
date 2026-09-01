/**
 * Load content.js the way Chrome does and check it actually works.
 *
 * This exists because a content script that throws on load looks exactly like a
 * page with no tools: silent, with nothing in the debugger to explain it. A
 * syntax check does not catch an undeclared variable; running the thing does.
 *
 *   node extension/content.test.mjs
 */

import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));

const DEMO_TOOLS = [
  { name: "get_user", description: "Get user", inputSchema: { type: "object" }, annotations: { readOnlyHint: true } },
  { name: "search_products", description: "Search", inputSchema: { type: "object" }, annotations: { readOnlyHint: true } },
  { name: "create_note", description: "Note", inputSchema: { type: "object" }, annotations: { readOnlyHint: false } },
];

/** A page that supports WebMCP and has registered the demo tools. */
function makeEnvironment({ withModelContext = true, executeTool } = {}) {
  const sent = [];
  const listeners = [];
  const toolchange = [];

  const modelContext = {
    getTools: async () => DEMO_TOOLS,
    executeTool: executeTool ?? (async () => ({ ok: true })),
    addEventListener: (name, handler) => toolchange.push({ name, handler }),
  };

  const documentStub = {
    readyState: "complete",
    title: "WebMCP demo",
    addEventListener() {},
    ...(withModelContext ? { modelContext } : {}),
  };

  const windowStub = {
    location: { href: "http://127.0.0.1:5173/", origin: "http://127.0.0.1:5173" },
  };
  windowStub.top = windowStub;

  const context = createContext({
    window: windowStub,
    document: documentStub,
    location: { href: "http://127.0.0.1:5173/", origin: "http://127.0.0.1:5173" },
    console,
    setTimeout,
    clearTimeout,
    chrome: {
      runtime: {
        sendMessage: async (message) => {
          sent.push(message);
        },
        onMessage: { addListener: (fn) => listeners.push(fn) },
      },
    },
  });

  const source = readFileSync(join(here, "content.js"), "utf8");
  runInContext(source, context, { filename: "content.js" });
  return { sent, listeners, toolchange, modelContext };
}

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Objects built inside the vm come from another realm, so deepEqual compares
// prototypes and fails on structurally identical values. Compare the shape.
const same = (actual, expected, message) =>
  assert.equal(JSON.stringify(actual), JSON.stringify(expected), message);

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

console.log("content.js");

await test("loads and announces itself without throwing", async () => {
  // The regression this file was written for: an undeclared variable in
  // watchTools threw on load, so boot never ran and the page looked toolless.
  const { sent } = makeEnvironment();
  assert.ok(
    sent.some((message) => message.type === "content_script_ready"),
    "content_script_ready was never sent, so the debugger never learns the page exists"
  );
});

await test("reports the page's registered tools", async () => {
  const { sent } = makeEnvironment();
  await wait(200); // the list is debounced
  const message = sent.find((item) => item.type === "tools_changed");
  assert.ok(message, "no tools_changed was ever sent");
  assert.equal(message.tools.length, 3);
  same(
    message.tools.map((tool) => tool.name),
    ["get_user", "search_products", "create_note"]
  );
});

await test("normalises annotations to the wire's snake_case", async () => {
  const { sent } = makeEnvironment();
  await wait(200);
  const message = sent.find((item) => item.type === "tools_changed");
  assert.equal(message.tools[0].annotations.read_only_hint, true);
  assert.equal(message.tools[0].input_schema.type, "object");
});

await test("subscribes to toolchange with addEventListener, not the property", async () => {
  const { toolchange, modelContext } = makeEnvironment();
  assert.equal(toolchange.length, 1, "expected exactly one listener");
  assert.equal(toolchange[0].name, "toolchange");
  assert.equal(
    typeof modelContext.ontoolchange,
    "undefined",
    "the property assignment would clobber the page's own handler"
  );
});

await test("a page without WebMCP says so instead of failing silently", async () => {
  const { sent } = makeEnvironment({ withModelContext: false });
  await wait(200);
  assert.ok(
    sent.some((message) => message.type === "tools_unavailable"),
    "the debugger must be told the page has no modelContext"
  );
});

await test("answers LIST_TOOLS, EXECUTE_TOOL and CANCEL_TOOL", async () => {
  const { listeners } = makeEnvironment();
  assert.equal(listeners.length, 1, "expected one message listener");
  const listener = listeners[0];

  const answer = (message) =>
    new Promise((resolve) => {
      const kept = listener(message, {}, resolve);
      assert.equal(kept, true, `${message.action} must keep the reply channel open`);
    });

  same(await answer({ action: "LIST_TOOLS" }), { ok: true });

  const executed = await answer({
    action: "EXECUTE_TOOL",
    name: "get_user",
    arguments: {},
    executionId: "exec_1",
  });
  assert.equal(executed.ok, true);

  // Nothing is in flight by then, so cancel truthfully reports that.
  same(await answer({ action: "CANCEL_TOOL", executionId: "exec_1" }), { ok: false });
});

await test("an unknown tool fails with a message rather than hanging", async () => {
  const { listeners } = makeEnvironment();
  const result = await new Promise((resolve) => {
    listeners[0]({ action: "EXECUTE_TOOL", name: "nope", arguments: {} }, {}, resolve);
  });
  assert.equal(result.ok, false);
  assert.match(result.error, /tool not found/);
});

await test("a tool that throws reports the page's own message", async () => {
  const { listeners } = makeEnvironment({
    executeTool: async () => {
      throw new Error("text is required");
    },
  });
  const result = await new Promise((resolve) => {
    listeners[0]({ action: "EXECUTE_TOOL", name: "create_note", arguments: {} }, {}, resolve);
  });
  assert.equal(result.ok, false);
  assert.equal(result.error, "text is required");
});

if (failures > 0) {
  console.error(`\n${failures} failing`);
  process.exit(1);
}
console.log("\nall passing");
