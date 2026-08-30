# gpuiXwebmcp

Native GPUI debugger for WebMCP-enabled Chrome tabs.

This is a local developer tool: it discovers page tools, lets you execute them manually, and records the execution trace. It is not an autonomous browser agent.

**Local-dev only.** Do not leave the extension loaded while browsing untrusted sites. Tool execution runs in the page as the logged-in user.

## Phase 2 (current)

Four-panel inspector driven by a fixture backend. Fill primitive schema fields and Execute in-process. No Chrome, no network.

```sh
cargo run -p debugger
```

Select `search_products`, type `gpui` in the query field, click Execute. The inspector should show `{ "results": [...] }` and the event log should append started/finished.

**Not verified yet.** GPUI compile needs the Xcode Metal toolchain. Pickup steps are under Phase 2 in [plan/webmcp-debugger.md](plan/webmcp-debugger.md).

### It's vibed.