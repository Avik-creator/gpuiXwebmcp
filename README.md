# gpuiXwebmcp

Native GPUI debugger for WebMCP-enabled Chrome tabs.

This is a local developer tool: it discovers page tools, lets you execute them manually, and records the execution trace. It is not an autonomous browser agent.

**Local-dev only.** Do not leave the extension loaded while browsing untrusted sites. Tool execution runs in the page as the logged-in user.

## Phase 5 (current)

The GPUI window listens on `ws://127.0.0.1:17321` (loopback only). The MV3 extension is a dumb bridge from `document.modelContext` to that socket. Paste a site URL in the header field and press GO (or Enter) to focus that tab or open it in Chrome. Execute in the inspector hits the real tab.

Do **not** also run `cargo run -p debugger --bin ws-server`; that binary is only for Phase 4 CLI checks and will collide on the port.

1. Enable `chrome://flags/#enable-webmcp-testing` and restart Chrome.

2. Load the unpacked extension from `extension/` (`chrome://extensions` → Developer mode → Load unpacked). The id is pinned to `ffaihbpimepkgggjclheahfddigmmfeg`.

3. Serve the demo site:

```sh
python3 -m http.server 5173 --bind 127.0.0.1 --directory demo-site
```

4. Start the debugger (owns the WebSocket server):

```sh
cargo run -p debugger
```

5. Open [http://localhost:5173/](http://localhost:5173/). The status pill should read **Connected**, the page origin should be visible, and Execute on `search_products` should return the same two books as Fixture mode.

Click the status pill to toggle **Fixture** (in-process demo, no Chrome) vs live Chrome.

See [plan/webmcp-debugger.md](plan/webmcp-debugger.md).

## Explainer site

TanStack Router app in `site/`, same colors as the GPUI window:

```sh
cd site
npm install
npm run dev
```

### It's vibed.
