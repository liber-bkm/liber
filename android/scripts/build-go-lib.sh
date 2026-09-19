#!/bin/sh
# Builds the liber Go binary for Android ABIs into app jniLibs.
# Run from the repo root: sh android/scripts/build-go-lib.sh
# VERSION overrides the stamped version (default: git describe).
set -eu
cd "$(dirname "$0")/../.."

VER="${VERSION:-$(git describe --tags --always --dirty 2>/dev/null || echo dev)}"

mkdir -p android/app/src/main/jniLibs/arm64-v8a
GOOS=android GOARCH=arm64 CGO_ENABLED=0 go build \
    -ldflags "-X main.Version=${VER}" \
    -o android/app/src/main/jniLibs/arm64-v8a/libliber.so .
echo "built arm64-v8a (${VER})"

# x86_64 (emulators, Chromebooks) needs an NDK clang as external linker,
# plain Go cannot link it (android/amd64 requires external linking):
#   NDK=<ndk> CC=${NDK}/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android35-clang \
#   CGO_ENABLED=1 GOOS=android GOARCH=amd64 go build \
#       -ldflags "-X main.Version=${VER}" \
#       -o android/app/src/main/jniLibs/x86_64/libliber.so .
