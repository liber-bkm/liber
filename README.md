# liber-rs

Professional grade local first bookmark manager. CLI plus modern web UI plus
Tauri desktop and mobile. Rust port of liber (see parent directory) with
clean break storage; every Go capability is available in enhanced form,
not cloned.

Status: core and web UI complete (111 Rust plus 2 UI tests), Tauri desktop
IPC batches 1 and 2 done (list/get/add/update/delete/open plus tags and
folders, each with REST fallback). See `local/PARITY.md` for the capability tracker.

## Install

Fastest path is Nix (flake provides the full toolchain):

```sh
cd project-liber
nix build .#default
./result/bin/liber --help
```

Desktop and all-in-one outputs are also flakes: `nix build .#liber-desktop`
(the Tauri app, binary plus `.desktop` entry and icons),
`nix run .#liber-desktop` to launch it, and `nix build .#release-bundle`
for a tarball with `liber`, `liber-serve`, and `liber-tauri` plus
checksums.

Without Nix, install the toolchain in `Building on plain Linux` below,
then build the components you want.

## Building with Nix

This is the primary path. Everything below runs from `project-liber/`;
prefix interactive commands with `nix develop --command` or enter the
shell once with `nix develop`.

* **Everything (CLI plus server plus UI, release)**: `nix build .#default`.
  Builds the frontend offline from the lockfile, then the release `liber`
  plus `liber-serve` binaries with the UI embedded. Output lands in
  `result/bin/`. Scoped to CLI plus server: the Tauri GUI binary is
  excluded (it has its own bundle pipeline below) via `cargoBuildFlags`,
  and tests via `cargoTestFlags`, so system webkit is never required.
* **CLI plus server (dev, fast, no UI)**: `cargo build -p liber-cli`.
  Serves API-only; `/` explains how to add the UI.
* **Web UI (dev)**: `cd frontend && pnpm install && pnpm build` once,
  then either `EMBED_UI=1 cargo build` to bake it in, or point any build
  at it with `liber serve --static-dir ./frontend/dist`. For UI iteration
  prefer `pnpm dev` (proxies `/api` to a local `liber serve`).
* **CLI plus server (release, local)**: build the frontend as above, then
  `EMBED_UI=1 cargo build --release -p liber-cli`. The binary serves the
  UI with zero flags. Without `EMBED_UI=1` the build stays API-only even
  in release mode, by design, so stale `dist` output never ships silently.
* **Tauri desktop app**: `nix build .#liber-desktop` installs the app
  natively (binary plus `.desktop` entry and icons, webkit wrapped).
  `nix run .#liber-desktop` launches it. For debugging, `cd
  crates/liber-tauri &&` run the frontend-local CLI
  (`../../frontend/node_modules/.bin/tauri`) `dev` (needs a display)
  or `cargo build -p liber-tauri` plus running the binary directly.
  Every app binary serves the embedded frontend bundle, never the Vite
  dev server: after changing UI code, run `pnpm build` in `frontend/`
  and rebuild the binary (the `generate_context!` embedding refreshes
  when the crate recompiles, so touch a Rust file if nothing else
  changed). For pure UI iteration prefer `pnpm dev` in a desktop
  browser against `liber serve` instead of relaunching the app.
  The raw `tauri build` produces `.deb`/`.rpm` (plus AppImage targets
  where configured). Frontend hook commands run with `crates/` as cwd.
  A display is needed for running, not for building.
* **All-in-one tarball**: `nix build .#release-bundle` packs `liber`,
  `liber-serve`, and `liber-tauri` with `SHA256SUMS` and install notes.
  The desktop binary inside is the unwrapped ELF (the `.desktop` install
  stays with `.#liber-desktop`); outside Nix it needs system webkit.

If `nix build .#default` fails with `ERR_PNPM_NO_OFFLINE_TARBALL`, the
`pnpmDeps.hash` in `flake.nix` is stale (frontend deps changed since it
was pinned). Fix: set the hash to `""`, rebuild, copy the `got: sha256-…`
value from the mismatch error back into the flake.

## Building on plain Linux

No Nix required. Install the toolchain, then the component commands are
identical to the Nix path above (cargo and pnpm fetch from the network
normally; the offline `pnpmDeps` pinning is a Nix-only concern).

Prerequisites: stable Rust via rustup (the repo's `rust-toolchain.toml`
selects the channel, plus rustfmt/clippy components) and Node 22 with
pnpm (`corepack enable` ships pnpm with Node 22, or install it
standalone). Only `liber` plus `liber-serve` need nothing else:
SQLite is bundled (rusqlite) and TLS uses rustls, so no system
libraries are required for the CLI and server.

