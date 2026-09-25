# liber Android wrapper

Thin native wrapper around the liber Go binary: on launch it starts
`liber --serve` on loopback and shows the web UI in a WebView. All bookmark
logic stays in Go; Android contributes lifecycle, private storage, and link
handling. No gomobile, no UI rewrite.

## Quick start (personal debug APK)

With nix (primary tool source; in distrobox, prefix host nix calls with
`distrobox-host-exec`):

```sh
nix develop .#android-fhs   # FHS chroot: same toolchain plus a standard
                            # /lib64 loader, so Gradle-fetched Google binaries
                            # (aapt2, d8, apksigner) run unmodified.
                            # Exiting the inner bash ends the session.
sh android/scripts/build-go-lib.sh
cd android && ./gradlew assembleDebug --no-daemon
adb install app/build/outputs/apk/debug/app-debug.apk
```

Without nix: install Temurin JDK 17, Gradle 8.x, and the SDK
(`platform-tools`, `platforms;android-35`, `build-tools;35.0.0`; accept
licenses via `yes | sdkmanager --licenses`), export `ANDROID_HOME`, then run
the same three commands. Enable USB debugging on the phone and accept the
host key so `adb devices` lists it. The APK is unsigned debug, fine for
personal use, not for stores.

Troubleshooting: if Gradle demands an SDK component (e.g. offers to install
build-tools), do not install it ad hoc. The nix SDK is read-only by design,
so the fix is always version pins: `platformVersions`/`buildToolsVersions`
in `flake.nix` must match `compileSdk`/`buildToolsVersion` in
`android/app/build.gradle` and the AGP line must support that API level
(AGP 8.7+ for 35).

## Layout

- `settings.gradle`, `build.gradle`, `gradle.properties`: Gradle project,
  Android Gradle Plugin 8.7.3, no third-party dependencies (framework
  WebView only).
- `app/build.gradle`: `bkm.liber`, minSdk 26, targetSdk/compileSdk 35.
  `versionName`/`versionCode` come from `-PliberVersionName`/`-PliberVersionCode`
  (CI passes the release tag and `major*10000 + minor*100 + patch`); the checked-in
  fallbacks are debug-build only. Bump the fallbacks when liber releases.
- `app/src/main/jniLibs/<abi>/libliber.so`: Go binary, built by
  `scripts/build-go-lib.sh`, never committed (gitignored).
- `app/src/main/`: manifest (INTERNET only), `MainActivity.kt`, layout,
  strings.

## Build

Prereqs: JDK 17, Android SDK with platform-35 and build-tools. Gradle itself
comes from the committed wrapper (`./gradlew`, pinned to 8.10.2), so no
system Gradle is needed.
With nix (primary tool source): `nix develop .#android` provides Go, JDK 17,
Gradle, and the SDK with `ANDROID_HOME`/`ANDROID_SDK_ROOT` preset (fast shell
for Go builds and scripts). Gradle assembly itself must run inside
`nix develop .#android-fhs`, which adds a standard /lib64 loader contract so
raw Google binaries run unmodified.

```sh
# 1. From the repo root, build the Go binary for arm64:
sh android/scripts/build-go-lib.sh
#    or reproducibly via nix: nix build .#liber-android-arm64
#    then copy result/bin/liber to
#    android/app/src/main/jniLibs/arm64-v8a/libliber.so

# 2. Assemble the debug APK:
cd android && ./gradlew assembleDebug
# APK: app/build/outputs/apk/debug/app-debug.apk
```

Verify the APK actually contains compiled code before installing (a missing
Kotlin plugin once shipped a codeless APK that crashed on launch with
`ClassNotFoundException`):

```sh
unzip -l app/build/outputs/apk/debug/app-debug.apk | grep classes
# expect a multi-megabyte classes.dex; bytes means something is wrong
```

Install with `adb install`, or transfer the APK to the device and open it.
First launch creates `<app-files>/bookmarks` (`LIBER_BASE_DIR`) and
`<app-files>/liber-config.json` (`LIBER_CONFIG`).

## Manual test checklist (on device)

File inputs in the WebView only work through the `WebChromeClient` bridge in
`MainActivity.kt`; verify after any change there:

