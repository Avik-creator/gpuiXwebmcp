# gpuiXwebmcp

Native GPUI debugger for WebMCP-enabled Chrome tabs.

This is a local developer tool: it discovers page tools, lets you execute them manually, and records the execution trace. It is not an autonomous browser agent.

**Local-dev only.** Do not leave the extension loaded while browsing untrusted sites. Tool execution runs in the page as the logged-in user.

## Phase 3 (current)

Vanilla demo site that registers the same three tools as the fixture backend.

```sh
python3 -m http.server 5173 --bind 127.0.0.1 --directory demo-site
```

Open [http://localhost:5173/](http://localhost:5173/) in Chrome with `chrome://flags/#enable-webmcp-testing` enabled. The official WebMCP inspector should list `get_user`, `search_products`, and `create_note`.

Without the flag, the page still loads and shows a banner.

The GPUI inspector (fixture Execute) is still:

```sh
cargo run -p debugger
```

See [plan/webmcp-debugger.md](plan/webmcp-debugger.md) for Phase 4.

### It's vibed.
