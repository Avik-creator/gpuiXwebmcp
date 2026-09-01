# gpuiXwebmcp site

Explainer for the native WebMCP debugger. Drawn with the debugger's own tokens from `crates/debugger/src/theme.rs`: paper `#0A0A0A`, ink `#E6E1D3`, mute `#8A8577`, hair `#2A2724`, and rust `#C45C3A` for faults only, all in the same monospace face and the same three type sizes. Keep the two in step.

```sh
cd site
npm install
npm run dev
```

Open [http://localhost:3000/](http://localhost:3000/). Routes: `/`, `/webmcp`, `/gpui`, `/try`. The last one is a WebMCP host of its own: paste a tool, it registers on `navigator.modelContext`, and the debugger lists it like any other site. It also checks other sites: `GET /api/probe?url=…` (served by `probe-plugin.ts` on the dev and preview servers only) reads a site's page and same-origin scripts and reports whether they reference WebMCP. It cannot run the page, so the tool names it spots are unverified; the debugger is where the real list is.

Other scripts: `npm run build`, `npm run preview`, `npm run generate-routes` (`tsr generate`).