The desktop app additionally needs the webkit system stack. Install one
block (CLI/server-only builders skip this):

* **Ubuntu/Debian**:
  `sudo apt install build-essential curl wget file pkg-config libssl-dev
  libdbus-1-dev libgtk-3-dev libsoup-3.0-dev libwebkit2gtk-4.1-dev
  libayatana-appindicator3-dev librsvg2-dev libxdo-dev patchelf`
* **Fedora**:
  `sudo dnf install gcc curl wget file pkg-config openssl-devel dbus-devel
  gtk3-devel libsoup3-devel webkit2gtk4.1-devel
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel patchelf`
* **Arch**:
  `sudo pacman -S base-devel curl wget file pkg-config openssl dbus gtk3
  libsoup3 webkit2gtk-4.1 libappindicator-gtk3 librsvg libxdo patchelf`

Then build from `project-liber/`:

```sh
rustup toolchain install stable   # honors rust-toolchain.toml
cargo build -p liber-cli          # CLI plus server, dev, API-only
cd frontend && pnpm install && pnpm build && cd ..
EMBED_UI=1 cargo build --release -p liber-cli                 # with UI baked in
cd crates/liber-tauri && ../../frontend/node_modules/.bin/tauri build  # desktop release
```

## Components

Four build outputs sharing one core (`crates/liber-core`, where every
feature lives first). They compile individually (`cargo build -p <crate>`)
but are not independent: the UI bundle and the API shapes flow into all
of them.

* **`liber` CLI** (`crates/liber-cli`, bin `liber`): the bookmark CLI
  (add, list, open, edit, pick TUI, tags, folders, rules, check, sync,
  history, profiles). Embeds `liber-server` as a library for its `serve`
  command, so one binary covers terminal and self-hosting use.
* **`liber-serve`** (`crates/liber-server/src/bin/liber-serve.rs`, bin
  `liber-serve`): a slim flags-only server (`--addr --auth-token
  --static-dir`) calling the same `serve` as `liber serve`. For machines
  that only self-host.
* **Web UI** (`frontend/`, Vite plus React): not a binary. Baked into
  `liber`/`liber-serve` via `rust-embed` when `EMBED_UI=1`, overridable
  per run with `--static-dir`, and bundled unchanged as the desktop
  frontend (`frontendDist` in `tauri.conf.json`). Serving order is always
  explicit `--static-dir`, then the embedded bundle, then an API-only
  notice.
* **Desktop app** (`crates/liber-tauri`, bin `liber-tauri`): Tauri 2
  shell calling `liber-core` directly over IPC (no localhost hop).
  Ships two ways: natively via `nix build .#liber-desktop` (binary plus
  `.desktop` entry and icons), or through its own bundle pipeline
  (`tauri build`) to `.deb`/`.rpm`/AppImage.

`nix build .#release-bundle` packs the CLI, server, and desktop
binaries into one tarball with checksums. Versioned releases with
installers stay a later step; until then, the flake outputs are the
distribution.

## Quickstart

```sh
liber add https://example.com -t reading --folder tech
liber list
liber open <uuid-prefix>
liber serve
```

Then open the printed address in a browser. `serve` ships the UI inside
the binary for release and nix builds; dev builds serve API-only unless
given `--static-dir` (see Building).

## CLI

Bookmarks are addressed by short numeric ID (`edit 3`) or UUID prefix.
Numbers are per-device display aliases assigned at creation in
`(created_at, uuid)` order; sync never renames them (UUIDs are the merge
key, incoming numbers are ignored for existing rows). Gaps from deletes
stay by default; `reindex --prune --compact-ids` closes them to a dense
`1..N` and resets the counter, so references like `[2]` can change meaning.
`--uuid` shows full UUIDs instead. Specs accept comma separated IDs.

