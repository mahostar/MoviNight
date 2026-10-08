# Development and verification

## Build and run

Install Node.js, Rust and the [native Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform. Install the Cargo Tauri CLI if missing: `cargo install tauri-cli --version '^2'`.

```bash
git clone https://github.com/mahostar/MoviNight.git
cd MoviNight
npm install
npm run check
npm run test:backend
cargo tauri dev
```

The frontend is plain HTML, CSS and JavaScript in `dist/`, embedded by Tauri. There is no separate frontend bundler or React application.

`npm run build` writes native bundles under `src-tauri/target/release/bundle/`. Windows builds produce NSIS and MSI packages. Other operating systems need native prerequisites and separate validation.

## Source layout

```text
dist/                       Embedded frontend HTML, JavaScript and CSS
src-tauri/src/
  lib.rs                    Commands, library records and seasons
  api.rs                    TMDB client and session response cache
  discovery.rs              Filters, compact records and cursors
  ai.rs                     Research, proposals, picks and approvals
  mcp.rs                    Loopback MCP transport and tools
  storage.rs                Atomic persistence and backups
  offline.rs                SQLite index, downloads and eviction
  snapshot.rs               ZIP transfer, validation and recovery
tests/                      Debug-WebView runtime and layout checks
branding/screenshots/v1.3.3/ Current UI captures
docs/                       Guides and architecture diagrams
MCP_GUIDE.md                 Client setup and agent workflows
SNAPSHOT_FORMAT.md           ZIP schema and conflict rules
```

## Existing checks

| Check | Purpose |
| --- | --- |
| `npm run check` | JavaScript syntax |
| `npm run test:backend` | Rust storage, cache, Discovery, MCP, approvals and snapshots |
| `npm run test:discovery-filter` | Incomplete filtering and pagination |
| `tests/runtime-check.cjs` | Research, review, library behavior and MCP |
| `tests/review-cards-check.cjs` | Card/detail review controls and layout |
| `tests/discovery-mcp-check.cjs` | Absorption, cursors, formats, filters and suggestions |
| `tests/spam-filter-runtime-check.cjs` | Incomplete filtering in the running WebView |
| `tests/snapshot-runtime-check.cjs` | Export, preview, merge, replace, research and thumbnails |
| `tests/responsive-check.cjs` | Responsive frontend layout |

Some scripts stage and approve records. Run mutating checks **only with an isolated debug data directory**. Set `MOVINIGHT_PLAYWRIGHT_PATH` to an installed Playwright module if it is not available as `playwright`.

## Runtime isolation

Before launching the debug executable, set:

```powershell
$env:MOVINIGHT_QA_DATA_DIR = 'E:\projects\MoviNight\.qa-data\my-check'
$env:WEBVIEW2_USER_DATA_FOLDER = 'E:\projects\MoviNight\.qa-data\my-webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9229'
```

Place sample library files and a local `config.json` in the isolated directory. Never print or commit its key. Runtime/responsive checks generally use **9229**; snapshot checks use **9233** by default. Inspect each script before launching it.

`MOVINIGHT_QA_DATA_DIR` is debug-only. **Release builds ignore it and use real saved data.** Do not use a release executable for mutating fixture checks.

Snapshot checks offer `--prepare`. Debug-only `MOVINIGHT_QA_SNAPSHOT_EXPORT` and `MOVINIGHT_QA_SNAPSHOT_IMPORT` select fixture ZIP paths without native pickers; release builds ignore these overrides.

## Documentation captures

The current gallery uses the actual 1.3.3 debug WebView, a separate data directory and profile, canonical TMDB metadata and labeled demonstration records. It does not replace app DOM content with mock cards or publish personal history.

Keep images in a versioned directory, inspect the rendered PNGs, check visible artwork loaded, and validate links and Mermaid syntax. Update published-installer wording only after a release exists. See [capture notes](screenshots.md).
