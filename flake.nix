{
  description = "liber-rs - professional bookmark manager (Rust + Tauri)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        version =
          let
            toml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          in
          toml.workspace.package.version or "0.1.0";

        rustToolchain = fenix.packages.${system}.complete.withComponents [
          "cargo"
          "clippy"
          "rust-src"
          "rustc"
          "rustfmt"
        ];
        rustTargets = fenix.packages.${system}.targets;
        crossTargets = with rustTargets; [
          x86_64-unknown-linux-gnu.toolchain
          x86_64-pc-windows-gnu.toolchain
          x86_64-apple-darwin.toolchain
          aarch64-apple-darwin.toolchain
          aarch64-linux-android.toolchain
        ];

        # Scoped package set for Android tooling. The SDK is proprietary, so
        # license acceptance and unfree permission live here only; the default
        # shell and packages stay fully free.
        pkgsAndroid = import nixpkgs {
          inherit system;
          config = {
            android_sdk.accept_license = true;
            allowUnfree = true;
          };
        };
        androidSdk = pkgsAndroid.androidenv.composeAndroidPackages {
          platformVersions = [ "35" ];
          buildToolsVersions = [ "35.0.0" ];
          includeNDK = true;
        };
        androidFhsEnv = pkgsAndroid.buildFHSEnv {
          name = "liber-rs-android-env";
          targetPkgs = pkgs: [
            rustToolchain
            pkgs.jdk17
            pkgs.gradle
            androidSdk.androidsdk
            androidSdk.platform-tools
            pkgs.nodejs_22
            pkgs.glibc
            pkgs.zlib
            pkgs.stdenv.cc.cc.lib
            pkgs.which
          ];
          runScript = pkgs.writeShellScript "enter-liber-rs-android-env" ''
            export ANDROID_HOME="${androidSdk.androidsdk}/libexec/android-sdk"
            export ANDROID_SDK_ROOT="$ANDROID_HOME"
            export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk-bundle"
            export PATH="$ANDROID_HOME/platform-tools:$PATH"
            echo "liber-rs Android FHS shell (standard loader contract; exit leaves nix develop)"
            echo "Android SDK: $ANDROID_HOME"
            exec bash
          '';
        };

        linuxTauriDeps = with pkgs; [
          pkg-config
          openssl
          dbus
          gtk3
          glib
          webkitgtk_4_1
          libsoup_3
          librsvg
          libappindicator-gtk3
          patchelf
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "liber-rs";
          inherit version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildInputs = [ pkgs.openssl ];
          nativeBuildInputs = [ pkgs.pkg-config ];
        };

        # Cross-built artifacts (provide-only; only default is tested).
        # Windows via mingw + rust target; macOS is compile-check only
        # (no Apple SDK/signing from Linux).
        packages.liber-windows-x86_64 =
          (pkgs.pkgsCross.mingwW64.rustPlatform.buildRustPackage {
            pname = "liber-rs-windows";
            inherit version;
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
          });

        # Default Linux dev+test shell. This is the ONLY shell the agent
        # tests in. Cross helpers are installed here so availability is
        # guaranteed, but cross/android builds are not executed by default.
        devShells.default = pkgs.mkShell {
          buildInputs =
            [
              rustToolchain
              pkgs.rust-analyzer
              pkgs.cargo-nextest
              pkgs.cargo-audit
              pkgs.cargo-zigbuild
              pkgs.cargo-xwin
              pkgs.cargo-ndk
              pkgs.zig
              pkgs.nodejs_22
              pkgs.nodePackages.pnpm
              pkgs.sqlite
              pkgs.pkg-config
              pkgs.chromium
            ]
            ++ crossTargets
            ++ linuxTauriDeps;
          shellHook = ''
            echo "liber-rs dev shell (Linux test-only)"
            echo "Targets available: linux-gnu, windows-gnu, apple-darwin, android (compile-check only except linux)"
            echo "Android work: use .#android or .#android-fhs instead"
          '';
        };

        # Cross-compile helper shell (Windows/macOS toolchains from Linux).
        devShells.cross = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.zig
            pkgs.cargo-zigbuild
            pkgs.cargo-xwin
            pkgs.pkgsCross.mingwW64.stdenv.cc
          ];
          shellHook = ''
            echo "liber-rs cross shell: cargo zigbuild / cargo xwin available"
            echo "macOS outputs are compile-check only (no signing)."
          '';
        };

        # Android wrapper toolchain: JDK, Gradle, SDK+NDK, Tauri Mobile deps.
        # Assemble via Gradle/Tauri (no network in nix builds), so nix is the
        # tool source, not the packager.
        devShells.android = pkgsAndroid.mkShell {
          buildInputs = [
            rustToolchain
            pkgsAndroid.jdk17
            pkgsAndroid.gradle
            androidSdk.androidsdk
            androidSdk.platform-tools
            pkgsAndroid.nodejs_22
            pkgsAndroid.cargo-ndk
          ];
          ANDROID_HOME = "${androidSdk.androidsdk}/libexec/android-sdk";
          ANDROID_SDK_ROOT = "${androidSdk.androidsdk}/libexec/android-sdk";
          shellHook = ''
            export PATH="$ANDROID_HOME/platform-tools:$PATH"
            echo "Android SDK: $ANDROID_HOME"
            echo "Note: APK assembly needs the FHS shell (nix develop .#android-fhs);"
            echo "this shell is for Rust/JS builds, scripts, and sdkmanager only."
          '';
        };

        packages.android-fhs = androidFhsEnv;

        devShells.android-fhs = pkgs.mkShell {
          buildInputs = [ androidFhsEnv ];
          shellHook = ''
            exec ${androidFhsEnv}/bin/liber-rs-android-env
          '';
        };

        apps.default = flake-utils.lib.mkApp { drv = self.packages.${system}.default; };
      }
    );
}