| Command | Purpose |
|---|---|
| `add <url>` | Add bookmark (`--title -t --folder --description --markdown --archive --attach`) |
| `list [query]` | Search and list, newest first by default, relevance ranked when a query is given (`--sort newest\|oldest\|visited\|title` overrides) |
| `open <spec>` | Open in browser, records history (falls back to search on miss) |
| `edit <spec>` | Update url, title, description, tags, folder (`--markdown --attach --detach`); with no flags opens the full TUI editor |
| `pick [query]` | Fuzzy TUI picker, then an open-or-edit menu (prints the URL when piped) |
| `delete <spec>` | Delete with confirm (`--yes` skips it) |
| `attachments <spec>` | List attachments with numbers |
| `archive <spec>` | Archive page (`--backend builtin\|browser\|single-file\|monolith\|auto`) |
| `reindex [--prune --compact-ids]` | Repair: adopt orphans, relink siblings, sweep conflicts, rebuild search; `--compact-ids` (with `--prune`) closes short-id gaps to `1..N` |
| `export --site [dir]` | Static `index.html` with relative links |
| `export --bookmarks <file>` | Netscape bookmark file |
| `import <file>` | Netscape import (`--markdown --archive`), duplicates skipped |
| `tags list\|rename\|delete` | Rename merges onto existing tags |
| `folders list\|rename\|delete` | Rename moves subtrees, delete moves to root |
| `auto add --match X` | Rule with `--folder` and repeatable `--tag` |
| `auto list\|edit\|apply\|learn\|delete` | Learn suggests host rules (`--min N --create`) |
| `check [spec]` | Link check (`--workers N --stale-hours H --apply` updates moves, quarantines dead) |
| `sync export\|import` | Exchange oplog bundles between devices |
| `sync commit [--push]` | Git snapshot of the base dir (never inits a repo) |
| `sync prune [--days N]` | Drop oplog entries older than N days (default 90) |
| `config get\|set\|list` | See Config below |
| `history` | Recently opened with open counts |
| `profile list\|switch` | Named base-dir profiles sharing one config |
| `completion <shell>` | Shell completions (bash, zsh, fish, powershell, elvish) |
| `serve` | Self host API plus web UI (`--addr --auth-token --static-dir`) |

All CLI commands are implemented.

## Web UI

Elegant library interface built from scratch (no Go UI carried over):

* Library with instant search, sort, card and table views, dark mode
* Detail drawer with inline edit, two-step delete, attachments, history
* Add dialog with live duplicate detection ("add anyway" or open existing)
* Bulk bar: multi-select delete (confirmed), set tags, move folder
* Tags and folders management with merge semantics
* Rules with learn suggestions, check center with run plus per-row apply
* Real history page, settings (archive backends, reindex, import/export)
* Command palette (`Ctrl+K`): fuzzy search plus navigation and actions
* Token login page; 401s redirect with `next` preserved
* Detail content tabs: notes editor, archive viewer (inline or new tab),
  attachment upload

Served embedded with zero flags in release and nix builds; `--static-dir`
overrides for development. The same bundle targets Tauri desktop and
mobile.

## API

JSON API under `/api/v2` (OpenAPI contract tests pin the shapes):

* `GET /api/v2/bookmarks` list with `q/sort/tag/folder/page/per_page`
* `POST` add with 409 duplicate flow (`confirm_dup` accepts idempotently)
* `GET`, partial `PUT`, confirm-gated `DELETE` on `/bookmarks/{id}`
* `POST /bookmarks/{id}/open`, attachment download, tags, folders, rules
  (plus learn), check run/apply, bulk, library import/export, sync
  export/import/prune, reindex

Auth follows the Go token model: `--auth-token`/`LIBER_AUTH_TOKEN`, empty
means open. Browsers use an HMAC signed cookie plus Origin/Referer checks on
writes; API clients use `Authorization: Bearer` with the derived bearer
token (`HMAC(token, "liber-bearer-v1")` as hex). `/api/*` without
credentials returns JSON 401.

## Config

Lives at `~/.config/liber-rs/config.json`, overridable with `LIBER_CONFIG`
and `LIBER_BASE_DIR`. Settable keys: `base_dir`, `device_id`,
`archive_backend` (`builtin`, `browser`, `single-file`, `monolith`, `auto`),
`browser_path`, `singlefile_cmd`, `singlefile_browser_path`,
`monolith_cmd`, `browser_cmd`, `auth_token`.

Archiving defaults to the builtin snapshot (monolith-class static embed,
scripts stripped, no external processes). `auto` chains browser pipe (when
a browser resolves), single-file, monolith, then builtin, warning at each
fallback. Explicit backends are strict.

## Sync

Devices share the base dir over any folder sync plus explicit bundle
exchange:

```sh
liber sync export /tmp/bundle.json   # on device A, transfer the file
liber sync import /tmp/bundle.json   # on device B, merges with UUID semantics
liber sync commit                    # git snapshot of the base dir
```

Newer `updated_at` wins scalars, tags and attachments union, tombstones
lose to newer edits, rule patterns union. Replay is idempotent, so bundles
can be exchanged freely in both directions.

## Storage layout

Under the base dir (plus active profile): `html/`, `markdown/`,
`archive/`, `attachments/`, `.liber/store.db` (SQLite index plus oplog),
`.liber/tantivy/` (regenerable search index, excluded from sync),
`unindexed/` (quarantine, never deleted by automation), `site/` (export).
Bookmark files are named `<uuid>-<slug>`, recorded per row, never globbed.
