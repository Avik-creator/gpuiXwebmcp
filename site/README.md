# gpuiXwebmcp site

Explainer for the native WebMCP debugger. Palette is taken from the debugger's own tokens in `crates/debugger/src/theme.rs` — near-black ground `#0A0A0A`, warm bone ink `#E6E1D3`, rust `#C45C3A` for faults only. Keep the two in step; the site previously claimed a palette the app had already replaced.

```sh
cd site
npm install
npm run dev
```

Open [http://localhost:3000/](http://localhost:3000/). Routes: `/`, `/webmcp`, `/gpui`.

Other scripts: `npm run build`, `npm run preview`, `npm run generate-routes` (`tsr generate`).
