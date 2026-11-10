# web

Browser demo: pick example files, parse them with `edi-parse` compiled to
WebAssembly, and view the JSON output.

- `wasm/`: a `cdylib` wrapper around the library (`parse_files`).
- `src/`: Vite + React + Tailwind app using shadcn/ui components
  (`src/components/ui`).
- `make-fs.mjs`: bundles `examples/x12`, `data/samples` and `data/invalid`
  into `public/fs.json`, the demo's file tree.
- `build.sh`: builds the wasm into `src/pkg`, writes `public/fs.json`, then
  runs `npm ci && npm run build` into `web/dist`. Installs rustup and
  `wasm-bindgen` if they're missing.

Build once (for the wasm and `fs.json`), then develop with hot reload:

    bash web/build.sh
    cd web && npm run dev

Vercel uses the repo-root `vercel.json`, which runs `web/build.sh` and serves
`web/dist`. No extra project settings are needed.
