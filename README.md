# gpuiXwebmcp

Native GPUI debugger for WebMCP-enabled Chrome tabs.

This is a local developer tool: it discovers page tools, lets you execute them manually, and records the execution trace. It is not an autonomous browser agent.

**Local-dev only.** Do not leave the extension loaded while browsing untrusted sites. Tool execution runs in the page as the logged-in user.

## Phase 4 (current)

MV3 extension that bridges `document.modelContext` to a localhost WebSocket. GPUI is still fixture-only; this phase is the dumb pipe.

1. Enable `chrome://flags/#enable-webmcp-testing` and restart Chrome.

2. Load the unpacked extension from `extension/` (`chrome://extensions` → Developer mode → Load unpacked). The id is pinned to `ffaihbpimepkgggjclheahfddigmmfeg`.

3. Serve the demo site:

```sh
python3 -m http.server 5173 --bind 127.0.0.1 --directory demo-site
```

4. Start the WebSocket server (binds `127.0.0.1:17321` only; rejects any Origin that is not `chrome-extension://ffaihbpimepkgggjclheahfddigmmfeg`):

```sh
cargo run -p debugger --bin ws-server --no-default-features
```

5. Open [http://localhost:5173/](http://localhost:5173/). Watch events:

```sh
python3 scripts/watch_bridge.py
```

You should see `hello`, `page_changed`, and `tools_changed` with `get_user`, `search_products`, and `create_note`.

The GPUI inspector is still fixture Execute:

```sh
cargo run -p debugger
```

See [plan/webmcp-debugger.md](plan/webmcp-debugger.md) for Phase 5.

### It's vibed.
