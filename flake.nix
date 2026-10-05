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
        # Versions must cover what every Gradle module demands:
        # our app pins compileSdk 36 (stable channel has no android-37,
        # and the Tauri library modules already target 36) with
        # build-tools 36.0.0; 37.0 is spare. Otherwise Gradle tries
        # to download into the read-only store and fails.
        androidSdk = pkgsAndroid.androidenv.composeAndroidPackages {
          platformVersions = [ "36" "37.0" ];
          buildToolsVersions = [ "36.0.0" ];
          includeNDK = true;
        };
        androidFhsEnv = pkgsAndroid.buildFHSEnv {
          name = "liber-rs-android-env";
          targetPkgs = pkgs: [
            # rustup, not nixpkgs cargo/rustc: the two collide on
            # bin/cargo plus bin/rustc, and only rustup can add the
            # Android std targets. Shares RUSTUP_HOME with the other
            # shells, so one stable toolchain serves everywhere.
            pkgs.rustup
            # Host C toolchain for build scripts and proc macros: cargo
            # recompiles everything under a fresh target dir here, and
            # the FHS chroot brings no compiler on its own (unlike a
            # regular mkShell, which always has stdenv.cc).
            pkgs.stdenv.cc
            pkgs.pkg-config
            pkgs.jdk17
            pkgs.gradle
            androidSdk.androidsdk
            androidSdk.platform-tools
            pkgs.nodejs_22
            # pnpm is a separate package: nodejs alone only brings node,
            # and the tauri beforeDev/beforeBuild hooks shell out to it.
            pkgs.pnpm
            pkgs.glibc
            pkgs.zlib
            pkgs.stdenv.cc.cc.lib
            pkgs.which
          ];
          runScript = pkgs.writeShellScript "enter-liber-rs-android-env" ''
            export ANDROID_HOME="${androidSdk.androidsdk}/libexec/android-sdk"
            export ANDROID_SDK_ROOT="$ANDROID_HOME"
            export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk-bundle"
            export ANDROID_NDK_HOME="$ANDROID_NDK_ROOT"
            export JAVA_HOME="${pkgsAndroid.jdk17}/lib/openjdk"
            export RUSTUP_HOME="$HOME/.rustup-liber"
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
            pkgs.makeWrapper
          ];
          pnpmDeps = pkgs.fetchPnpmDeps {
            pname = "liber-frontend";
            inherit version;
            src = ./frontend;
            fetcherVersion = 4;
            hash = "sha256-VRR+Ky196ZIAkB0VeuzKgdQtPDCBNOUa+CC0Qk1zucU=";
          };
          pnpmRoot = "frontend";
          env.EMBED_UI = "1";
          # Runtime archiver backends on PATH. Never add these (in
          # particular the npm-based single-file-cli) to
          # nativeBuildInputs: its setup hook pollutes the Node env and
          # breaks the offline pnpm build (esbuild optional dep goes
          # missing). Wrapping only touches installed binaries.
          postFixup = ''
            for bin in $out/bin/liber $out/bin/liber-serve; do
              wrapProgram "$bin" \
                --prefix PATH : "${pkgs.lib.makeBinPath [ pkgs.single-file-cli pkgs.monolith ]}"
            done
          '';
          cargoBuildFlags = [
            "--bin"
            "liber"
            "--bin"
            "liber-serve"
          ];
          cargoTestFlags = [
            "-p"
            "liber-cli"
            "-p"
            "liber-server"
          ];
          preBuild = ''
            (cd frontend && pnpm build)
          '';
        };

        # Desktop app: the liber-tauri binary with GApps wrapping for the
        # webkit runtime, plus a .desktop entry and icons. The Tauri
        # .deb/.rpm bundler does not run here; this is the native Nix way
        # to install the same app. Needs a display to run, not to build.
        packages.liber-desktop = pkgs.rustPlatform.buildRustPackage {
          pname = "liber-desktop";
          inherit version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          buildInputs = linuxTauriDeps;
          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.wrapGAppsHook3
            pkgs.nodejs_22
            pkgs.pnpm
            pkgs.pnpmConfigHook
            pkgs.makeWrapper
          ];
          # Same runtime-PATH rule as packages.default: external archiver
          # backends are wrapped in, never build inputs. Runs after the
          # GApps wrap, wrapping the wrapper is supported.
          postFixup = ''
            wrapProgram "$out/bin/liber-tauri" \
              --prefix PATH : "${pkgs.lib.makeBinPath [ pkgs.single-file-cli pkgs.monolith ]}"
          '';
          pnpmDeps = pkgs.fetchPnpmDeps {
            pname = "liber-frontend";
            inherit version;
            src = ./frontend;
            fetcherVersion = 4;
            hash = "sha256-VRR+Ky196ZIAkB0VeuzKgdQtPDCBNOUa+CC0Qk1zucU=";
          };
          pnpmRoot = "frontend";
          cargoBuildFlags = [
            "--bin"
            "liber-tauri"
          ];
          cargoTestFlags = [
            "-p"
            "liber-core"
          ];
          preBuild = ''
            (cd frontend && pnpm build)
          '';
          postInstall = ''
            mkdir -p $out/share/applications $out/share/icons/hicolor/32x32/apps $out/share/icons/hicolor/128x128/apps
            cat > $out/share/applications/liber.desktop <<EOF
            [Desktop Entry]
            Type=Application
            Name=liber
            Comment=Local-first bookmark manager
            Exec=liber-tauri
            Icon=liber
            Categories=Utility;
            Terminal=false
            EOF
            cp crates/liber-tauri/icons/32x32.png $out/share/icons/hicolor/32x32/apps/liber.png
            cp crates/liber-tauri/icons/128x128.png $out/share/icons/hicolor/128x128/apps/liber.png
          '';
        };

        # All-in-one release tarball: CLI, server, and desktop binaries
        # plus checksums and install notes. The web UI ships embedded in
        # liber and liber-serve, so no separate bundle is staged.
        packages.release-bundle =
          let
            cli = self.packages.${system}.default;
            desktop = self.packages.${system}.liber-desktop;
          in
          pkgs.runCommand "liber-rs-${version}-${system}"
            {
              nativeBuildInputs = [ pkgs.gnutar ];
            }
            ''
              mkdir -p bundle/liber-rs-${version}
              cp ${cli}/bin/liber ${cli}/bin/liber-serve bundle/liber-rs-${version}/
              cp ${desktop}/bin/.liber-tauri-wrapped bundle/liber-rs-${version}/liber-tauri
              cat > bundle/liber-rs-${version}/INSTALL.txt <<EOF
              liber-rs ${version} (${system})
              Run from this directory or add it to PATH:
                ./liber --help                 CLI bookmark manager
                ./liber-serve --help           standalone server (API plus web UI)
                ./liber-tauri                  desktop app (needs a display plus system webkit)
              Verify with: (cd liber-rs-${version} && sha256sum -c SHA256SUMS)
              EOF
              (cd bundle/liber-rs-${version} && sha256sum liber liber-serve liber-tauri > SHA256SUMS)
              (cd bundle && tar -czf $out liber-rs-${version})
            '';

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
              # External archiver backends for testing browser/single-file/
              # monolith paths. Interactive shell only: online pnpm tolerates
              # the extra Node env, the offline package build does not.
              single-file-cli
              monolith
            ])
            ++ linuxTauriDeps;
          shellHook = ''
            export RUSTUP_HOME="$HOME/.rustup-liber"
            echo "liber-rs dev shell (Linux test-only)"
            echo "cargo $(cargo --version 2>/dev/null || echo missing)"
            echo "Extra targets via: rustup toolchain install stable --target <triple>"
            echo "Android work: use .#android or .#android-fhs instead"
            echo "Desktop: app serves the embedded bundle (pnpm build, then rebuild the binary); tauri dev needs a display"
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
            # Same swap as the FHS env below: rustup over nixpkgs
            # cargo/rustc (file collision), targets via rustup.
            rustup
            jdk17
            gradle
            androidSdk.androidsdk
            androidSdk.platform-tools
            nodejs_22
            pnpm
            cargo-ndk
          ];
          ANDROID_HOME = "${androidSdk.androidsdk}/libexec/android-sdk";
          ANDROID_SDK_ROOT = "${androidSdk.androidsdk}/libexec/android-sdk";
          shellHook = ''
            export RUSTUP_HOME="$HOME/.rustup-liber"
            export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk-bundle"
            export ANDROID_NDK_HOME="$ANDROID_NDK_ROOT"
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
        apps.liber-desktop = flake-utils.lib.mkApp {
          drv = self.packages.${system}.liber-desktop;
          exePath = "/bin/liber-tauri";
        };
      }
    );
}
