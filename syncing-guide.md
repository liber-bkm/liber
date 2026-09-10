# Syncing liber across devices

liber stores everything as flat files plus one JSON index, so any
folder-syncing tool works with no plugins and no server. When sync tools
clash, `liber -r --merge` folds the conflict copies back into one index.
This guide covers four methods: git (versioned), Syncthing, Nextcloud, and
Google Drive / Dropbox. It also covers syncing without git at all,
including syncing with an Android device that is used regularly.

> [!Note]
> An Android version of liber is currently on the long-term roadmap. Although liber works on Android perfectly using Termux, a native Android port will only be planned if there is enough interest. Alternatively, liber can be self-hosted and a reverse proxy used for `liber --serve` ports, which eliminates the troubles of syncing since a single device manages liber and hosts it for others to consume. Just make sure to run that through a tunnel, since liber serves the WebUI with full read and write permissions for bookmarks.

## How liber storage maps to sync

- `<base_dir>/` holds `html/`, `markdown/`, `archive/`, `attachments/`,
  plus `.liber/index.json` (the index: ids, tags, folders, rules) and
  `.liber/journal/` (one small file per change, used to replay changes
  that last-writer-wins sync would otherwise drop).
- Point any sync tool at `base_dir` and the whole collection follows.
  Change it with `liber config set base_dir <path>`.
- Each `--profile` is a subfolder with its own index, so profiles sync
  independently under the same `base_dir`.
- `liber -r` never deletes user data on mismatch: files whose index entry
  is gone move to `<base_dir>/unindexed/...`, never to trash.

## The one rule for all methods

Do not add bookmarks on two devices while both are offline. Each device
bumps its own copy of the id counter, so both assign the same id to
different bookmarks and the sync has to pick a winner. `--merge` keeps
both by reassigning fresh ids (see below), but avoiding the clash is
still cheaper: sync before switching devices, and after adding on one
device, let it finish syncing before adding on another.

## Merging conflicts with `liber -r --merge`

When a sync tool hits a clash it keeps conflict copies of `index.json`
next to it in `.liber/` (Syncthing `sync-conflict`, Nextcloud/Dropbox
conflicted copies). Plain `liber -r` lists them and changes nothing.
`liber -r --merge` folds them in, then runs the normal cleanup:

```sh
liber -r            # reports conflict copies, changes nothing
liber -r --merge    # merge, replay journal, adopt orphans, relink siblings
liber -r --merge --all  # same, also merging .liber/*.json copies without conflict in the name
liber -r --prune    # drop entries with missing files after the merge step
liber -r --compact  # renumber ids to close gaps (renames files, syncs more)
liber -r --prune-journal  # delete applied journal files older than 90 days
```

Merge rules, in order:

- Same id, same bookmark: fields merge. Newer edit wins text, tags and
  attachments union, nothing is lost from either side.
- Same id, different bookmarks (added offline on both sides): the
  incoming one gets a fresh id and its files are renamed to match, so
  both bookmarks survive.
- Same URL under different ids: duplicates fold into the richer entry
  (tags merge); the loser's files move to `unindexed/` instead of being
  deleted.
- Automation rules union by match text. Copies that fail to parse are
  reported and skipped, never fatal.

Consumed copies move to `.liber/resolved/` (never deleted), so the next
`-r` finds nothing left to merge. The run prints what it merged,
reassigned, folded away, and skipped.

Merge base: the index with the most bookmarks wins. On equal size the
older file wins, except ties within 30 seconds keep the local index and
print a clock skew notice. Device clocks affect this tie break.

Safety defaults: missing HTML files stay in the index as pending and are
listed on every run. They are only dropped with `-r --prune`. Sibling
markdown and archive files found on disk are relinked automatically.
Orphan attachments named `NNNN-slug.ext` are relinked to the bookmark
with that id; other orphan attachments move to `unindexed/attachments/`.
Content conflict files (`sync-conflict`, `conflicted`) in bookmark
directories are never adopted as new bookmarks. They move to
`unindexed/` with the conflict name preserved so both versions survive
for manual review.
Renumbering only runs with `-r --compact` because it renames files and
creates extra sync traffic.

## The journal

Every change (add, edit, delete, tag and folder moves, automation rules)
also appends one small file to `.liber/journal/`, named with a timestamp,
a per-device id, and random characters, so two devices never produce the
same filename and sync never collapses two changes into one. `liber -r
--merge` replays journal files it has not applied yet, using the same
rules as index merging: same id plus same URL merges fields with newer
`UpdatedAt` winning, same id plus different bookmarks keeps both under
fresh ids, same URL folds into one entry. Deletes replay as tombstones:
a bookmark edited after the delete wins and survives, otherwise the
delete applies and its files move to `unindexed/`. This is what makes
Drive-style last-writer-wins sync converge: even when `index.json` keeps
only the last writer, the journal still carries every change.

