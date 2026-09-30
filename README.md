# liber-rs

Professional grade local first bookmark manager. CLI plus modern web UI plus
Tauri desktop and mobile. Rust port of liber (see parent directory), with
clean break storage and full feature parity as the goal.

Status: early scaffold. See `local/PARITY.md` for what works today.

## Install

Requires Nix (flake provides Rust, Node, SQLite) or a local Rust plus Node
toolchain.

```sh
cd project-liber
nix develop --command bash -c 'cargo build --release -p liber-cli'
./../target/release/liber --help
```

Frontend:

```sh
cd frontend && pnpm install && pnpm build
```

## Quickstart

```sh
liber add https://example.com -t reading --folder tech
liber list tech
liber open <uuid-prefix>
liber serve --addr 127.0.0.1:8080
```

Then open the printed address, or run the Tauri app once packaged.

## CLI

| Command | Purpose |
|---|---|
| `add <url>` | Add bookmark (`--title --tags --folder --description --markdown --archive`) |
| `list [query]` | Search and list |
| `open <spec>` | Open in browser, records history |
| `edit <spec>` | Update url, title, tags, folder |
| `delete <spec>` | Delete (use `--yes` to skip confirm) |
| `reindex` | Rebuild index (`--merge --prune --compact`) |
| `export` | `--site` static HTML or `--bookmarks` Netscape file |
| `import <file>` | Netscape import, `--from-go` migrates a Go liber dir |
| `tags` | `list`, `rename`, `delete` |
| `folders` | `list`, `rename`, `delete` |
| `auto` | `add`, `list`, `apply`, `learn`, `delete` rules |
| `history` | Recently opened |
| `check` | Link check, `--apply` to act |
| `sync` | Git snapshot sync, `--merge` to replay oplog |
| `config` | `get`, `set`, `list` |
| `pick` | Interactive nucleo picker |
| `profile` | `list`, `switch` |
| `serve` | Self host API plus web UI |
| `completion` | Shell completions |

## Config

Lives at `~/.config/liber-rs/config.json`, overridable with `LIBER_CONFIG`
and `LIBER_BASE_DIR`. Key `archive_backend` selects `builtin` (default),
`monolith`, or `single-file`. Key `auth_token` or env `LIBER_AUTH_TOKEN`
protects `serve`.

## Import from Go liber

```sh
liber import --from-go ~/.config/liber/base-dir
```

Reads old `index.json` plus files, writes new UUID store. Old dir is never
modified.

## Sync

Point two devices at the same synced folder (Syncthing, Nextcloud, git) and
run `liber sync --merge` on each. Same merge rules as Go liber, minus the
numeric ID renames.

## Web UI and mobile

`liber serve` hosts the React UI for any browser. Tauri desktop bundles the
same UI with direct core IPC. Tauri Mobile targets Android with the same
codebase. Mobile builds need `nix develop .#android-fhs`, see `local/AGENTS.md`.
