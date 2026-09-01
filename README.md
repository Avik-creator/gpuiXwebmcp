# gpuiXwebmcp

Native GPUI debugger for WebMCP-enabled Chrome tabs.

This is a local developer tool: it discovers page tools, lets you execute them manually, and records the execution trace. It is not an autonomous browser agent.

**Local-dev only.** Do not leave the extension loaded while browsing untrusted sites. Tool execution runs in the page as the logged-in user.

## Prerequisites

| | |
|---|---|
| macOS | The only platform this has been run on. `.cargo/config.toml` hard-codes a macOS path; Linux and Windows are untested. |
| **Xcode** — not just Command Line Tools | GPUI compiles Metal shaders with `xcrun metal`, which ships inside `Xcode.app`. With only CLT installed the build fails with an opaque shader error. `.cargo/config.toml` points `DEVELOPER_DIR` at `/Applications/Xcode.app/Contents/Developer`. |
| Rust stable | Pinned by `rust-toolchain.toml`. |
| **Chrome 150 or newer** | `extension/manifest.json` sets `minimum_chrome_version: 150`. On an older Chrome the extension loads but no tools are ever found. |
| Node | Only for the explainer site in `site/`. |

## Run it

**New here? Open the playground** — `⌃T`, or the link on the Tools screen. It runs a three-step walkthrough: pick a tool, run it, look at History. The bar says `Playground · sample data` with a LEAVE link the whole time it is on.

The playground provides its own subject. With Chrome connected it serves the bundled demo site on `127.0.0.1:5173` and opens it, so the walkthrough is a real round-trip against a real page. Without Chrome it falls back to built-in sample tools, so it works with no browser at all. **The demo server starts when you enter and stops when you leave** — it never holds the port for the rest of the session.

The app starts expecting Chrome, so a first run without the extension shows **Waiting for Chrome**.

1. Enable `chrome://flags/#enable-webmcp-testing` and restart Chrome.

2. Load the unpacked extension from `extension/` (`chrome://extensions` → Developer mode → Load unpacked). The id is pinned to `ffaihbpimepkgggjclheahfddigmmfeg` by the `key` field in the manifest.

3. Start the debugger:

```sh
cargo run -p debugger
```

4. Point it at a site, or press `⌃T` for the playground. The top right should read `Chrome connected`, and **Tools** should list whatever that page offers.

Do **not** also run `cargo run -p debugger --bin ws-server`; that binary is only for CLI checks and will collide on the port.

## Getting around

Three places, always named in the top-left of the bar. The current one is lit; click it or use the shortcut.

| | | |
|---|---|---|
| **Tools** | `⌘1` | What this site offers — every tool, what it does, and whether it can change things. |
| **Run** | `⌘2` | Fill in and run one tool. Dimmed until you pick one. |
| **History** | `⌘3` | What has happened. *Runs* for what you did, *All activity* for everything the browser sent. |

Run is one screen that becomes the run, then the result, then the failure — the tool name never moves.

`⌘K` opens the command palette: any tool, page or command by name. Everything below is also in there, so this table is a convenience, not the only way to find things.

| Key | |
|---|---|
| `⌘K` | Command palette |
| `⌘↵` | Execute |
| `⌘.` | Cancel the run |
| `⌘⇧C` | Copy the result |
| `⌘O` | Focus the site field |
| `⌘D` | Open the playground's demo site |
| `⌘[` / `⌘]` | Back / forward a screen |
| `esc` | Close the palette, or go back |
| `⌘⇧L` | Switch between dark and light |
| `⌃T` | Open or leave the playground |
| `⌘E` | History: runs, or all activity |
| `⇥` / `⇧⇥` | Next / previous form field |
| `↑` `↓` `↵` `esc` | Move, choose and close in the palette |

Text fields support the usual macOS editing: `⌘←`/`⌘→` line ends, `⌥←`/`⌥→` by word, `⌘⌫` delete to line start, `⌥⌫` delete the previous word, and the shift variants to select.

Cancel is `⌘.` rather than `⌃C` because `⌃C` is already Copy inside a text field.

## Tests

```sh
cargo test --workspace && node extension/content.test.mjs
```

The second one loads the content script the way Chrome does and runs it. A
syntax check does not catch an undeclared variable, and a content script that
throws on load looks exactly like a page with no tools — silent, with nothing in
the debugger to explain it.

## Troubleshooting

| Symptom | Cause |
|---|---|
| Build fails compiling Metal shaders | Xcode is not installed, or `xcode-select` points at Command Line Tools. See Prerequisites. |
| Top right stuck on **Waiting for Chrome** | Nothing is connected. Check the extension is loaded and enabled, that Chrome is 150+, and that the WebMCP flag is on. |
| Top right reads **2 browsers connected** | More than one browser is connected. Commands go to whichever connected first, so the other will not respond. |
| Top right reads **Port 17321 in use** | Another debugger (or a stray `ws-server`) already holds the port. Find it with `lsof -nP -iTCP:17321`. |
| Open says the page reported no tools | Chrome focused or opened the tab, but nothing came back within 12 seconds. The page probably does not use WebMCP, or the tab needs reloading so the content script runs. |
| Extension loaded, but it never leaves **Waiting for Chrome** | The extension id must match the origin allowlist. If the manifest's `key` field was removed, Chrome assigns a different id and the socket rejects the handshake with 403. |
| Tools never appear for a page | The page has no `document.modelContext`, or Chrome is older than 150. Reload the tab: the content script runs at `document_start`, so a tab opened before the extension was loaded never got it. |
| Everything was working, then stopped | The extension's service worker was evicted. The debugger pings every 20s to prevent that; if you changed `manifest.json` you must reload the extension for the `alarms` permission to take effect. |

## Explainer site

TanStack Router app in `site/`:

```sh
cd site
npm install
npm run dev
```

See [plan/webmcp-debugger.md](plan/webmcp-debugger.md).

### It's vibed.