The journal must sync (do not exclude `.liber/journal/`). Old clients
ignore the directory. Each device gets an id in `config.json`
(`device_id`, generated on first write, changeable with `liber config
set device_id <name>`). Journal files older than 90 days that were
already applied are deleted with `liber -r --prune-journal`. A device
offline longer than that still converges through `index.json`, except a
deliberate delete it never saw stays pending until pruned by hand.

## Method 1: git / jj (`liber --sync`)

Best when: you want history and explicit conflict handling.

```sh
cd <base_dir> && git init   # liber never inits for you; once, manually
liber --sync                # commit
liber --sync -p             # commit and push
```

`--sync` finds the repo at or above `base_dir`, commits, and with `-p`
pushes. Text conflicts resolve with normal git tools; `index.json` is
readable JSON, so merges are usually trivial. Folder-sync conflicts
inside a git repo still resolve through `--merge`; run it before
committing so the commit captures one clean index.

## Method 2: Syncthing (recommended non-git)

Best when: you want automatic sync with no account and no server company.

1. Share `<base_dir>` as a Syncthing folder on each device (including
   Syncthing-for-Android).
2. Keep default conflict handling: on a clash Syncthing keeps both copies
   instead of overwriting.
3. Run `liber -r --merge` on the desktop. It reports what it merged and
   moves consumed copies to `.liber/resolved/`.

Works over LAN without internet. Versioning (trash can) is optional per
folder and worth enabling for `index.json` peace of mind.

## Method 3: Nextcloud

Best when: you already run Nextcloud.

1. Move or point `base_dir` inside the synced Nextcloud folder.
2. Conflicts surface as `conflicted copy` files; `liber -r --merge`
   handles them the same way as Syncthing copies.
3. Large archives sync slowly on first upload; afterward only changed
   files move. The desktop client handles this better than the mobile one,
   so expect the first sync to take a while on big collections.

## Method 4: Google Drive / Dropbox

Best when: that is where your files already live. Works, with caveats:

- Conflict handling is proprietary (Drive: keeps both, renames opaquely;
  Dropbox: `conflicted copy`). `--merge` detects any `*conflict*` name.
  Use `liber -r --merge --all` for copies without the word conflict in
  the name. Even with no copy to merge, `-r --merge` still replays the
  journal, so adds from the other side converge. Concurrent edits to the
  same bookmark converge on newer `UpdatedAt`; notes file content follows
  the surviving file, so check `unindexed/` when both sides edited text.
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
   up, then run `liber -r --merge` on the desktop. Colliding adds each
   keep their bookmark under a fresh id; check the merge report for
   reassigned ids.
5. If the merge report mentions skipped (unparseable) copies, resolve
   those by hand: compare against the current index, keep what matters,
   delete the copy, re-run `liber -r --merge`.

## Recommended workflow

1. Let the sync client finish fully before switching devices, and again
   before running liber after switching.
2. Add on one side at a time where you can; colliding offline adds still
   merge, but avoiding them is cheaper.
3. Run `liber -r --merge` on the desktop (add `--all` on Drive or
   Dropbox). Read the merge and journal report for reassigned ids.
4. Check `unindexed/` when the report mentions duplicates, conflicts, or
   deleted entries, and resolve those by hand.
5. Only run `--prune` or `--compact` on a fully synced desktop. Never
   prune on a partial sync: pending entries are how liber waits for
   files that have not arrived yet.
6. Deletes propagate as pending entries on other devices; run `--prune`
   there too once sync is complete.

## Recovery cheat sheet

| Symptom | Fix |
|---|---|
| Conflict copies appeared | `liber -r --merge`; consumed copies move to `.liber/resolved/` |
| Bookmarks vanished after sync | Check `<base_dir>/unindexed/`; files are moved, not deleted |
| Duplicate ids suspected | Back up `index.json`, run `liber -r --merge`, read the report |
| Phone shows stale data | Force a sync in the provider app; Android sync is battery-gated |
| Unparseable copy reported | Compare by hand, delete it, re-run `liber -r --merge` |

## What not to sync

- `site/` (static export output): regenerable via `liber --export-site`,
  exclude it to save bandwidth.
- `.liber/resolved/` and `.liber/restage/`: regenerable bookkeeping;
  exclude them to avoid sync noise.
- `unindexed/`: quarantine for duplicates and orphans; excluding it avoids
  syncing files back and forth. Keep a local copy if you need recovery.
- `*.tmp` next to `index.json`: partial writes; exclude them so sync tools
  never copy a half written index.
- `.liber/journal/` must sync. It carries changes that the index alone
  would lose on last-writer-wins providers.
- `config.json` is per-machine (paths differ); profiles and settings do
  not roam with these methods. Only `base_dir` content syncs. Each
  machine keeps its own `device_id`.
