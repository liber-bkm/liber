# liber

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](https://www.gnu.org/licenses/gpl-3.0.html)

A local first bookmark manager that stores bookmarks as browsable plain HTML files with powerful search, tags, folder and options to archive webpages for offline reading, create notes and add file attachments for additional relevance.It can classify bookmarks automatically into specific tags and folders based on automation rules. It can be used on command line with powerful TUI or as GUI using browser or desktop app. 

## Table of contents

- [Features](#features)
- [Install](#install)
- [Quickstart](#quickstart)
- [Using liber](#using-liber)
  - [Command line](#command-line)
  - [Web UI](#web-ui)
  - [Desktop app](#desktop-app)
  - [Android app](#android-app)
  - [Self hosting](#self-hosting)
- [Configuration](#configuration)
- [Sync without loss](#sync-without-loss)
- [Storage layout](#storage-layout)
- [Building from source](#building-from-source)
- [Architecture](#architecture)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgments](#acknowledgments)

## Features

**Capture**

- One-command add with automatic title fetch, duplicate guard, notes,
  file attachments, and optional archived copy at creation
- Inert page snapshots: stylesheets, fonts, images, and media embedded,
  scripts stripped, so archives can never phone home or run code
- Multiple archive backends: builtin snapshot, live browser DOM pipe,
  plus `single-file` and `monolith` wrappers with automatic fallback

**Organize**

- Tags and folders with rename, delete, and merge semantics; folder
  moves relocate files on disk
- Automation rules that stick: host and title matching with a ledger, so
  manual moves are never overwritten; includes a learner that suggests
  rules from your library
- Short numeric ids (`edit 3`) as sync-safe display aliases, full UUIDs
  behind `--uuid`

**Find**

- Instant search with relevance ranking, plus deep full-text search
  across saved page content and notes
- Sort by newest (default), relevance, oldest, visited, or title
- Fuzzy terminal picker with preview, and a `Ctrl+K` command palette

**Verify**

- Link checker (HEAD first, redirect aware) with safe bulk actions:
  update moved URLs, quarantine dead ones
- Reindex repair: adopts orphan files, relinks siblings, quarantines
  conflicts, rebuilds search; missing files stay pending unless pruned

**Multiply**

- Every bookmark action runs on every frontend: CLI, web UI, desktop
  app over IPC, Android app over IPC
- Multi-device sync with tombstones, tag union, and idempotent replay
- Self host the API plus web UI with token auth, or point the mobile
  app at your server in remote mode

## Install

**Nix (recommended, everything provided):**

```sh
cd project-liber
nix build .#default
./result/bin/liber --help
```

Desktop and bundles are flake outputs too: `nix build .#liber-desktop`
(native install with `.desktop` entry and icons),
`nix run .#liber-desktop` to launch, and `nix build .#release-bundle`
for a tarball of `liber`, `liber-serve`, and `liber-tauri` with
checksums.

**Any Linux distro (via Nix, no NixOS needed):** install Nix once per
machine, then the same flake outputs work everywhere:

```sh
sh <(curl -L https://nixos.org/nix/install) --daemon   # or --no-daemon for single-user
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf   # skip if your installer already enables flakes
nix profile install github:liber-bkm/liber#bundle      # liber, liber-serve, liber-tauri plus desktop entry
```

Updates are `nix profile upgrade` (or re-run install); pin a release
with a tag once tagged (`github:liber-bkm/liber/vX.Y.Z#bundle`). There
is no binary cache yet, so the first install compiles from source and
takes a while; later installs are instant. Native distro packages and
CI-built artifacts arrive via GitHub workflows at project completion.

**On NixOS via flake input** (all three programs in one entry):

```nix
inputs.liber-rs.url = "github:liber-bkm/liber";
inputs.liber-rs.inputs.nixpkgs.follows = "nixpkgs";
# ...
environment.systemPackages = [
  inputs.liber-rs.packages.${pkgs.stdenv.hostPlatform.system}.bundle
];
```

Prefer `.#default` in place of `.bundle` for CLI/server only. The
`release-bundle` tarball is not installable this way (its output is a
`.tar.gz` file) — build it with `nix build .#release-bundle` and unpack
manually instead.

**Packaged releases:** every `vX.Y.Z` tag builds all platforms in CI
(Linux x86_64 plus ARM64 tarballs with `.deb`, Windows zip plus NSIS
installer, macOS universal `.dmg` plus CLI tarball, signed Android APK
plus AAB) and attaches them to the GitHub release with checksums. The
macOS build is unsigned for now, so Gatekeeper needs a one-time
`xattr -d com.apple.quarantine` bypass. Arch Linux gets both a
`liber-bin` package (repacks the release tarball, no compiling) and a
`liber` source package under `dist/arch/`; refresh their checksums per
release with `dist/arch/bump-pkgbuild.sh <version>`.

**Without Nix:** install stable Rust (via rustup), Node 22 with pnpm,
then follow [Building from source](#building-from-source). Only the
desktop app needs system libraries (webkit stack); CLI and server build
on bare toolchains. Android needs a JDK, Gradle, and the SDK, see
[Android app](#android-app).

## Quickstart

```sh
liber add https://example.com -t reading --folder tech
liber list
liber open 1
liber serve
```

Open the printed address in a browser. That is the whole loop: capture
from anywhere, find instantly, open with history, and optionally serve
the library to your network.

Search terms accept `field:value` prefixes (`title:`, `url:`, `tag:`,
`folder:`, `desc:`, with `tags:`/`description:` synonyms), combined with
free words; every term must match. Quote multi-word values
(`tag:"rust web"`). A bare word searches all fields; `--in` on the CLI,
the `scope` API parameter, and the Library field selector restrict bare
words to listed fields (names or Go letters `n,u,t,d,f`). Prefixes always
win for their own term. The same syntax works in `list`, `open`, the
`pick` and web search boxes, and Tauri/Android search.

## Using liber

### Command line

Bookmarks are addressed by short numeric id (`edit 3`) or UUID prefix,
with comma lists and dash ranges (`edit 1-3,5`, at most 1000 ids per
range). Ids are per-device display aliases; sync merges by UUID and never
renames them. Gaps from deletes stay unless you compact them; a range
hitting a gap fails naming the missing id.

| Command | Purpose |
|---|---|
| `add <url>` | Add bookmark (`--title -t --folder --description --markdown --archive --attach`; `-i` prompts for missing fields, markdown notes, plus attachments, needs a terminal) |
| `list [query]` | Search and list, newest first (`--sort newest\|oldest\|visited\|title`, `--in title,url,tag,folder,desc`); `[md]`/`[arch]` markers |
| `open <spec>` | Open in browser, records history (falls back to search on miss) |
| `edit <spec>` | Update fields; with no flags opens the full terminal editor |
| `pick [query]` | Fuzzy picker, then open, archived copy, notes, or edit (prints the URL when piped) |
| `delete <spec>` | Delete with confirm (`--yes` skips it) |
| `attachments <spec>` | List attachments with numbers |
| `archive <spec>` | Archive page (`--backend builtin\|browser\|single-file\|monolith\|auto`) |
| `reindex [--prune --compact-ids]` | Repair library; `--compact-ids` closes id gaps to dense `1..N` |
| `export --site [dir]` / `--bookmarks <file>` | Static site / Netscape file |
| `import <file>` | Netscape import (`--markdown --archive`), duplicates skipped, archive failures reported as warnings |
| `tags list\|rename\|delete` | Rename merges onto existing tags |
| `folders list\|rename\|delete` | Rename moves subtrees, delete moves to root |
| `auto ...` | Rules: `add`, `list`, `edit`, `apply`, `learn`, `delete` |
| `check [spec]` | Link check (`--workers N --stale 7d --apply` for bulk update plus quarantine; TTY apply prompts per item with retitle and delete; DNS always uses the system resolver, no third-party fallback) |
| `sync export\|import` | Exchange merge bundles between devices |
| `sync commit [--push]` | Git snapshot of the base dir (never inits a repo) |
| `sync prune [--days N]` | Drop oplog entries older than N days (default 90) |
| `config get\|set\|list` | See [Configuration](#configuration) |
| `history` | Recently opened with open counts |
| `profile list\|switch\|delete` | Named base-dir profiles sharing one config (delete untracks only, files stay) |
| `completion <shell>` | Shell completions |
| `serve` | Self host API plus web UI; prints localhost and LAN URLs |

### Web UI

An interface built for the library rather than ported from anywhere:

- Library with instant search (`field:value` prefixes plus field selector), sort, card and table views, dark mode
- Detail drawer with inline edit, two-step delete, attachments, history
- Add dialog with live duplicate detection plus optional notes and archive
- Bulk bar, tags and folders management, rules with learn suggestions
- Check center with run plus per-row apply, history page, settings
  with sync snapshots and profile switching
- Command palette (`Ctrl+K`), token login, notes editor, archive viewer

Served embedded with zero flags in release and nix builds;
`--static-dir` overrides for development.

### Desktop app

The Tauri app calls the same core directly over IPC, no localhost hop,
so it works fully offline: every screen, rule, check, note, archive,
and attachment action, with files opened in the system browser. Install
with `nix build .#liber-desktop` or `tauri build` (`.deb`/`.rpm`/AppImage).

### Android app

The same app compiled for Android. On device it uses private storage,
forces the builtin archiver, and refuses git snapshots. Highlights:

- `liber://add?url=...` links open the add dialog prefilled
- Optional remote mode (server URL plus token in Settings) talks to a
  LAN `liber-serve` instead of the local library
- Phone layout with drawer navigation and notch-safe insets; back
  closes overlays, then returns to the library, then exits
- Archived copies open in the system browser from the archive tab

Build it yourself (device work is never automated):

```sh
nix develop .#android-fhs   # rustup, JDK 17, Gradle, SDK, NDK, licenses pre-accepted
rustup target add aarch64-linux-android
cd crates/liber-tauri
../../frontend/node_modules/.bin/tauri android init   # first time only
../../frontend/node_modules/.bin/tauri android build
adb install path/to/app.apk   # under gen/android/app/build/outputs/
```

Without Nix: JDK 17, Gradle and `android-tools` from your package
manager, then SDK `platform-tools`, `platforms;android-36`,
`build-tools;36.0.0` and an NDK via `sdkmanager`. `tauri android dev`
plus `adb reverse tcp:1420 tcp:1420` forwards the dev server for
iteration. Release APKs are unsigned: sign with `apksigner` before
installing (debug builds sign automatically).

### Self hosting

```sh
liber serve --addr 127.0.0.1:8080
liber-serve --addr 0.0.0.0:8080 --auth-token <token>   # server only
```

Serving order is always explicit `--static-dir`, then the embedded
bundle, then an API-only notice. JSON API lives under `/api/v2`
(bookmarks, tags, folders, rules, check, bulk, history, settings, sync,
library, reindex, profiles, pick) with a checked-in `openapi.json` contract. The
bookmark list takes `q/sort/tag/folder/page/per_page/scope` (`scope`
restricts bare words to fields; `q` also accepts `field:value`
prefixes, see Quickstart). Auth
follows one token model: empty means open; browsers use an HMAC signed
cookie, API clients use `Authorization: Bearer` with the derived bearer
token. `POST /api/v2/sync/commit {push?}` snapshots the library
directory with git (always HTTP 200, errors carried in the body), and
`GET /api/v2/pick?q=` serves scripts as plain text: a single URL for
one match, else up to 30 `[id] title` rows.

## Configuration

Lives at `~/.config/liber-rs/config.json`, overridable with
`LIBER_CONFIG` and `LIBER_BASE_DIR`. Settable keys: `base_dir`,
`html_dir`, `markdown_dir`, `archive_dir`, `attachment_dir`,
`device_id`, `archive_backend` (`builtin`, `browser`, `single-file`,
`monolith`, `auto`), `browser_cmd`, `browser_path`, `editor_cmd`,
`singlefile_cmd`, `singlefile_browser_path`, `monolith_cmd`.
`auth_token` is CLI and env only, never set over the API. Changing a
directory switches it immediately; move your files first, then run
`reindex` so orphans are adopted. `GET /api/v2/settings` also reports
`maintenance_status` (bookmark, oplog, quarantine, and short-id gap
counts) for the future sync status.

## Sync without loss

Devices share the base dir over any folder sync plus explicit bundle
exchange (`sync export` on A, `sync import` on B). Newer `updated_at`
wins scalars, tags and attachments union, tombstones lose to newer
edits, replay is idempotent so bundles flow freely both ways. Short
numeric ids stay local-only: every import closes gaps and folds
collisions into dense `1..N` in creation order, so converged devices
agree on the mapping. Anything
automation cannot place safely lands in `unindexed/` for review. It is
never deleted by automation; data loss is treated as the unforgivable
bug class.

## Storage layout

Under the base dir (plus active profile): `html/`, `markdown/`,
`archive/`, `attachments/`, `.liber/store.db` (SQLite index plus
oplog), `.liber/tantivy/` (regenerable search index, excluded from
sync), `unindexed/` (quarantine), `site/` (export). Bookmark files are
named `<uuid>-<slug>`, recorded per row, never globbed.

## Building from source

With Nix (primary path, from `project-liber/`, prefix with
`nix develop --command` or enter the shell once):

- `nix build .#default` - release CLI plus server with UI embedded
- `cargo build -p liber-cli` - fast dev build, API-only
- `cd frontend && pnpm install && pnpm build`, then `EMBED_UI=1 cargo
  build` to bake the UI in, or `liber serve --static-dir
  ./frontend/dist` to point at it (`pnpm dev` proxies `/api` for UI work)
- `nix build .#liber-desktop` / `nix run .#liber-desktop` - desktop app
- `nix build .#release-bundle` - all-in-one tarball with checksums

Without Nix: stable Rust via rustup (`rust-toolchain.toml` selects it),
Node 22 with pnpm, then the same component commands. Desktop needs the
webkit stack (Ubuntu/Debian: `libwebkit2gtk-4.1-dev libgtk-3-dev
libsoup-3.0-dev` plus build tools; Fedora and Arch equivalents in the
same family). Android needs a JDK, Gradle, and the SDK pieces listed
above.

Verify with `cargo test --workspace`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check`, and
`cd frontend && pnpm install && pnpm build && pnpm test`.

## License

GPL-3.0-or-later. See the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html).

