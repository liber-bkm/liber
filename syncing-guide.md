# Syncing liber across devices

liber stores everything as flat files plus one JSON index, so any
folder-syncing tool works with no plugins and no server. This guide covers
four methods: git (versioned), Syncthing, Nextcloud, and Google Drive /
Dropbox. It also covers syncing without git at all, including syncing with
an Android device that is used regularly.

> [!Note]
> An Android version of liber is currently on the long-term roadmap. Although liber works on Android perfectly using Termux, a native Android port will only be planned if there is enough interest. Alternatively, liber can be self-hosted and a reverse proxy used for `liber --serve` ports, which eliminates the troubles of syncing since a single device manages liber and hosts it for others to consume. Just make sure to run that through a tunnel, since liber serves the WebUI with full read and write permissions for bookmarks.

## How liber storage maps to sync

- `<base_dir>/` holds `html/`, `markdown/`, `archive/`, `attachments/`,
  plus `.liber/index.json` (the index: ids, tags, folders, rules).
- Point any sync tool at `base_dir` and the whole collection follows.
  Change it with `liber config set base_dir <path>`.
- Each `--profile` is a subfolder with its own index, so profiles sync
  independently under the same `base_dir`.
- `liber -r` never deletes user data on mismatch: files whose index entry
  is gone move to `<base_dir>/unindexed/...`, never to trash. This is the
  recovery path for every method below.

## The one rule for all methods

Do not add bookmarks on two devices while both are offline. Each device
bumps its own copy of the id counter, so both assign the same id to
different bookmarks and the sync has to pick a winner. The losing entry's
files land in `unindexed/` (nothing is lost), but the index entry is gone.
In practice: sync before switching devices, and after adding on one
device, let it finish syncing before adding on another.

## Method 1: git / jj (`liber --sync`)

Best when: you want history and explicit conflict handling.

```sh
cd <base_dir> && git init   # liber never inits for you; once, manually
liber --sync                # commit
liber --sync -p             # commit and push
```

`--sync` finds the repo at or above `base_dir`, commits, and with `-p`
pushes. Conflicts resolve with normal git tools; `index.json` is readable
JSON, so merges are usually trivial. Run `liber -r` after resolving.

## Method 2: Syncthing (recommended non-git)

Best when: you want automatic sync with no account and no server company.

1. Share `<base_dir>` as a Syncthing folder on each device (including
   Syncthing-for-Android).
2. Keep default conflict handling: on a clash Syncthing keeps both copies
   (`sync-conflict-<date>-<device>.<ext>`) instead of overwriting.
3. If you see a `sync-conflict-...index.json`: compare, keep one as
   `.liber/index.json`, back the other up elsewhere, then run `liber -r`
   to quarantine anything orphaned into `unindexed/`.

Works over LAN without internet. Versioning (trash can) is optional per
folder and worth enabling for `index.json` peace of mind.

## Method 3: Nextcloud

Best when: you already run Nextcloud.

1. Move or point `base_dir` inside the synced Nextcloud folder.
2. Conflicts surface as `conflicted copy` files in the web UI and client;
   resolve the same way as Syncthing: pick the surviving `index.json`,
   back up the other, run `liber -r`.
3. Large archives sync slowly on first upload; afterward only changed
   files move. The desktop client handles this better than the mobile one,
   so expect the first sync to take a while on big collections.

## Method 4: Google Drive / Dropbox

Best when: that is where your files already live. Works, with caveats:

- Conflict handling is proprietary (Drive: keeps both, renames opaquely;
  Dropbox: `conflicted copy`). Recovery is the same: choose the surviving
  `index.json`, back up the other, `liber -r`.
- Treat these as last-writer-wins and keep the one-adder discipline
  stricter than with Syncthing/Nextcloud.
- On mobile, sync is battery-gated and partial: fine for reading, avoid
  adding bookmarks from the phone on these providers.

Typical setup uses the provider's desktop client plus
`liber config set base_dir ~/Google\ Drive/Bookmarks`
(or `rclone mount` on Linux for Drive).

## Syncing with a regularly used Android device (no git needed)

The supported shape is: sync the folder with one of the methods above,
point the Android build at the synced copy. Since the phone adds
bookmarks too, both sides follow the same discipline.

1. Install a sync client on the phone (Syncthing-for-Android is the
   smoothest for two-way use; Nextcloud works; Drive/Dropbox apps are
   read-mostly, see Method 4).
2. Let it sync `<base_dir>` fully before first launch.
3. When switching sides, sync first: on the phone, force a sync in the
   provider app (Android sync is battery-gated and may lag); on desktop,
   let the client finish before opening liber.
4. After adding on the phone: sync the phone, then let the desktop catch
   up, then run `liber -r` on the desktop and check `unindexed/` for
   anything the merge dropped.
5. If both sides added while offline and ids collide, the desktop pass in
   step 4 is the recovery point: surviving entries stay, losers land in
   `unindexed/` with their files intact, ready to re-add.

## Recovery cheat sheet

| Symptom | Fix |
|---|---|
| `sync-conflict-...index.json` appeared | Keep one as `.liber/index.json`, back up the other outside `base_dir`, run `liber -r` |
| Bookmarks vanished after sync | Check `<base_dir>/unindexed/`; files are moved, not deleted |
| Duplicate ids suspected | `liber -l` shows gaps or wrong titles; back up `index.json`, run `liber -r` |
| Phone shows stale data | Force a sync in the provider app; Android sync is battery-gated |

## What not to sync

- `site/` (static export output): regenerable via `liber --export-site`,
  exclude it to save bandwidth.
- `config.json` is per-machine (paths differ); profiles and settings do
  not roam with these methods. Only `base_dir` content syncs.
