# liber

[![License: GPL-3.0-or-later](https://img.shields.io/badge/License-GPL--3.0--or--later-blue.svg)](https://www.gnu.org/licenses/gpl-3.0.html)

A local first bookmark manager that stores bookmarks as browsable plain HTML files with powerful search, tags, folders, and options to archive webpages for offline reading, create notes, and add file attachments for extra relevance. It classifies bookmarks automatically into tags and folders with automation rules. Use it on the command line (full CLI plus a terminal UI), in a browser (modern web UI), or as a native app (desktop and Android).

>[!TIP]
>This Readme is simple and conscise, for detailed guide and documentation visit [liber-bkm.github.io](https://liber-bkm.github.io)

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

- Plain app aganostic HTML bookmarks 
- Automatic fetch titles and descriptions 
- Web page archives, notes, and file attachments
- Third party archive backend support. [single-file](https://github.com/gildas-lormeau/single-file-cli) and [monolith](https://github.com/y2z/monolith)
- Tags and directories 
- Instant search with relevance ranking, plus deep full-text search
  across saved page content and notes
- Fully functional through CLI and GUI
- Highly configurable 
- Bulk actions support, add multiple bookmarks, edit tags and folders at once
- Duplicate detection
- Available for mac, windows, linux and android
- Check for stale, or changed URLS
- Import and export bookmarks
- rules to automatically classify bookmarks into specific tags or folders
- Self hosting and sync friendly
- Merge conflict resolution with third party sync (Drive, NextCloud, Syncthing)
- Profile for independant collections
- One-click capture without an extension via a bookmarklet (see
  [Web UI guide](#web-ui-guide)

## Install

### Dependencies

Liber has no dependencies, however it allows using third party backends for archiving webpages [single-file](https://github.com/gildas-lormeau/single-file-cli) or [monolith](https://github.com/y2z/monolith). This allows including javascript content within the archived pages, liber by default disables javascript from the archived pages. Optionally `git` is required for users who want to sync using git.

### Appimage 

You can download the appimage for `x86_64` devices from [here](https://github.com/liber-bkm/liber/releases/download/v0.11.1/liber_0.11.1_amd64.AppImage) and for`arm64` devcies from [here](https://github.com/liber-bkm/liber/releases/download/v0.11.1/liber_0.11.1_aarch64.AppImage)

### Archlinux

``` sh
wget https://github.com/liber-bkm/liber/blob/main/dist/arch/liber-bin/PKGBUILD
makepkg -si
```

### NixOS 

Add the repo into the flake inputs

``` nix
inputs.liber = {
    url = "github:liber-bkm/liber";
    inputs.nixpkg.follows = "nixpkgs";
};
```

And install the package either using `environment.systemPackages` or using home-manager `home.packages`

``` nix
{inputs, pkgs, ... }:
{
    environment.systemPackages = [ inputs.liber.packages.${pkgs.stdenv.hostPlatform.system}.bundle ]; # you can relapce .bundle with .default or .desktop if you want cli or desktop versions only 
}
```

### Debian based Distros (Debian, Ubuntu, Mint, ... )

Download and install the `.deb` package:

- For x86_64 devices [Download](https://github.com/liber-bkm/liber/releases/download/v0.11.1/liber_0.11.1_amd64.deb)
- For arm64 devices [Download](https://github.com/liber-bkm/liber/releases/download/v0.11.1/liber_0.11.1_arm64.deb)

### RPM based distros (Fedora, OpenSUSE, ... )

Download and install the `.rpm` package:

- For x86_64 devices [Download](https://github.com/liber-bkm/liber/releases/download/v0.11.1/liber-0.11.1-1.x86_64.rpm)
- For arm64 devices [Download](https://github.com/liber-bkm/liber/releases/download/v0.11.1/liber-0.11.1-1.aarch64.rpm)

### MacOS

Download for MacOS (works for both intel and Apple Silicon)

- Download the app installer from [here](https://github.com/liber-bkm/liber/releases/latest/download/liber-darwin.dmg)
- Download tarball from [here](https://github.com/liber-bkm/liber/releases/latest/download/liber-darwin.tar.gz)

### Windows 

- Download windows installer from [here](https://github.com/liber-bkm/liber/releases/latest/download/liber-setup-x64.exe)
- Portable version can be downloaded from [here](https://github.com/liber-bkm/liber/releases/download/v0.11.1/liber-windows-x86_64.zip)

### Android 

You can download the unviersal apk, that works on almost all devices, from [here](https://github.com/liber-bkm/liber/releases/latest/download/liber-app-universal.apk)

### Linux Tarballs 

If your distro specific package is not available you can either install the appimage or use the [linux tarball](https://github.com/liber-bkm/liber/releases/latest/download/liber-linux-x86_64.tar.gz). To install binaries from tarball simply extract the download and place the in you path.

``` sh
# Download and extract
wget https://github.com/liber-bkm/liber/releases/latest/download/liber-linux-x86_64.tar.gz
tar xzf liber-linux-x86_64.tar.gz
cd liber-*
# Move the file to PATH
mv liber* /usr/local/bin # to install them system wide or use ~/.local/bin to install them per user
```

## Build from source 

To build liber from source, you'd need rust/cargo, nodejs and pnpm. Or if you have `nix` installed you can simply use `nix develop .#default` and all the build tools will be available in nix shell 
 
To build CLI app without server support

``` sh
rustup target add <rust cpu architecture> # you can check with rustup target list 
rustup toolchain install stable 
cargo build --release -p liber-cli
```

To build CLI app with server support 

``` sh
cd frontend && pnpm install && pnpm build && cd ..
EMBED_UI=1 cargo build --release -p liber-cli 
```

To build desktop app (provided you have already built the web front end using pnpm)

``` sh
cd crates/liber-tauri && ../../frontend/node_modules/.bin/tauri build 
```


## Usage

>You can find the detailed guide and usage on documentation site [liber-bkm.github.io](https://liber-bkm.github.io)

At start run `liber init` to setup defaults. Using GUI is pretty straightforward. A quick glance at cli usage:

``` sh
liiber serve # Serve the webUI for GUI usage
liber add <url> # add bookmark, you can add multiple urls separated by <space>
liber add -h # all options for add command
liber add -i # an interact prompt that allows you to add url, tags, folder and so on 
liber list # list bookmarks 
liber list --in <field> # list in specific field i.e tags folder
liber pick # Interactively search 
liber edit <id> # Edit speicif id or ranges of them 
liber reindex # Recalculate bookmark ids 
liber tags -h # options to edit, list and rename tags 
liber auto -h # otions to automatically classify bookmarks 
liber check # check link health
liber attachments <id> # list attachments of that bookmark 
liber archive <id> # add archive of that bookmark 
liber profile -h # create, list, switch and delete profiles 

```


### Addressing bookmarks and search syntax

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

### Example CLI usage 

A typical session moves through capture, triage, verify, and sync:

```sh
liber add https://example.com/article -t reading --folder tech --archive
liber add -i https://example.com/other     # prompted: title, tags, folder, notes, attachments
liber list reading --sort newest
liber edit 3 --tags "reading rust" --folder tech
liber check --stale 7d --apply             # review moved/dead links
liber sync commit                          # snapshot when the base dir is a git repo
```

## One Click bookmark add 

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

## License

GPL-3.0-or-later. See the [GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html).
