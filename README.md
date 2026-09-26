
A simple cross-platform manager, operated through CLI or web UI, that
saves bookmarks as plain browsable HTML files, optionally with web
archives, markdown notes, or file attachments. Your collection stays
fully browsable and usable without liber itself. For more details visit the documentation site at [liber-bkm.github.io](https://liber-bkm.github.io)
# Features
- Plain HTML bookmarks
- Web page archives, markdown notes and file attachments
- Tags and directories
- Fully functional CLI and web UI
- Duplicate detection
- Link checker (finds stale or outdated bookmarks)
- Powerful search: scoped fields plus full-text search inside archives
- Configurable bookmark directories and paths
- Import and export bookmarks
- Automation rules to classify bookmarks into tags or folders
- Self-hosting and sync friendly, with merge conflict resolution
- Profiles

# Installation
> [!Note]
> liber fully functions as a standalone app, but some quality-of-life
> features need optional third-party tools (fzf for live search,
> single-file or monolith for full archives, git for syncing). Fallbacks
> are built in when they are missing.

## Arch Linux
```sh
sudo pacman -S fzf # official repos
paru -S single-file-cli # or install single-file/monolith your own way
wget https://github.com/liber-bkm/liber/releases/latest/download/PKGBUILD
makepkg -si
# or PKGBUILD-git for the latest master branch instead
```

## NixOS
Add this repo to your flake inputs
```nix
inputs = {
	liber.url = "github:liber-bkm/liber";
	liber.inputs.nixpkgs.follows = "nixpkgs";
};
```
and install it on your system using `environment.systemPackages` or using home-manager `home.packages`
```nix
{config, lib, pkgs, inputs, ... }; {
	environment.systemPackages = [
		inputs.liber.packages.${stdenv.hostPlatform.system}.default
	];
}
```
You can also run liber directly without installing 
```sh
nix shell github:liber-bkm/liber # enter shell with liber present
# or 
nix run github:liber-bkm/liber # run liber commands directly 
```

## Linux binary
Download the binary for your architecture from the
[latest release](https://github.com/liber-bkm/liber/releases/latest/)
and place it in your PATH (`~/.local/bin`).

## Windows
Download `liber-setup.exe` from the
[latest release](https://github.com/liber-bkm/liber/releases/latest/).
For terminal use, Windows Terminal or
[wezterm](https://wezterm.org/index.html) is recommended; otherwise run
`liber --serve` and use it in the browser.

## macOS
Install fzf
```sh
brew install fzf
```
and single-file-cli
```sh
curl -L "https://github.com/gildas-lormeau/single-file-cli/releases/latest/download/single-file-aarch64-apple-darwin" -o ~/.local/bin/single-file # for Apple silicon 
# or 
curl -L "https://github.com/gildas-lormeau/single-file-cli/releases/latest/download/single-file-x86_64-apple-darwin" -o ~/.local/bin/single-file # for Apple intel
```
Download `liber-darwin-arm64` (Apple silicon) or `liber-darwin-amd64`
(Intel) from the
[latest release](https://github.com/liber-bkm/liber/releases/latest/),
rename it to `liber`, and place it in `~/.local/bin`.

## Android
liber runs natively on Android, standalone or as a client for a
self-hosted server. Download the APK from the
[latest release](https://github.com/liber-bkm/liber/releases/latest/).

## Build from source
Requires Go:
```sh
git clone https://github.com/liber-bkm/liber.git
cd liber
go build -o liber . # or: make && sudo make install
```

# Usage
Full guides with examples can be found at
[documentation](https://liber-bkm.github.io/). Everyday commands:

```
liber <url>          save a bookmark
liber <url> -i       save interactively (description, tags, folder)
liber -s             search and browse (fzf when available)
liber -o <id|query>  open a bookmark
liber -l             list all bookmarks with ids
liber -e <id>        edit a bookmark
liber -d <id>        delete a bookmark (asks first)
liber --import <f>   import a browser bookmark export
liber --serve        local web UI in your browser
```

Topics: [search](https://liber-bkm.github.io/guide/searching) ·
[editing](https://liber-bkm.github.io/guide/editing) ·
[tags and folders](https://liber-bkm.github.io/guide/tags-folders) ·
[automation](https://liber-bkm.github.io/guide/automation) ·
[link health](https://liber-bkm.github.io/guide/link-health) ·
[import](https://liber-bkm.github.io/guide/import) ·
[profiles](https://liber-bkm.github.io/guide/profiles) ·
[sync](https://liber-bkm.github.io/guide/sync) ·
[web UI](https://liber-bkm.github.io/guide/web-ui)

# Configuration
Use the web UI or edit `~/.config/liber/config.json` (use `liber config` command
for the active path). Example:

```json
{
  "base_dir": "/home/you/Bookmarks",
  "singlefile_cmd": "single-file",
  "archive_backend": "single-file"
}
```

All keys are documented at
[liber-bkm.github.io/config](https://liber-bkm.github.io/config).

