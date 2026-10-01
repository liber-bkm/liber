# liber-rs

Professional grade local first bookmark manager. CLI plus modern web UI plus
Tauri desktop and mobile. Rust port of liber (see parent directory) with
clean break storage; every Go capability is available in enhanced form,
not cloned.

Status: core complete (81 automated tests), web UI in progress. See
`local/PARITY.md` for the capability tracker.

## Install

Requires Nix (flake provides Rust, Node, SQLite) or a local Rust plus Node
toolchain.

```sh
cd project-liber
nix develop --command bash -c 'cargo build --release -p liber-cli'
./target/release/liber --help
```

Frontend:

```sh
cd frontend
nix develop --command bash -c 'pnpm install && pnpm build'
```

## Quickstart

```sh
liber add https://example.com -t reading --folder tech
liber list
liber open <uuid-prefix>
liber serve --addr 127.0.0.1:8080 --static-dir ./frontend/dist
```

Then open the printed address in a browser. The CLI and the served UI share
the same store: changes on either side appear on the other after a reload.
`serve --static-dir` points at the built frontend (`frontend/dist`).

## CLI

Bookmarks are addressed by UUID prefix (at least the leading characters
needed to be unique). Specs accept comma separated prefixes.

| Command | Purpose |
|---|---|
| `add <url>` | Add bookmark (`--title -t --folder --description --markdown --archive --attach`) |
| `list [query]` | Search and list (`--sort newest\|oldest\|visited\|title`) |
| `open <spec>` | Open in browser, records history (falls back to search on miss) |
| `edit <spec>` | Update url, title, description, tags, folder (`--markdown --attach --detach`) |
| `delete <spec>` | Delete with confirm (`--yes` skips it) |
| `attachments <spec>` | List attachments with numbers |
| `archive <spec>` | Archive page (`--backend builtin\|browser\|single-file\|monolith\|auto`) |
| `reindex [--prune]` | Repair: adopt orphans, relink siblings, sweep conflicts, rebuild search |
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
| `serve` | Self host API plus web UI (`--addr --auth-token --static-dir`) |

Planned but not yet implemented: `history`, `pick`, `profile`, `completion`.

## Web UI

Elegant library interface built from scratch (no Go UI carried over):

* Library with instant search, sort, card and table views, dark mode
* Detail drawer with inline edit, two-step delete, attachments, history
* Add dialog with live duplicate detection ("add anyway" or open existing)
* Bulk bar: multi-select delete (confirmed), set tags, move folder
* Tags and folders management with merge semantics
* Token login page; 401s redirect with `next` preserved

Remaining screens: rules with learn, check center, history, settings,
command palette. Served by `liber serve --static-dir`; the same bundle
targets Tauri desktop and mobile later.

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
