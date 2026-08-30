# gpuiXwebmcp

Native GPUI debugger for WebMCP-enabled Chrome tabs.

This is a local developer tool: it discovers page tools, lets you execute them manually, and records the execution trace. It is not an autonomous browser agent.

**Local-dev only.** Do not leave the extension loaded while browsing untrusted sites. Tool execution runs in the page as the logged-in user.

## Phase 1 (current)

Four-panel inspector driven by a fixture backend. No Chrome, no network.

```sh
cargo run -p debugger
```

Click a tool on the left-middle list. The inspector schema should change. Execute is Phase 2.

See [plan/webmcp-debugger.md](plan/webmcp-debugger.md) for the rest of the build.

### It's vibed.