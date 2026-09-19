# liber Android wrapper

Thin native wrapper around the liber Go binary: on launch it starts
`liber --serve` on loopback and shows the web UI in a WebView. All bookmark
logic stays in Go; Android contributes lifecycle, private storage, and link
handling. No gomobile, no UI rewrite.

## Layout

- `settings.gradle`, `build.gradle`, `gradle.properties`: Gradle project,
  Android Gradle Plugin 8.5.2, no third-party dependencies (framework
  WebView only).
- `app/build.gradle`: `bkm.liber`, minSdk 26, targetSdk/compileSdk 35.
  Bump `versionCode`/`versionName` together with liber releases.
- `app/src/main/jniLibs/<abi>/libliber.so`: Go binary, built by
  `scripts/build-go-lib.sh`, never committed (gitignored).
- `app/src/main/`: manifest (INTERNET only), `MainActivity.kt`, layout,
  strings.

## Build

Prereqs: JDK 17, Gradle 8.x, Android SDK with platform-35 and build-tools.

```sh
# 1. From the repo root, build the Go binary for arm64:
sh android/scripts/build-go-lib.sh

# 2. Assemble the debug APK:
cd android && gradle assembleDebug
# APK: app/build/outputs/apk/debug/app-debug.apk
```

Install with `adb install`, or transfer the APK to the device and open it.
First launch creates `<app-files>/bookmarks` (`LIBER_BASE_DIR`) and
`<app-files>/liber-config.json` (`LIBER_CONFIG`).

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

## Distribution

F-Droid first (source-based review, matching audience), Play Store second.
Play notes: keep permissions at INTERNET only, no foreground service while
the server lives and dies with the activity, verify 16KB-page-clean native
output for Android 15+ targets, and expect new personal accounts to need
14 days of closed testing before production access.
