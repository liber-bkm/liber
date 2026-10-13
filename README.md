# liber

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](https://www.gnu.org/licenses/gpl-3.0.html)

A local first bookmark manager that stores bookmarks as browsable plain HTML files with powerful search, tags, folders, and options to archive webpages for offline reading, create notes, and add file attachments for extra relevance. It classifies bookmarks automatically into tags and folders with automation rules. Use it on the command line (full CLI plus a terminal UI), in a browser (modern web UI), or as a native app (desktop and Android).

liber-rs is a Rust port of [liber](https://github.com/liber-bkm/liber) (Go). Port means capabilities, not clone: every Go feature is available in enhanced form, and the Rust design supersedes Go where the new stack allows better (UUID identity instead of numeric ids, tantivy full-text search instead of substring search, a builtin page archiver).

## Table of contents

- [Features](#features)
- [Install](#install)
- [Quickstart](#quickstart)
- [Addressing bookmarks and search syntax](#addressing-bookmarks-and-search-syntax)
- [Command line guide](#command-line-guide)
- [CLI reference](#cli-reference)
- [Terminal UI guide](#terminal-ui-guide)
- [Web UI guide](#web-ui-guide)
- [Desktop app](#desktop-app)
- [Android app](#android-app)
- [Self hosting](#self-hosting)
- [Configuration](#configuration)
- [Sync without loss](#sync-without-loss)
- [Storage layout](#storage-layout)
- [Troubleshooting](#troubleshooting)
- [Building from source](#building-from-source)
- [Architecture](#architecture)
- [Contributing](#contributing)
- [License](#license)

## Features

**Capture**

- One-command add with automatic title fetch, duplicate guard, notes,
  file attachments, and optional archived copy at creation
- Multi-URL paste: several URLs at once on the CLI and in the web add
  dialog, with a per-URL added/duplicate/failed report
- One-click capture without an extension via a bookmarklet (see
  [Web UI guide](#web-ui-guide))
- Inert page snapshots: stylesheets, fonts, images, and media embedded,
  scripts stripped, so archives can never phone home or run code
- Multiple archive backends: builtin snapshot, live browser DOM pipe,
  plus `single-file` and `monolith` wrappers with automatic fallback

**Organize**

- Tags and folders with rename, delete, and merge semantics; folder
  moves relocate files on disk
- Tag autocomplete and folder suggestions in the add and edit dialogs
- Automation rules that stick: host and title matching with a ledger, so
  manual moves are never overwritten; includes a learner that suggests
  rules from your library, plus per-rule apply with review
- Short numeric ids (`edit 3`) as sync-safe display aliases, full UUIDs
  behind `--uuid`

**Find**

- Instant search with relevance ranking, plus deep full-text search
  across saved page content and notes, with expandable highlighted
  match snippets (click to expand)
- Saved search presets: named `query plus scope plus sort` shortcuts in
  the sidebar with one-click apply (`liber searches`, API, Settings)
- Sort by newest (default), relevance, oldest, visited, or title
- Site icons in card and table views, with a Settings refresh action
- Fuzzy terminal picker with preview, and a `Ctrl+K` command palette

**Verify**

- Link checker (HEAD first, redirect aware) with safe bulk actions:
  update moved URLs, quarantine dead ones, optional title refetch
- Reindex repair: adopts orphan files, relinks siblings, quarantines
  conflicts, rebuilds search; missing files stay pending unless pruned

**Multiply**

- Every bookmark action runs on every frontend: CLI, web UI, desktop
  app over IPC, Android app over IPC or remote mode
- Multi-device sync with tombstones, tag union, and idempotent replay
- Library status in Settings: counts, health hints, last snapshot
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
`.tar.gz` file), so build it with `nix build .#release-bundle` and unpack
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

**Direct downloads (always the latest release):** asset filenames are
stable, so these links never change. Replace `latest` with
`download/vX.Y.Z` for a pinned version.

| Platform | Download |
|---|---|
| Windows installer | [liber-setup-x64.exe](https://github.com/liber-bkm/liber/releases/latest/download/liber-setup-x64.exe) |
| Windows portable | [liber-windows-x86_64.zip](https://github.com/liber-bkm/liber/releases/latest/download/liber-windows-x86_64.zip) |
| Linux x86_64 | [liber-linux-x86_64.tar.gz](https://github.com/liber-bkm/liber/releases/latest/download/liber-linux-x86_64.tar.gz) |
| Linux ARM64 | [liber-linux-aarch64.tar.gz](https://github.com/liber-bkm/liber/releases/latest/download/liber-linux-aarch64.tar.gz) |
| macOS (Apple Silicon and Intel) | [liber-darwin.dmg](https://github.com/liber-bkm/liber/releases/latest/download/liber-darwin.dmg) |
| macOS CLI tools | [liber-darwin.tar.gz](https://github.com/liber-bkm/liber/releases/latest/download/liber-darwin.tar.gz) |
| Android app | [liber-app-universal.apk](https://github.com/liber-bkm/liber/releases/latest/download/liber-app-universal.apk) |
| Android Play upload | [liber-app-universal.aab](https://github.com/liber-bkm/liber/releases/latest/download/liber-app-universal.aab) |

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

First run tip: `liber init` walks through base directory, device id,
archive backend, and auth token when nothing is configured yet.

## Addressing bookmarks and search syntax

These two concepts work identically on the CLI, the API, the web UI,
and the apps.

**Addressing.** Bookmarks carry stable UUIDs internally and short
numeric ids for humans. Every command that takes `<spec>` accepts:

- a short id: `edit 3`, `open 12`
- a UUID prefix: `open a1b2c3d4`
- comma lists and dash ranges: `edit 1-3,5`, `delete 2,4-6` (digit-only
  ranges expand; at most 1000 ids per range; reversed ranges are
  rejected)
- a search query as fallback: `open rust` opens the match, or lists
  matches when ambiguous

Ids are per-device display aliases, not identity: sync merges by UUID
and never renames your numbers. Gaps left by deletes stay until the
next sync import or a manual `reindex --prune --compact-ids` closes
them into dense `1..N`. A range hitting a gap fails and names the
missing id. `--uuid` anywhere prints full UUIDs instead of short ids.

**Search syntax.** Terms accept `field:value` prefixes (`title:`,
`url:`, `tag:` with `tags:` synonym, `folder:`, `desc:` with
`description:` synonym), combined with free words; every term must
match (AND). Quote multi-word values (`tag:"rust web"`). A bare word
searches all fields; `--in` on the CLI, the `scope` API parameter, and
the Library field selector restrict bare words to listed fields (names
or Go letters `n,u,t,d,f`). Prefixes always win for their own term.
There is no `content:` prefix by design: saved page content is searched
only for bare words under `--deep` (CLI/API) or the Deep toggle (web).
The same syntax works in `list`, `open`, the `pick` and web search
boxes, saved search presets, and Tauri/Android search.

Examples:

```sh
liber list tag:rust                      # everything tagged rust, newest first
liber list "title:async runtime" --in title,desc
liber list --sort visited --in url example.com
liber open "folder:recipes cake"         # one match opens, several list
```

## Command line guide

A typical session moves through capture, triage, verify, and sync:

```sh
liber add https://example.com/article -t reading --folder tech --archive
liber add -i https://example.com/other     # prompted: title, tags, folder, notes, attachments
liber list reading --sort newest
liber edit 3 --tags "reading rust" --folder tech
liber check --stale 7d --apply             # review moved/dead links
liber sync commit                          # snapshot when the base dir is a git repo
```

**Clever tricks worth knowing:**

- Bulk address everything: `edit 1-10 --folder inbox`, `delete 2,4-6 --yes`, `check 1-50 --apply`.
- `pick` is the fastest opener: `liber pick rust` prints the URL for
  scripts when piped, opens an interactive fuzzy finder on a terminal,
  and falls back to a numbered list elsewhere.
- `open` doubles as search: `liber open async` jumps straight to the
  single match.
- `add` takes several URLs at once: shared `--tags`, `--folder`, and
  `--archive` apply to all; single-URL flags (`--title`,
  `--description`, `--attach`, `-i`) are refused in bulk. Duplicates
  report as skipped, never prompt.
- `check --stale 1d12h` accepts combined durations (`ms/s/m/h/d/w`,
  bare digits mean hours); `--stale-hours` remains as a deprecated
  alias.
- `edit` with no flags opens the full terminal editor (see
  [Terminal UI guide](#terminal-ui-guide)).
- `liber --uuid` flips any listing to full UUIDs for scripting.
- `searches run daily` replays a saved preset through the normal list
  path.
- Shell completions: `liber completion bash >> ~/.bashrc` (also `zsh`,
  `fish`, `powershell`, `elvish`).
- Non-interactive use: every destructive command has a `--yes` escape
  hatch, and `pick` prints plain URLs when stdout is not a terminal.

## CLI reference

Global flags: `-l/--list` (legacy list), `-s/--serve` (legacy stub),
`--addr`, `--auth-token`, `--uuid`. Every command also accepts
`--uuid` and `-h/--help`. Reference generated from `liber --help`
(v0.11.0); when in doubt the binary is the truth.

**add [URL]...** Add one bookmark, or several at once.

```sh
liber add <url> [--title T] [-t tags...] [-f folder] [--description D]
  [--markdown] [--archive] [--attach file...] [-i]
liber add <url1> <url2> ... [-t tags] [-f folder] [--markdown] [--archive]
```

Single add fetches the page title when `--title` is missing and falls
back to the URL on failure. Duplicates print the existing entry and
ask before continuing. `-i/--interactive` prompts for everything in
order, starting with the URL itself (so `liber add -i` with no URL
works): duplicate pre-check, title, description, tags, folder,
markdown notes, archived copy, then loops an attachments
menu (`rm <num|name>` detaches, empty finishes); it needs a terminal
and refuses pipes. An empty URL answer cancels. Bulk add shares tags/folder/archive across all
URLs and prints `Added X, skipped Y duplicates, failed Z of N`.

**list [QUERY]** Search and list, newest first by default.

```sh
liber list [query] [--sort newest|oldest|visited|title] [--in FIELDS] [--deep]
```

`[md]`/`[arch]` markers flag notes and archived copies. `--deep`
unions saved-content matches with match fragments. An empty query
lists newest first; any query ranks by relevance unless `--sort`
overrides.

**open \<spec\>** Open in the system browser (or `browser_cmd`) and
record history with an open count. Falls back to search on miss.

**edit \<spec\>** Update fields in place.

```sh
liber edit <spec> [-u URL] [--title T] [--description D] [-t tags...]
  [-f folder] [--markdown] [--attach file] [--detach name|num]
```

Folder moves relocate files on disk; changing the URL re-checks the
duplicate guard. With no flags (and a terminal) it opens the full
screen editor instead.

**delete \<spec\> [--yes]** Delete with a confirm prompt (`--yes`
skips it). Files are removed and a tombstone enters the sync log.

**pick [QUERY]** Fuzzy finder with preview pane, then an action menu
(open, archived copy, notes, edit). One match skips interaction; piped
output prints the bare URL for scripts.

**attachments \<spec\>** List attachments with numbers for
`--detach`.

**archive \<spec\> [--backend builtin|browser|single-file|monolith|auto]**
Snapshot the page now. `auto` tries browser, then single-file and
monolith binaries, then the builtin, warning at each fallback.

**tags list|rename|delete, folders list|rename|delete** Taxonomy.
Rename merges onto an existing name instead of duplicating; folder
rename moves whole subtrees on disk; folder delete moves contents to
the root. Deletes go through the confirm flow.

**auto** Automation rules (`match` is `title:...`, `host:...`, else URL
substring, case insensitive).

```sh
liber auto add --match "host:example.com" --folder tech --tag rust [--tag ...]
liber auto list | edit <id> [--match ...] [--folder ...] [--tag ...] [--reapply]
liber auto apply [id] | learn [--min N] [--create] | delete <id>
```

Rules fill empty folders and union tags; manual moves stick thanks to
the ledger. `learn` suggests rules from host clusters (`--create`
asks per suggestion). Deleting a rule leaves classified bookmarks
untouched.

**history** Recently opened bookmarks with open counts.

**check [SPEC]** Link health.

```sh
liber check [spec] [--workers N] [--stale 7d] [--stale-hours N] [--apply]
```

HEAD first with redirect tracking: 404/410 is dead, 301/308 reports
moved with a resolved target, cross-host redirects and DNS failures
are uncertain (never auto-deleted). `--stale` limits the scan to
entries unchecked within the duration. `--apply` updates moved URLs
(skipping targets already bookmarked), refetches titles, and
quarantines dead ones; confirm-gated deletes ask first. On a terminal
each item prompts (moved: update plus retitle; dead/uncertain:
delete, quarantine, or skip); piped runs apply in bulk. DNS always
uses the system resolver.

**reindex [--prune] [--compact-ids]** Repair shop: adopts orphan HTML
by its own metadata, relinks sibling notes and archives, sweeps
conflict-named files to `unindexed/`, quarantines orphan attachments,
rebuilds the search index. Missing files stay pending unless
`--prune`; `--compact-ids` additionally needs `--prune` and closes
short-id gaps to dense `1..N`.

**export [OUT] [--site] [--bookmarks]** Static site (`--site`,
regenerable `index.html` with relative links, default dir) or a
Netscape bookmark file (`--bookmarks`).

**import \<FILE\> [--markdown] [--archive]** Netscape HTML import with
duplicate skip. `--archive` snapshots imported pages at creation,
failures reported as warnings.

**backup \<FILE\> / restore \<FILE\> [--force] [--yes]** Self-contained
`.tar.gz` (content plus database plus manifest; `site/` and the search
index excluded as regenerable, config excluded so identity stays
local). Restore refuses a non-empty library without `--force`, asks
unless `--yes`, merges over existing trees, then reindexes. No restart
needed on any frontend.

**sync** Multi-device exchange; see [Sync without loss](#sync-without-loss).

```sh
liber sync export <file> [--since N] | import <file>
liber sync push <url> [--token T] [--since N] | pull <url> [--token T] [--since N]
liber sync commit [--message T] [--push]
liber sync prune [--days N]        # default 90
```

**searches** Saved search presets (name plus query/scope/sort).

```sh
liber searches [list] | add <name> [--query Q] [--scope FIELDS] [--sort MODE]
liber searches delete <name> | run <name>
```

`run` lists through the normal search path. Adds upsert
case-insensitively; names cap at 64 chars; scope and sort validate
against the same parsers as `list`.

**config get|set|list** Read and write settings; see
[Configuration](#configuration). `list` prints the whole config as
JSON. `auth_token` is the one CLI-only extra key.

**profile list|switch|delete** Named libraries sharing one config.
`switch` creates and activates (live, no restart); `delete` untracks
the name only and refuses the active profile, leaving files on disk
for later re-adoption.

**init** First-run wizard (base dir, device id, backend, token).
Refuses when already configured; needs a terminal.

**serve** Serve the API plus web UI; see
[Self hosting](#self-hosting). Flags: `--addr` (default
`127.0.0.1:8080`), `--auth-token`, `--static-dir`, `--qr`, `--no-mdns`.

**discover [--timeout N]** Browse `_liber._tcp.local.` for servers and
print instance plus URL lines. Default timeout 5 seconds.

**completion \<shell\>** Print completions for `bash`, `zsh`, `fish`,
`powershell`, or `elvish`.

## Terminal UI guide

Two full-screen interfaces ship in the CLI, both closed with `Esc`
(and `Ctrl-C` anywhere bails out).

**Picker (`pick`, also `add -i` selection).** Type to fuzzy-filter the
list; `Up`/`Down` move, `Enter` confirms, `Esc` cancels. A preview
pane shows title, URL, description, tags, and folder. Confirming opens
the action menu: open in browser, open archived copy, open notes, or
edit. Without a terminal it degrades to a numbered list accepting
numbers or UUID prefixes.

**Editor (`edit` with no flags).** A ten-row form: title, URL,
description, tags, folder, markdown toggle, archive toggle, archive
backend cycle (`auto`, `builtin`, `browser`, `single-file`,
`monolith`), attach path, detach selector. `Tab` moves between rows,
`Space` toggles, `a`/`d` flip the artifact toggles, `Ctrl-S` saves,
`Esc` discards.

**Interactive add (`add -i`).** A prompt loop reusing the same line
helpers: URL first (empty cancels), duplicate pre-check, title
(fetched page title as default), description, space-separated tags,
folder, a markdown-notes confirm plus an archived-copy confirm, then
the normal create plus archive plus attach path, then an
attachments menu loop. Flag values seed the prompt defaults. Refuses
to run without a terminal instead of blocking on dead stdin.

## Web UI guide

One app for library, triage, and administration, served embedded with
zero flags in release and nix builds (`--static-dir` overrides for
development).

- **Library.** Instant search with `field:value` prefixes and a field
  selector, Deep toggle (saved page content and notes, slower;
  matches render as highlighted snippets, click to expand), saved
  search presets in the sidebar (save button beside the Deep toggle
  when a query is active), sort selector, card and table views
  (toggle button; table rows carry small favicons),
  multi-select with bulk bar, dark mode. Virtualized rendering keeps
  large libraries smooth; the fetch cap is 500 rows per query.
- **Detail drawer.** Tabs for details (inline edit with tag
  autocomplete and folder suggestions, two-step delete), notes
  (markdown editor), and archive (inline render or new-tab preference;
  archived copies open in the system browser). The header
  shows the full UUID; attachments upload per file and remove per
  file.
- **Add dialog.** Live duplicate detection (existing entry offered
  with Add anyway / Open existing), tag autocomplete chips plus folder
  dropdown, multi-URL paste (one per line) with a per-URL
  added/duplicate/failed report, optional notes, archive, and
  attachments at creation.
- **Bulk bar.** Appears on selection: confirm-gated delete, set tags,
  move folder.
- **Tags / Folders pages.** Counts, rename (merges onto existing),
  delete (folders move contents to root).
- **Rules page.** List with edit and delete, add form, apply per rule,
  learn suggestions with one-click create.
- **Check center.** Run with stale durations (`7d` style), per-row
  status, per-row apply buttons (update moved with retitle confirm,
  quarantine dead, confirm-gated delete).
- **History page.** Recently opened with counts.
- **Settings.** Library status first (device plus profile, counts,
  health hints, last snapshot), then archive backend, tools, library
  paths (saving switches immediately: move files first, reindex
  after), remote server (URL plus token for remote mode), profiles
  (switch creates, delete untracks only), sync snapshot (with push
  flag), maintenance (reindex with prune plus compact-ids), site
  icons (refresh missing or re-fetch cached), import/export plus full
  backup download and armed restore.
- **Command palette (`Ctrl+K`).** Jump to views, open bookmarks, start
  an add.

One-click capture without an extension via a bookmarklet. Create a
bookmark with this URL (change `LIBER_BASE` to match your `serve
--addr`, default `http://127.0.0.1:8080`, LAN address included):

```js
javascript:(function(){var B='http://127.0.0.1:8080';open(B+'/?add='+encodeURIComponent(location.href)+'&title='+encodeURIComponent(document.title),'_blank')})()
```

It opens the liber add dialog prefilled with the page URL and title
(plus optional `&tags=` and `&folder=`); tags and folder are typed in
the dialog. Token-gated servers reuse the login cookie, so log in
once first. The query form is shared with the Android deep link, so
`/?add=` links work everywhere the UI runs.

## Desktop app

The Tauri app calls the same core directly over IPC, no localhost hop,
so it works fully offline: every screen, rule, check, note, archive,
and attachment action, with files opened in the system browser. Remote
mode (server URL plus token in Settings) turns the same UI into a LAN
client. The system back button walks overlays first, then returns to
the library, then exits. Archived copies render in-app; attachments
download through the app. Install with `nix build .#liber-desktop` or
`tauri build` (`.deb`/`.rpm`/AppImage).

## Android app

The same app compiled for Android. On device it uses private storage,
forces the builtin archiver, and refuses git snapshots. Highlights:

- `liber://add?url=...` links open the add dialog prefilled
- Optional remote mode (server URL plus token in Settings) talks to a
  LAN `liber-serve` instead of the local library
- Phone layout with drawer navigation and notch-safe insets; back
  closes overlays, then returns to the library, then exits
- Archived copies open in the system browser from the archive tab

Every API feature needs its remote branch to work here; all shipped
features have one.

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

## Self hosting

```sh
liber serve --addr 127.0.0.1:8080
liber-serve --addr 0.0.0.0:8080 --auth-token <token>   # server only
```

`liber-serve` is the same server without CLI baggage; flags are
`--addr`, `--auth-token`, `--static-dir`, `--qr`, `--no-mdns`.
Serving order is always explicit `--static-dir`, then the embedded
bundle, then an API-only notice. Dev builds stay API-only unless
`--static-dir` points at a built UI.

**Pairing phones and tablets.** `serve --qr` prints a terminal QR of
the LAN URL next to the address lines; the server also advertises
`liber-<device-id>` over mDNS (`_liber._tcp.local.`, TXT carries
device id and API version) for its whole lifetime unless `--no-mdns`.
`liber discover` on another machine lists servers. The QR encodes the
URL only, never the token: scanning opens the login page, where the
token still gates entry. Browsers cannot browse mDNS, so the Settings
remote fields plus login page are the pairing endpoint there.

**Auth: one token model.** `--auth-token` or `LIBER_AUTH_TOKEN`;
empty means open. Browsers get an HMAC signed cookie after the login
form (POSTs additionally check Origin/Referer against Host, so a
foreign page cannot drive your browser session). API clients send
`Authorization: Bearer` with the *derived* bearer
`HMAC-SHA256(token, "liber-bearer-v1")` in hex (constant-time
compare); the web UI and Android remote mode derive it for you with
SubtleCrypto, and `sync push/pull` derive it from `--token` or
`LIBER_AUTH_TOKEN`. A 401 answers with the token hint instead of a
bare status. `auth_token` is CLI and env only and can never be set
over the API: a bearer that could mint itself is no auth at all.

**JSON API.** Lives under `/api/v2` (bookmarks, tags, folders, rules,
check, bulk, history, settings, sync, library, reindex, profiles,
searches, pick) with a checked-in `openapi.json` contract. The
bookmark list takes `q/sort/tag/folder/page/per_page/scope/deep`
(`scope` restricts bare words to fields; `q` also accepts
`field:value` prefixes). Adds answer 201, or 409
`{error: duplicate, existing, hint}` with `confirm_dup: true` retrying
to 200. Deletes, bulk deletes, tag deletes, check-apply deletes, and
restores answer `{confirm_required: true, ...}` until confirmed.
`POST /api/v2/sync/commit {push?}` snapshots the library directory
with git (always HTTP 200, errors carried in the body), and
`GET /api/v2/pick?q=` serves scripts as plain text: a single URL for
one match, else up to 30 `[id] title` rows (400 on missing query, 404
on no match).

**Bring your own TLS.** There is no in-app TLS by design; expose the
server through a tunnel or reverse proxy (Caddy, nginx, Tailscale
Serve, Cloudflare Tunnel) and point the apps at the `https://` URL.
Remote mode and the bookmarklet work unchanged over any base URL.

**LAN checklist.** Bind `0.0.0.0` (loopback is unreachable off-box),
open the port in the host firewall, confirm `discover` or the QR URL
from the client, then save server URL plus token in the client
Settings (remote mode) or log in through the browser.

## Configuration

**Locations.** The config file resolves to `LIBER_CONFIG` when set,
else the platform config dir:

| Platform | Config file | Default library base |
|---|---|---|
| Linux | `~/.config/liber-rs/config.json` | `~/.local/share/liber-rs` |
| macOS | `~/Library/Application Support/liber-rs/config.json` | `~/Library/Application Support/liber-rs` data dir |
| Windows | `%APPDATA%\liber-rs\config.json` | `%LOCALAPPDATA%\liber-rs` |
| Android | app private storage `config.json` | app private storage `library/` |

`LIBER_BASE_DIR` overrides the base dir without touching the file.
The file is written `0600` (owner only) since it can hold local paths.
`liber config list` prints the live config as JSON.

**Settable keys** (`liber config set <key> <value>`, Settings form, or
`PUT /api/v2/settings {key, value}` under one shared allowlist):

| Key | Meaning | Default |
|---|---|---|
| `base_dir` | Library root (switches live, no file moves) | platform data dir |
| `html_dir`, `markdown_dir`, `archive_dir`, `attachment_dir` | Per-kind overrides (`~` expands); empty means profile default, unsettable over the API | profile defaults |
| `device_id` | Shown in status and mDNS TXT | random UUID at first run |
| `archive_backend` | `builtin`, `browser`, `single-file`, `monolith`, `auto` | `builtin` (`auto` on first run) |
| `browser_cmd` | Command used by `open` | system opener |
| `browser_path` | Explicit browser for the browser archiver backend | empty (auto-detect Chromium family) |
| `editor_cmd` | Reserved for external editing | empty |
| `singlefile_cmd`, `singlefile_browser_path`, `monolith_cmd` | External archiver commands | bundled PATH entries under Nix |
| `auth_token` | CLI/env only, never returned or set over the API | empty (open) |

`monolith_browser_path`, `monolith_use_browser`, and `dns_fallback`
are Go-only keys with no Rust consumer. Unknown keys are rejected.
Changing a directory switches it live with no file moves: move your
files first, then run `reindex` so orphans are adopted; opening a
fresh dir lazily creates an empty library there.

**Profiles.** Named libraries sharing one config (`profile
list|switch|delete`, Settings profiles section, live in both
daemons). `default` is the base dir itself and is never tracked.
Switching creates the directory and activates immediately; deleting
untracks the name only and refuses the active profile, leaving files
on disk for later re-adoption.

**Saved searches.** Named `query plus scope plus sort` presets
(`searches list|add|delete|run`, API `/api/v2/searches`, sidebar
section). Stored in the config file as device-local UI state, never
synced; adds upsert case-insensitively, names cap at 64 chars.

**Daemon config is live, never snapshotted.** The server holds
`Arc<RwLock<Config>>` and Tauri a `Mutex<Config>` plus its config
path; profile switches and setting saves write through the lock and
the file together, so the running daemon serves the new state without
a restart (this also covers Android saves).

## Sync without loss

Devices share the base dir over any folder sync plus explicit bundle
exchange (`sync export` on A, `sync import` on B, or `sync push` /
`sync pull` directly against a `liber-serve` URL with derived-bearer
auth). Exchange files are JSON bundles; `--since` filters by oplog
sequence for incremental moves, and per-peer cursors remain a later
slice, so whole bundles are the norm. Newer `updated_at`
wins scalars, tags and attachments union, tombstones lose to newer
edits, replay is idempotent so bundles flow freely both ways, and the
originator marks its own entries applied so echoes never replay.
`sync commit [--message T] [--push]` snapshots with git (never inits
a repo; nothing-to-commit reports in the body), `sync prune`
(default 90 days) trims the log.

Short numeric ids stay local-only: every import closes gaps and folds
collisions into dense `1..N` in creation order, so converged devices
agree on the mapping. Anything
automation cannot place safely lands in `unindexed/` for review. It is
never deleted by automation; data loss is treated as the unforgivable
bug class. Notes save/remove and archive add/remove ride the bundle
as artifact ops with tolerant replay; history bumps and check stamps
skip the log by design, like attach/detach file bytes (metadata
converges, bytes follow via folder sync).

## Storage layout

Under the base dir (plus active profile): `html/`, `markdown/`,
`archive/`, `attachments/`, `.liber/store.db` (SQLite index plus
oplog), `.liber/tantivy/` (regenerable search index, excluded from
sync), `.liber/favicons/` (one raster icon per host, 64KB cap, fetched
lazily), `unindexed/` (quarantine), `site/` (export). Bookmark files
are named `<uuid>-<slug>`, recorded per row, never globbed. Slugs come
from the title at creation and never change on edit; every filename
passes the Windows reserved-name screen on all platforms.

## Troubleshooting

- **Search acts stale or empty after touching files by hand:** run
  `liber reindex` (or Settings, Maintenance, Run reindex). It adopts
  orphans, relinks siblings, sweeps conflicts to `unindexed/`, and
  rebuilds the search index. Add `--prune` only to drop rows whose
  files are gone.
- **Login loops or API 401s:** the server token comes from
  `--auth-token` or `LIBER_AUTH_TOKEN` at serve time; API clients must
  send the *derived* bearer, not the raw token. Remote mode derives it
  from the Settings token field; `sync push/pull` take `--token` or
  the env var.
- **Port already in use:** pick another `--addr`, e.g.
  `liber serve --addr 127.0.0.1:8081`, and update the bookmarklet base
  and remote URL to match.
- **Remote mode cannot reach the server:** bind `0.0.0.0` on the
  server, open the firewall port, and use the LAN address (not
  `localhost`) on the client. `liber discover` confirms visibility.
- **No icons after refresh:** icons cache with day-long headers;
  reload the library view after Settings, Site icons, Refresh icons.
- **Snippet looks stale:** snippets come from the search index, which
  updates on every write and rebuilds on reindex. Re-archive plus
  reindex refreshes them.
- **Restore refuses:** it guards non-empty libraries; pass `--force`
  (CLI) or arm the confirm (web) only when replacement is intended.
- **Check shows everything uncertain:** DNS failures and cross-host
  redirects classify uncertain by design and never auto-delete; the
  system resolver is the only resolver.

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

New files must be `git add`ed before any `nix build` (untracked files
are invisible to the sandbox). Frontend dependency changes additionally
need a `pnpmDeps.hash` refresh in `flake.nix`.

Without Nix: stable Rust via rustup (`rust-toolchain.toml` selects it),
Node 22 with pnpm, then the same component commands. Desktop needs the
webkit stack (Ubuntu/Debian: `libwebkit2gtk-4.1-dev libgtk-3-dev
libsoup-3.0-dev` plus build tools; Fedora and Arch equivalents in the
same family). Android needs a JDK, Gradle, and the SDK pieces listed
above.

Verify with `cargo test --workspace`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo fmt --check`, and
`cd frontend && pnpm install && pnpm build && pnpm test`.

## Architecture

Four thin crates over one domain core, plus the web UI:

- `liber-core`: all logic first (store, search, sync, archive, check,
  automation, taxonomy, import/export, backup, discovery, favicons).
  No frontend code may duplicate it.
- `liber-cli` (`liber`): clap commands plus ratatui TUI, thin over
  core. `serve` reuses the server crate.
- `liber-server` (`liber-serve`): axum JSON API under `/api/v2` plus
  the embedded/static web UI host. Per-resource contract tests plus a
  checked-in `openapi.json` drift test pin the API.
- `liber-tauri`: desktop and Android app calling core directly over
  IPC, with per-function REST fallback so remote mode reuses every
  feature.
- `frontend/`: Vite plus React plus TypeScript plus Tailwind plus
  TanStack. `tauri.ts` picks IPC or REST at runtime per function.

Maintainer internals live in `local/developer-docs.md`, the capability
tracker in `local/PARITY.md`, and the history of record in
`local/changelog.md`.

## Contributing

One slice at a time: core function first in `liber-core`, then
CLI/server/UI adapters, then tests, then the verify gates above, then
in the same change the PARITY checkbox, changelog entry, README plus
developer-docs updates for user-visible changes, and
SESSION-HANDOFF test counts when totals move. Keep all four ends
consistent (CLI/TUI, web UI, Tauri desktop IPC, Android app). No em
dashes in docs or code strings. Data loss is the unforgivable bug
class: quarantine, never delete user data. Auth follows the Go token
model exactly. Never commit unless asked; the maintainer commits
manually.

## License

GPL-3.0-or-later. See the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html).
