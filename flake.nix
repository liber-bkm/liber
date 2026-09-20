{
  description = "liber - a small CLI bookmark manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        version = "0.9.1";
        # Scoped package set for Android tooling. The SDK is proprietary, so
        # license acceptance and unfree permission live here only; the default
        # shell and packages stay fully free.
        pkgsAndroid = import nixpkgs {
          inherit system;
          config = {
            android_sdk.accept_license = true;
            # The SDK is a proprietary build tool only; it never ships in the
            # product (liber stays GPL, static pure-Go binary).
            allowUnfree = true;
          };
        };
        androidSdk = pkgsAndroid.androidenv.composeAndroidPackages {
          platformVersions = [ "35" ];
          buildToolsVersions = [ "35.0.0" ];
        };
      in
      {
        packages.default = pkgs.buildGoModule {
          pname = "liber";
          inherit version;
          src = ./.;

          vendorHash = null;

          ldflags = [ "-X main.Version=${version}" ];
          buildInputs = [
            pkgs.fzf
            pkgs.single-file-cli
            pkgs.monolith
          ];

          meta = with pkgs.lib; {
            description = "A small CLI bookmark manager (html + markdown + archive)";
            homepage = "https://example.invalid/liber";
            license = licenses.mit;
            mainProgram = "liber";
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            pkgs.go
            pkgs.gopls
            pkgs.fzf
            pkgs.monolith
            pkgs.single-file-cli
          ];
        };

        # Cross-built Go binary for embedding in the Android wrapper
        # (android/app/src/main/jniLibs/<abi>/libliber.so). Only arm64 builds
        # with plain Go; other ABIs need an NDK clang (see the build script).
        # A plain derivation (not buildGoModule) because cross env vars
        # collide with buildGoModule's own GOOS/GOARCH/CGO_ENABLED.
        packages.liber-android-arm64 = pkgs.stdenv.mkDerivation {
          pname = "liber-android-arm64";
          inherit version;
          src = ./.;
          nativeBuildInputs = [ pkgs.go ];
          GOOS = "android";
          GOARCH = "arm64";
          CGO_ENABLED = "0";
          buildPhase = ''
            export GOCACHE="$TMPDIR/go-cache"
            export GOPATH="$TMPDIR/go-path"
            export GOMODCACHE="$TMPDIR/go-modcache"
            export HOME="$TMPDIR"
            go build -ldflags "-X main.Version=${version}" -o liber .
          '';
          installPhase = ''
            install -Dm755 liber $out/bin/liber
          '';
        };

        # Android wrapper toolchain: JDK, Gradle, SDK. The APK itself is
        # assembled by Gradle (no network in nix builds), so nix is the tool
        # source, not the packager: `nix develop .#android`, then
        # `sh android/scripts/build-go-lib.sh` and `gradle assembleDebug`.
        devShells.android = pkgsAndroid.mkShell {
          buildInputs = [
            pkgsAndroid.go
            pkgsAndroid.jdk17
            pkgsAndroid.gradle
            androidSdk.androidsdk
            androidSdk.platform-tools
          ];
          ANDROID_HOME = "${androidSdk.androidsdk}/libexec/android-sdk";
          ANDROID_SDK_ROOT = "${androidSdk.androidsdk}/libexec/android-sdk";
          shellHook = ''
            export PATH="$ANDROID_HOME/platform-tools:$PATH"
            echo "Android SDK: $ANDROID_HOME"
          '';
        };

        apps.default = flake-utils.lib.mkApp { drv = self.packages.${system}.default; };
      }
    );
}
