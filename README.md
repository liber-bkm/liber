# liber-rs

Professional grade local first bookmark manager. CLI plus modern web UI plus
Tauri desktop and mobile. Rust port of liber (see parent directory) with
clean break storage; every Go capability is available in enhanced form,
not cloned.

Status: core and web UI complete (102 Rust plus 2 UI tests), Tauri desktop
phase 1 done. See `local/PARITY.md` for the capability tracker.

## Install

Requires Nix (flake provides Rust, Node, SQLite) or a local Rust plus Node
toolchain.

```sh
cd project-liber
nix build .#default
./result/bin/liber --help
```

## Building

Everything builds from inside `nix develop` (run from `project-liber/`).
End users never build anything: release artifacts ship complete.

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
* **Nix package (`nix build .#default`)**: builds the frontend offline
  from the lockfile, then the release `liber` plus `liber-serve` binaries
  with the UI embedded. Scoped to CLI plus server only: the Tauri GUI
  binary is excluded (it has its own bundle pipeline below) via
  `cargoBuildFlags`, and tests via `cargoTestFlags`, so system webkit is
  never required.
* **Tauri desktop app**: `cd crates/liber-tauri &&` run the frontend-local
  CLI (`../../frontend/node_modules/.bin/tauri`) `build --debug` for an
  unoptimized bundle, or `build` for release. Produces `.deb`/`.rpm`
  (plus AppImage targets where configured) using the embedded frontend.
  Frontend hook commands run with `crates/` as cwd. Needs the flake dev
  shell for webkit system deps; a display for running, not for building.

Serving order is always explicit `--static-dir`, then the embedded
bundle, then an API-only notice.

If `nix build .#default` fails with `ERR_PNPM_NO_OFFLINE_TARBALL`, the
`pnpmDeps.hash` in `flake.nix` is stale (frontend deps changed since it
was pinned). Fix: set the hash to `""`, rebuild, copy the `got: sha256-…`
value from the mismatch error back into the flake.

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