1. Attach a file to a bookmark from the edit page, then open it back.
2. Import a browser bookmark export via settings, confirm counts.
3. Open the picker and cancel it, confirm no crash and the next picker still works.
4. Attach two files at once (multiple selection path).
5. Share a page URL from the browser to liber, confirm the add form opens prefilled.
6. Share plain text without a URL, confirm it falls back to search.
7. Share while the app is already open, confirm it navigates without losing history.
8. Export the static site from settings, tap the download, confirm it lands in Downloads/liber with a completion notice.
9. Open the ⋮ menu, pick a sync folder, confirm the toast, restart the app, confirm the folder is remembered.
10. Open the ⋮ menu and dismiss it without choosing (back button, then again via tap-outside), reopen both times and confirm it appears every time.
11. With an export present, use Export to sync folder, confirm index.html appears in the picked folder (re-run replaces, never duplicates).
12. Fresh install shows the mode dialog; pick standalone, confirm the local UI loads.
13. From the ⋮ menu open Server mode, enter a LAN URL, confirm the remote UI loads and loopback links stay in-app.
14. Against an authed server: save the token in Server mode, confirm the UI loads without a manual login; wrong token falls back to the server login page (logcat notes the rejection).
15. Switch back to standalone, confirm the local UI returns and no second server lingers (logcat shows one liber startup).
16. Cold start loads the page without manual retry; killing the server mid-run shows the error view whose Retry recovers without changing ports.
17. Full backup loop: settings, Library, Download browser export, confirm `liber-bookmarks.html` in Downloads; reinstall the app; settings, Library, import the file; confirm every bookmark, tag, and folder is back.
18. ⋮ menu, Native UI (beta): list loads the same bookmarks as the WebView view; a search narrows them; tapping one opens it externally; airplane-mode error shows instead of a hang.
19. Native detail: tapping a result shows full fields with working Open (history count increments, visible in WebView history); missing id shows retryable error.
20. Native add: new URL saves and appears in both views; duplicate URL shows the already-bookmarked dialog with working Add-anyway.
21. Native edit: from detail open Edit, change title/tags, save, confirm the change shows in both views; blank title/URL blocks saving; missing id shows retryable error.
22. Native tags: list matches the WebView tags page counts; rename updates every bookmark (renaming onto an existing tag merges); delete asks with the affected count and updates both views; offline shows a retryable error.
23. Native folders: reached from the tags screen, counts match the WebView page; rename moves the subtree (merging onto an existing folder); delete moves the subtree back to root after a count confirm; root has no actions; offline shows a retryable error.
24. Native rules: reached from the tags screen, list matches `--auto list` with applied counts; add with folder/tags backfills; suggestions preview with min, create-all, and apply-all work; delete confirms and leaves classified bookmarks as-is; offline shows a retryable error.
25. Native check: Check button on the list, spec/stale scan matches the WebView buckets; moved rows update URL with optional title refresh; dead/uncertain rows delete (with confirm) or quarantine and drop from the list; offline or bad spec shows a retryable error.
26. Native settings: counts, base dir, and maintenance status match the WebView settings page; backend selector persists and reloads; offline shows a retryable error.
27. Native look: gruvbox light and dark schemes follow the system setting; every screen shows its top bar, cards, and dialogs correctly in both; icons and chips render; rotation keeps state.
28. Native list parity: collection auto-loads; scope chips, Deep toggle, and sort narrow results like the WebView; detail delete confirms and the list refreshes without the deleted entry.

## Backup and reinstall (standalone mode)

The collection lives in app-private storage, which Android deletes with the
app. Before uninstalling or wiping, download a portable backup from settings,
Library, Download browser export (`liber-bookmarks.html`, Netscape format).
After reinstalling, restore it through settings, Library, import. Reimport
assigns fresh ids but keeps every URL, title, tag, folder, and description
(first line); nothing else is needed, since a fresh install has nothing to
conflict with.

## ABIs

`arm64-v8a` builds with plain Go, no NDK. `x86_64` (emulators, Chromebooks)
and 32-bit ABIs need an NDK clang as external linker; see the commented
recipe in `scripts/build-go-lib.sh`. Only ship ABIs you built.

## Behavior notes

- Server binds `127.0.0.1` on a runtime-picked free port and stops with the
  app. Nothing runs in the background.
- Links to 127.0.0.1 stay in the WebView; external links open the system
  browser (history tracking still applies, since taps go through `/open/`).
- Back button walks WebView history. Rotation does not restart the server
  (`configChanges` in the manifest).
- Server output goes to logcat under the `LiberApp` tag.
- Storage is app-private. Sync story: export/share from the app, or point a
  sync client at an app-exposed folder later; scoped storage makes arbitrary
  shared folders painful, so this is deliberately not attempted in v1.

## Android option semantics

- Config lives at app-private `filesDir/liber-config.json` (`LIBER_CONFIG`),
  collection at `filesDir/bookmarks` (`LIBER_BASE_DIR`). The file is not
  directly editable: manage everything through the settings page, which shows
  the effective path.
- Archiving is native-snapshot only. The backend selector still lists the
  other backends, but an empty backend resolves to native on Android and the
  settings page says so; `single-file` and `monolith` have no on-device
  binaries to call.
- `browser_cmd` is irrelevant inside the WebView (links are handled natively).
- `device_id` is per install. Each phone gets its own on first write.

## Distribution

F-Droid first (source-based review, matching audience), Play Store second.
Play notes: keep permissions at INTERNET only, no foreground service while
the server lives and dies with the activity, verify 16KB-page-clean native
output for Android 15+ targets, and expect new personal accounts to need
14 days of closed testing before production access.

## Release CI

`.github/workflows/release.yml` builds `liber-android-debug.apk` on every
release and uploads it next to the desktop binaries (included in
`SHA256SUMS.txt`). A manual `workflow_dispatch` run exercises the same path
without uploading anything. A signed `liber-android.apk` is built and uploaded only
when these repository secrets exist:

- `ANDROID_KEYSTORE_BASE64`: release keystore, base64-encoded
- `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`

The Gradle build reads them from `LIBER_KEYSTORE_*` env vars and skips
signing cleanly when absent, so forks without secrets still get the debug
APK. Generate the keystore once with
`keytool -genkey -v -keystore liber-release.keystore -alias liber -keyalg RSA
-keysize 2048 -validity 10000`, back it up somewhere safe, and never commit
it: losing the key means a new app identity on every store.
