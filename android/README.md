# liber for Android

liber on your phone: your bookmarks, tags, notes, and saved pages live
on the device and work offline. The app opens in the native view; a
legacy web view of the same collection is one tap away in settings.
No account, no cloud, no background services.

## Install

Get the APK from the release page (`liber-android-debug.apk` for
personal use; `liber-android.apk` when a signed build is published),
transfer it to your phone, and open it to install. Your Android version
may ask you to allow installs from that source first.

## First launch

On first launch the app asks how it should run:

- **Standalone (on this device)** keeps everything on the phone. Pick
  this unless you run liber on a computer too.
- **Connect to a server** talks to liber on your desktop over your
  local network. Enter its LAN address (and the auth token, if that
  server requires one).

## Finding bookmarks

The list shows your whole collection. Type to search; the chips narrow
the search to titles, URLs, tags, notes, or folders, Deep also searches
saved page content, and the Sort button reorders (newest, oldest, most
visited, title). Tapping a bookmark opens its detail page; Open launches
the live page in your browser.

Long-press list entries to select several at once, then delete them,
set their tags, or move them to a folder together.

## Adding bookmarks

Tap Add, enter the URL (the title is fetched automatically), and
optionally tick Markdown notes or a full-page archive. Archives take a
while to fetch. Attaching files works too.

The fastest way to save while browsing: use Android's share sheet on a
page and send it to liber. The add form opens prefilled. Sharing plain
text without a link opens a narrowed search instead.

Adding a URL that is already bookmarked tells you so, with the option
to add it anyway.

## Organizing

- **Detail, Edit, Delete**: every bookmark can be retitled, retagged,
  moved, given notes or an archive after the fact, or deleted (with a
  confirm step).
- **Tags** lists every tag with counts. Rename merges into an existing
  tag of that name; delete asks first and tells you how many bookmarks
  are affected.
- **Folders** works the same way for folders and subfolders. Deleting a
  folder moves its bookmarks back to the root; the root itself has no
  actions.
- **Rules** files new bookmarks automatically by URL or title match.
  The learn section suggests rules from how your collection already
  clusters, with create-all and apply-all. Editing a rule can reapply
  it to bookmarks it already classified.
- **Profiles** (from settings) keep fully separate collections, e.g.
  work and personal. Switching changes the collection everywhere;
  removing a profile only untracks it, nothing on disk is touched.
- **History** shows what you opened, most recent first.

## Saved pages and health

Detail offers the saved copies when they exist: the card (always kept),
the full-page archive, and your notes, all readable offline.
Attachments download and open in an external viewer.

Check scans your links and groups them into moved (update the URL,
optionally refreshing the title), dead, and uncertain (delete with a
confirm step, or quarantine into a folder for later review).

## Library and backup

From settings, Library covers the heavy lifting:

- **Import** a browser bookmark export picked from your files.
- **Share export** sends the whole collection as a portable bookmark
  file you can keep or move to another device.
- **Static site** writes a browsable offline copy into the collection.
- **Sync** commits the collection when it lives in a git/jj repo, with
  an optional push.
- **Maintenance** (reindex) merges conflict copies and journals, and
  can prune or compact on a fully synced collection. Prune and compact
  ask first.

**Before uninstalling or wiping the phone, export.** The collection
lives in app-private storage, which Android deletes with the app. Share
an export to somewhere safe first; after reinstalling, import it back.
Reimport assigns fresh ids but keeps every URL, title, tag, folder, and
description.

The ⋮ menu's sync folder picks a folder that survives reinstalls and
can carry the static-site export out of app-private storage.

## Settings

Settings shows the collection location, counts, and health status, plus:

- **Archive backend**: on Android only the built-in native snapshot is
  available (single-file and monolith need desktop binaries), so leave
  it on auto.
- **Profiles** and **Library** screens described above.
- **Switch to legacy WebView**: restarts into the web view of the same
  collection. The ⋮ menu there returns to the native view.

## Info

- The app needs the INTERNET permission only (loopback server, page
  fetches, archives). Nothing runs when the app is closed.
- Everything is stored privately on the device; settings shows the
  effective path but the config file itself is managed through the app.
- Rotating the phone never loses your place or restarts anything.
- If something goes wrong, airplane-mode screens show a retry instead
  of hanging. For a bug report, `adb logcat -s LiberApp` captures the
  app log.
