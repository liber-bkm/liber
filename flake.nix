{
  description = "liber-rs - professional bookmark manager (Rust + Tauri)";

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
        version =
          let
            toml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          in
          toml.workspace.package.version or "0.1.0";

        rustToolchain = with pkgs; [
          cargo
          rustc
          clippy
          rustfmt
          rust-analyzer
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
            pkgs.cargo
            pkgs.rustc
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
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.nodejs_22
            pkgs.pnpm
            pkgs.pnpmConfigHook
          ];
          pnpmDeps = pkgs.fetchPnpmDeps {
            pname = "liber-frontend";
            inherit version;
            src = ./frontend;
            fetcherVersion = 4;
            hash = "sha256-728DkKKUaN/HfAYK9U7RZAVh0EaUilySiRsNh69r8ZI=";
          };
          pnpmRoot = "frontend";
          env.EMBED_UI = "1";
          preBuild = ''
            (cd frontend && pnpm build)
          '';
        };

        # Default Linux dev+test shell. This is the ONLY shell the agent
        # tests in. Cross helpers are installed here so availability is
        # guaranteed, but cross/android builds are not executed by default.
        # Extra Rust targets (windows/macos/android std) are provided via
        # rustup: `rustup toolchain install stable --target <triple>`.
        devShells.default = pkgs.mkShell {
          buildInputs =
            rustToolchain
            ++ (with pkgs; [
              cargo-nextest
              cargo-audit
              cargo-zigbuild
              cargo-xwin
              cargo-ndk
              rustup
              zig
              nodejs_22
              pnpm
              sqlite
              pkg-config
            ])
            ++ linuxTauriDeps;
          shellHook = ''
            export RUSTUP_HOME="$HOME/.rustup-liber"
            echo "liber-rs dev shell (Linux test-only)"
            echo "cargo $(cargo --version 2>/dev/null || echo missing)"
            echo "Extra targets via: rustup toolchain install stable --target <triple>"
            echo "Android work: use .#android or .#android-fhs instead"
          '';
        };

        # Cross-compile helper shell (Windows/macOS toolchains from Linux).
        devShells.cross = pkgs.mkShell {
          buildInputs =
            rustToolchain
            ++ (with pkgs; [
              zig
              cargo-zigbuild
              cargo-xwin
              rustup
              pkgsCross.mingwW64.stdenv.cc
            ]);
          shellHook = ''
            export RUSTUP_HOME="$HOME/.rustup-liber"
            echo "liber-rs cross shell: cargo zigbuild / cargo xwin available"
            echo "macOS outputs are compile-check only (no signing)."
          '';
        };

        # Android wrapper toolchain: JDK, Gradle, SDK+NDK, Tauri Mobile deps.
        # Assemble via Gradle/Tauri (no network in nix builds), so nix is the
        # tool source, not the packager.
        devShells.android = pkgsAndroid.mkShell {
          buildInputs = with pkgsAndroid; [
            cargo
            rustc
            jdk17
            gradle
            androidSdk.androidsdk
            androidSdk.platform-tools
            nodejs_22
            cargo-ndk
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
