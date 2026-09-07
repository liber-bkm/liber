


<img width="800" height="450" alt="liber-cli-ezgif com-video-to-gif-converter" src="https://github.com/user-attachments/assets/a8f68f77-e7c8-48c6-8bf3-4e2df584bcbb" />
<img width="800" height="450" alt="liber-webui-ezgif com-video-to-gif-converter" src="https://github.com/user-attachments/assets/08e04fdd-8ada-4872-92a4-4c2309bca375" />

> [!Tip]
> If this readme feels too long visit [liber-bkm.github.io](https://liber-bkm.github.io/start/quickstart/) for structred guide, detailed usage examples. The site covers everything including what readme has missed.

- [Introduction](#introduction)
- [Features](#features)
- [Installing](#installing)
	- [Dependencies](#dependencies)
	- [Arch Linux](#arch-linux)
	- [NixOS](#nixos)
	- [Build from Source](#build-from-source)
	- [Windows](#windows)
	- [MacOS](#macos)
- [Usage](#usage)
	- [Usage Overview](#usage-overview)
	- [Detailed Usage](#detailed-usage)
- [Configuration](#configuration)
	- [Layout Example](#layout-example)
	- [Profiles](#profiles)
	- [Reindexing](#reindexing)
- [Design notes](#design-notes)
	- [Why tags and folders both](#why-tags-and-folders-both)
	- [Markdown Copy](#markdown-copy)
	- [Why not a Database for Indexing](#why-not-a-database-for-indexing)

# Introduction

Liber is a cross-platform, simple, private and local bookmark manager that saves bookmarks as browsable plain-text HTML files, optionally archives webpages and writes a markdown copy for bookmark-specific notes, with configurable directories. It can work in CLI or as webUI that is simple yet efficient to work in the browser. It uses a simple JSON index (to avoid depending on a database) to manage bookmarks behind the scenes.

<h2 style="text-align: center;">Available For</h2>
<div style="display: flex; gap: 20px;"> <div style="flex: 1;" align= center >Linux</div> <div style="flex: 1;" align= center >MacOS</div> <div style="flex: 1;" align= center>Windows</div> </div>

# Features

- Plain-text HTML bookmarks
- Tags and directories, see [Design notes](#design-notes)
- Markdown copy of bookmarks for personal notes and additional description
  - You can simply store them as HTML and later create a markdown copy or archive when editing a bookmark
- Web UI in case you don't want to work with cli
- Duplicate detection (tags, folder, and bookmarks)
- Full archive of webpages (requires `single-file-cli`)
- Attach files or related content to bookmarks
- Check whether a bookmark has an archive, markdown copy, or attachments
- Powerful search and edit: bulk management, deep search, missing archives
- Configurable location
- Import bookmarks from browser
- Rule-based automation of bookmarks into specific folders or tags
- Git integration for history and sync
- Profiles, each working independently from the others

# Installing

## Dependencies

### Runtime dependencies

Liber itself has no dependencies, but some features require:

- [fzf](https://github.com/junegunn/fzf) fuzzy finder, for live search
- [single-file-cli](https://github.com/gildas-lormeau/single-file-cli) for full web page archive
	- or [monolith](https://github.com/y2z/monolith)
- `git` for history and syncing

### Build dependencies

To build liber from source you only need:

- Go (version 1.22 or later)

## Arch Linux

- Install the optional dependencies above
- Download the package build from [latest releases](https://github.com/liber-bkm/liber/releases/latest/download/PKGBUILD) and install it, or simply:

```sh
wget https://github.com/liber-bkm/liber/releases/latest/download/PKGBUILD
makepkg -si
```

## NixOS

This repo provides a flake to install it on your NixOS system.

- Add the repository to your flake inputs:

  ```nix
  inputs = {
    liber.url = "github:liber-bkm/liber";
    liber.inputs.nixpkgs.follows = "nixpkgs";
  };
  ```

  and install it on your system with `environment.systemPackages` or with home-manager `home.packages`:

  ```nix
  { config, pkgs, inputs, ... }: {
    home.packages = [
      inputs.liber.packages.${pkgs.system}.default
      single-file-cli # optional
      fzf              # optional
    ];
  }
  ```

  You can also try it on the go:

  ```sh
  nix shell github:liber-bkm/liber   # enter a shell with liber
  # or run the binary directly
  nix run github:liber-bkm/liber
  ```

> [!Note]
> The Nix and Arch builds don't ship with optional dependencies; you'll have to install/declare them on your own.


## Build from Source

Clone this repo and enter the directory:

```sh
git clone https://github.com/liber-bkm/liber.git
cd liber
```

Use either `go build` or the Makefile, not both.

With `go`:

```sh
go build -o liber .
sudo mv liber /usr/bin/   # add it to your PATH
```

or with the Makefile:

```sh
make && sudo make install
```

Make sure to install the optional dependencies if you want the optional features.

## Generic Linux install
- Download the binary from latest releases or directly from [latest release](https://github.com/liber-bkm/liber/releases/latest/) based on your architecture 
- Place the binary in your PATH:
- It is recommended to build from source instead of using prebuild binaries 
```/usr/local/bin # or  ~/.local/bin```




## Windows

- Download the `liber-setup.exe` from the [latest releases](https://github.com/liber-bkm/liber/releases/latest) or use the [direct download link](https://github.com/liber-bkm/liber/releases/latest/download/liber-setup.exe)
- Install it as you would any other `.exe` file
- Open a terminal and you can start using liber. If you plan to use liber on the terminal extensively on Windows instead of the web UI (`liber --serve`), I'd recommend using Windows Terminal or [wezterm](https://wezterm.org/index.html).
- In terminal run `liber config` to find the config directory and set it up as you see fit. See [Configuration](#configuration)
- In windows make sure to set `singlefile_browser_path` to something like `firefox.exe` or `C:\Program Files...` accordingly.
	- Or in case of monolith use `monolith_browser_path`
- Although it should work by default but you may have to set `singlefile_cmd` to `single-file.exe` if it doesn't work by default.
	- Or in case of monolith use `monolith_cmd`

## MacOS

- It is recommended to compile liber but you can download binary from [latest release](https://github.com/liber-bkm/liber/releases/latest).
- Install `fzf` and `single-file-cli` (either from their respective GitHub repos or via Homebrew)
- Clone the repository or download the source tarball from the latest release
- Build using `go` and place the binary in your `PATH`

# Usage

## Usage Overview

If you don't want to go through every usage detail, here is the TLDR:

```
liber <url>                    save a bookmark
liber <url> -i                 save interactively (prompts for description, tags, folder)
liber <url> -md                also write a markdown copy
liber <url> -a                 also write a full-page archive (backend set via
                                config archive_backend: single-file / monolith / native)
liber <url> -md -a             both markdown and archive
liber <url> -t tag-a tag-b     attach tags at creation time
liber <url> -f subfold         save into a subfolder of the base directory
liber <url> -at report.pdf     attach a local file (repeatable; see "Attachments")
liber -o 3                     open bookmark id 3 
liber -o 1,3-8                 open bookmark at ids 1 and 3 to 8
liber -o <query>               find by search text (one match opens, several offer a pick)
liber pick <query>             print the URL to stdout, for pipes and scripts
liber -s                       search/browse bookmarks, open or edit them (fzf if available)
liber -sn / -su / -st / -sd / -sf
                                same, but scoped to one field: title / url / tags / description / folder
liber -sdf                     fields combine freely, e.g. this is folder+description only
liber -sl                      force the plain prompt even if fzf is installed
liber -sld                     legacy prompt scoped to descriptions (mix -l with any of n/u/t/d/f)
liber -s --deep / -sl --deep   also full-text search inside archived pages (see "Deep search")
liber -l                       list all bookmarks with their ids
liber -e <id>                  edit a bookmark interactively (also offers to add a
                                markdown copy or archive if either is missing)
liber -e <id> -t tag-a tag-b   set a bookmark's tags directly (non-interactive)
liber -e <id> -f subfold       move a bookmark to a different folder (non-interactive)
liber -e <id> -md              add a markdown copy if it doesn't have one yet
liber -e <id> -a               add an archive if it doesn't have one yet
liber -e <id> -u <new url>     edit url of specific id
liber -e <id> -at report.pdf   attach a file to an existing bookmark (repeatable)
liber -e <id> -dt report.pdf   detach by name or number (deletes the saved copy)
liber -e <ids> ...             <id> can be a range/list too: 1-3, 2,5,3, or 1-4,7-9 --
                                applies the same flags (or interactive edit, one at a
                                time) to each matched bookmark; see "Batch operations"
liber -d <id>                  delete a bookmark (asks for confirmation)
liber -d <id> -y               delete without confirmation
liber -d <ids>                 <id> can be a range/list too, same as -e above
liber -r                       reindex: clean up + renumber (see "Reindexing" below)
liber --import <path>          import a browser bookmark export (see "Import" below)
liber --import <path> -md -a   same, also generating markdown/archives for each (slow)
liber --tags / --folders       list tags/folders with counts (see "Tag and folder hygiene")
liber --tags rename <a> <b>    rename a tag everywhere (merges if <b> already exists)
liber --tags delete <tag>      remove a tag from every bookmark that has it
liber --folders rename <a> <b> rename a folder (and its subfolders) everywhere
liber --folders delete <f>     move a folder's bookmarks back to the root
liber --history                list bookmarks by most recently opened (see "History")
liber --check [ids]            check link health, then prompt per item (see "Link health")
liber --check --workers N      same, with N parallel requests (default 12)
liber --auto add --match <s> --folder <f> --tag <t1 t2>
                                auto-classify new bookmarks by URL (see "Automation")
liber --auto / --auto edit / --auto delete / --auto apply
                                list/edit/delete/re-run automations (see "Automation")
liber --auto learn [--min N] [--create]
                                suggest host rules from folder clusters
liber --profile                list profiles, with the active one marked (see "Profiles")
liber --profile <name>         switch to <name>, creating it if it's new
liber --profile default        switch back to the non-profile layout
liber --profile delete <name>  stop tracking a profile (its data is untouched)
liber --sync / --sync -p       commit (and optionally push) if it's a jj/git repo (see "Sync")
liber config                   show the active config file and its path
liber -v                       print the version
liber --serve                  local web UI for search + add (see "Web UI")
liber --serve --addr <host:port>
                                use a different address (default 127.0.0.1:8080)
liber --export-site             # export to a static html 
liber --export-site <path>      # export to a specific path

```

## Detailed Usage

This section provides detailed usage and examples.

By default liber stores bookmarks in a folder named `Bookmarks` created inside your home directory. You can run `liber` with no arguments to list usage flags.

- **Create a bookmark**

  ```sh
  liber <url>
  ```

- **Create a bookmark interactively**

  ```sh
  liber <url> -i
  ```

  This opens prompts to edit the title name, add tags, and put the bookmark in a specific subdirectory. There are individual commands for quick tag or folder edits (listed below), but this command lets you interact during bookmark creation.

- **Create a bookmark along with an archive and/or markdown copy**

  ```sh
  liber <url> -md   # for markdown copy
  liber <url> -a    # for archiving
  ```

  This creates a markdown copy and a webpage archive for that URL. Liber keeps archives and markdown copies in their respective folders under the base bookmarks folder. The archive, markdown, and bookmark are indexed and point to each other. For example, a bookmark of `google.com` would have `indexnumber-google.com.html` in the `html` folder, while the archive and markdown would live in the `archive` and `markdown` folders respectively with the same index. The index is dynamic and points to the bookmarks correctly; even if one file is deleted, the index rearranges them accordingly.

  Archiving picks a backend by configuration: by default `auto` tries `single-file` first (heaviest, most faithful), then `monolith` (fast, no browser needed), then the built-in `native` snapshot (plain Go, no dependencies, no JavaScript). See `archive_backend` in [Configuration](#configuration).

- **Create a bookmark with specific tags or in a specific subdirectory**
  ```sh
  liber <url> -t tag-a tag-b   # use space to separate tags
  liber <url> -f folder-name   # subfolder the bookmark goes into
  ```

- **Search**
  ```sh
  liber -s
  ```

  This flag lets you search bookmarks interactively. If you have fzf, by default it opens a live preview; otherwise a simple plain-text prompt. By default the search includes everything, i.e. title, URL, tags, and folder (the plain prompt also searches description by default). You can limit to a specific field with:

  ```sh
  liber -sn   # title or name
  liber -su   # url
  liber -st   # to search bookmarks with specific tags
  liber -sf   # bookmarks in a specific folder
  liber -sd   # description
  # search flags can be combined:
  liber -sfn  # for folders and titles
  ```

  It falls back to the plain legacy search if you don't have fzf, or you can force it with:

  ```sh
  liber -sl
  # or combine:
  liber -sln  # title or name
  liber -slu  # url
  liber -slt  # tags
  liber -sld  # description
  liber -slf  # folders
  # search flags can be combined:
  liber -sldf # for folders and descriptions
  ```

  Either way, picking a bookmark drops you into the open / edit / delete menu.
  The menu can open the live page `(o)`, the saved card `(c)`, the markdown
  copy `(m)`, or the archive `(a)`.

  Liber can also perform deep search to look for text inside archived pages:

  ```sh
  liber -s --deep
  ```

  `--deep` follows the criteria of `-s`; any search flag like `s`, `sn`, `sl`, and so on, can be followed by `--deep`.

- **List** all bookmarks with their id or index

  ```sh
  liber -l
  ```

- **Edit** a bookmark (name, tag, folder, description, or URL):

  ```sh
  liber -e <id>
  ```

  This lets you edit a bookmark of the given id. You can also edit bookmarks during search: selecting a bookmark during `liber -s` opens a prompt asking whether to edit. It also lets you create a web archive or markdown copy of a specific bookmark if they aren't present. Or you can edit something specific directly:

  ```sh
  liber -e <id> -t tag-a tag-b   # creates tag-a and tag-b for that bookmark
  liber -e <id> -f folder-a      # moves the bookmark to folder-a
  ```

- **Add markdown or archive** of a bookmark that doesn't have one:

  ```sh
  liber -e <id> -md   # for a markdown copy
  liber -e <id> -a    # for an archive
  ```

  > [!Note]
  > The `<ids>` can be a range as well. For example, `liber -e 1-3 -md` will create a markdown copy of bookmarks 1–3; it can also be a comma-separated list: `liber -e 1-5,7,9-11`.

- **Open a bookmark**
    ```sh 
    liber -o 3            # open bookmark 3 in the browser
    liber -o 1-3,7        # several at once (same range/list syntax as -e/-d)
    ```

- **Delete** a bookmark:

  ```sh
  liber -d <id>
  ```

  The search flag (`liber -s`) also opens a prompt to delete a bookmark. **Just like editing, the deletion ids can also be a range or comma-separated list.**
- **Edit Url**
  ```sh
  liber -e <id> -u <new-url>       # non-interactive (works with id ranges)
  liber -e <id>                    # interactive edit also prompts for the URL
  ```

 **Attachments**

  ```sh
  liber <url> -at example-file   # attach example-file to the related bookmark
  liber -e <id> -at example-file # attach example-file to the selected bookmark
  liber -e <id> -dt example-file # remove attachment
  ```

  Liber lets you attach files to bookmarks with the `-at` flag and remove them with the `-dt` flag. Useful for associating multiple archives related to a bookmark, or other related files.

- **Show config and base directories:**
  ```sh
  liber config
  ```
  This shows where your config is and where bookmarks are stored.
- **Import:**
  ```sh
  liber --import <path>
  ```

  Liber can import bookmarks from a browser's exported bookmarks file where `<path>` is the location of that file. See details in the [Importing Bookmarks](#importing-bookmarks) section below.

- **Tags and folder management:**
  ```sh
  liber --tags                     # list all tags and the bookmarks in them (count)
  liber --folders                  # list all folders
  liber --tags rename <a> <b>      # rename tag a to b; merge a into b if b already exists
  liber --tags delete <tag>        # delete a tag
  liber --folders rename <a> <b>   # same as tags
  liber --folders delete <folder>  # delete folder and move its bookmarks to root
  ```

  There's no separate "merge" command; renaming _onto_ a name that already exists **is** the merge: if a bookmark already has both the old and new tag, the rename just drops the old one rather than creating a duplicate. The same idea applies to folders (renaming `work` to `personal` when `personal` already has bookmarks just combines them). Folder rename/delete affects subfolders too (`work/urgent` follows `work` when you rename or delete it) and physically moves the affected files, the same as editing a single bookmark's folder does. Tag rename/delete rewrites the affected HTML/markdown files in place so their content stays consistent with the index.

- **History:**

  ```sh
  liber --history   # list bookmarks by most recently opened
  ```

- **Link health:**

  ```sh
  liber --check               # scan all bookmarks, then prompt per flagged item
  liber --check 1-100,200     # same, limited to an id range (idspec syntax)
  liber --check --workers 20  # more parallel requests (default 12)
  ```

  Results fall into three buckets: `moved` (permanent redirect, proposes a URL update), `dead` (404/410 from the site itself, proposes deletion), and `uncertain` (timeouts, DNS/TLS errors, 403/429/503, blocks, anything ambiguous: shown for review, never auto deleted). Every destructive action asks first and defaults to no, except URL updates which default to yes. Scans use a browser-like user agent with a 15s timeout per request.

- **Open by name and pick:**

  ```sh
  liber -o "some query"   # one match opens directly, several offer a picker
  liber pick "some query" # print the URL to stdout (for pipes and scripts)
  ```

  Both use the full-scope search. `pick` keeps stdout clean (prompts go to stderr) and exits non-zero when nothing matches.

- **Automation / Rules**, see the dedicated section [below](#automation):

  ```sh
  liber --auto add --match <s> --folder <f> --tag <t1 t2>
  # auto-classify new bookmarks whose URL contains <s>

  liber --auto edit    # edit rules
  liber --auto delete  # delete rules
  liber --auto apply   # apply rules
  liber --auto learn   # suggest host rules from folder clusters
  ```

  `learn` groups bookmarks by host and suggests a `host:` rule wherever 3 or more share one folder (tune with `--min N`). It only suggests by default and asks per rule; `--create` creates all without asking. Hosts already covered by a rule are skipped.

- **Sync:**

  ```sh
  liber --sync     # commit if the bookmarks directory is a git repo
  liber --sync -p  # git push
  ```

- **Profiles:** Liber also has profiles. By default there's just one collection, living directly under `base_dir`. If you want separate, fully independent collections — say, `work` and `personal` — profiles give you that. See details [here](#profiles).

  ```sh
  liber --profile                # list profiles, with the active one marked
  liber --profile <name>         # switch to <name>, creating it if it's new
  liber --profile default        # switch back to the non-profile layout
  liber --profile delete <name>  # stop tracking a profile (its data is untouched)
  ```

- **WebUI:**

  ```sh
  liber --serve
  liber --serve --addr 127.0.0.1:8181   # use a specific address
  ```
  This opens a web UI to manage bookmarks. It allows editing, searching, and adding bookmarks in a browser window. By default it uses port `8080` of `localhost`.

- **Static site export**
```sh
liber --export-site            # writes <base_dir>/site/index.html
liber --export-site /tmp/site  # or any directory
```
Generates a single browsable index.html of the whole collection, grouped by folder, each entry linking to the bookmark’s HTML file with badges for markdown, archive, and attachments. Links are relative (../html/...), so the generated page works straight off disk or dropped onto any static file host next to the collection.

The export is regenerable output, not data — rerun after changes. Nothing in the site/ directory is managed or reindexed; delete it freely.

> [!Note]
> The flags can be combined. For example, `liber <url> -t tag-a -f folder-n` or `liber <url> -md -a` or `liber <url> -i -t news reading -f articles -md -a`.

### Shell completions 
```sh
liber completion bash  > ~/.local/share/bash-completion/completions/liber
liber completion zsh   > ~/.zsh/completions/_liber        # add dir to $fpath, then compinit
liber completion fish  > ~/.config/fish/completions/liber.fish

```
The scripts complete all commands and flags, and fetch bookmark ids, tag names, and folder names live from liber itself (liber -l, liber --tags, liber --folders), so they never go stale and always reflect the active profile.

### Indexing and Reindexing

Liber indexes bookmark ids in a simple JSON file. They are the ids that liber uses to identify and sync bookmarks, their markdown, archive, and attachment copies. If you delete a bookmark, the empty index slot remains, and adding further bookmarks proceeds without any problems; but if you want to close the id gaps you can use:

```sh
liber -r
```

to reindex the bookmarks list. It straightens the index and syncs up copies. It also checks for mismatched copies. For example, if a bookmark is deleted and its markdown or archives aren't (deleted directly in the path by the user outside of liber), liber moves the mismatched copies to an `unindexed` folder. This way you won't lose archives even if you delete anything in folders. Details explained below under Configuration.

### Importing Bookmarks

`liber --import <path>` reads a browser bookmark export. Folders in the export become folders in liber (nested folders become `Parent/Child`); Firefox's per-bookmark `TAGS` and description are picked up too. Each imported bookmark gets a normal HTML file, exactly as if you'd run `liber <url>`, and you can pass `-md`/`-a` to also generate markdown/archives for every import, though for a large export that's slow (archiving in particular makes one `single-file` call per bookmark) and probably better done selectively afterward with `liber -e <id> -md`/`-a`.

Anything that normalizes to a URL you already have is skipped automatically (no per-item prompt, unlike adding one bookmark at a time). So re-running `--import` on a refreshed export from your browser won't pile up duplicates. Entries with no `HREF` are skipped too. Both counts are reported at the end.

### Sync

`liber --sync` looks for a `.jj` or `.git` directory at or above `base_dir` and, if it finds one, commits the current state of your collection there (`liber --sync -p` also pushes afterward). It never initializes a repo itself. If there isn't one, it tells you and stops, since creating one unasked would be a strange thing for a bookmark tool to do. If `base_dir` is nested inside a larger repo (e.g. a dotfiles checkout), it still finds the right root.

This is deliberately minimal: one commit, optionally one push, nothing that manages branches, jj bookmarks, or remotes for you. Since everything liber writes is flat files and JSON, git or jj sync was already going to work without this command; `--sync` just saves you the two-or-three manual commands.

### Automation

Auto-classify bookmarks whose URL contains a given string. Also works to specify based on host/site or title. You can either user webUI or cli for automations, for cli consider the examples below:

```sh
liber --auto add --match doxy --folder hot
liber --auto add --match doxy --folder hot --tag important urgent
liber --auto                                  # list automations, with how many bookmarks each has classified
liber --auto edit <id> --folder other-folder  # change what a rule does
liber --auto edit <id> --folder x --reapply   # change it AND re-sync bookmarks it already classified
liber --auto delete <id>                      # remove a rule (doesn't undo what it already did)
liber --auto apply                            # re-run every rule against existing bookmarks
liber --auto apply <id>                       # re-run just one

liber --auto add --match host:github.com --folder code   # host only (port ignored)
liber --auto add --match "title:how to" --tag reference  # title only
liber --auto add --match doxy --folder hot               # URL, as before

```
```
host contains "host:github.com" -> folder "code"  (applied to 4 bookmark(s))
```
Everything from `doxy.com` should always land in a `hot` folder.



**Automation never overrides an explicit choice, and never reopens a decision it's already made.** Concretely:

- Creating a bookmark with `-f somefolder` (or importing one that already has a folder from your browser) always wins; a folder rule only ever fills in an _empty_ folder. Tags still get added either way, since tags are additive rather than exclusive.
- Adding a new rule immediately applies it to any existing bookmarks that match (bookmarks created before the rule existed still get classified), but only bookmarks that don't already have a folder. Anything already organized, by hand or by another rule, is left alone.
- **Once a bookmark has been classified (automatically or manually) and you later move it yourself, that move sticks.** Automation tracks which rules have already had their one chance at each bookmark, so re-running `--auto apply` or adding an unrelated new rule never revisits a decision that's already been made, including ones automation itself made earlier.
- Editing a rule updates its definition for future bookmarks; it does _not_ retroactively touch bookmarks it already classified unless you add `--reapply`. Even then, `--reapply` only advances a bookmark to the rule's new value if the bookmark's current folder still exactly matches what that same rule set it to last time; if you've moved it since, `--reapply` leaves it alone too.
- Deleting a rule removes the rule only; bookmarks it already classified keep their folder/tags exactly as they are.

### Web UI

`liber --serve` starts a local web UI at `http://127.0.0.1:8080`. It allows full configuration of liguration and bookmark management (search, edit, add, archive, ...)

> [!Tip]
> Add a bookmarklet to your browser's toolbar with this as the URL (adjust the port if you used `--addr`) to quickly bookmark whatever page you're currently on:
>
> ```
> javascript:location.href='http://127.0.0.1:8080/?prefill='+encodeURIComponent(location.href)
> ```

It reflects whichever profile is active, and even picks up a profile switch made via the CLI in another terminal on its next request — no restart needed.

Once a search's result count passes 500, simple `?page=N` pagination appears automatically (no controls at all below that). A scoped or deep search's page-forward/back links carry the same query along, so paging through a filtered search keeps it filtered. A filter box above the results narrows the shown rows instantly in the browser (client side only, works within any search).

Single attachments link straight to the file from the results list; bookmarks with several link to the edit page where each file opens individually.

The gear button (top right) opens `/settings`, a settings page for your collection. It shows which external tools liber detected on your machine (`single-file`, `monolith`, chromium for the browser pipe, your open/editor commands), shows the effective directories, and lets you override any of them, including `archive_backend` and `monolith_use_browser`. Changes are written straight to `config.json` and take effect immediately. The same page manages automation rules: add, edit (optionally with reapply), delete, and re-run all rules against existing bookmarks, the same things `liber --auto` does on the command line.

# Configuration

You can either use webUI for configuring liber or use config file / cli. 

On first run, liber writes a default config to:

```sh
$XDG_CONFIG_HOME/liber/config.json    # usually ~/.config/liber/config.json
```

The configuration is simple and lets you define your bookmarks directory and the `single-file` command (useful if you're using a different command name or path):

```json
{
  "base_dir": "/home/you/Bookmarks",
  "singlefile_cmd": "single-file"
  // "singlefile_browser_path": "firefox"
  // "monolith_cmd": "monolith"
  // "archive_backes": "single-file" or "monolith" or "native"
  // "monolith_use_browser": false,
  // "monolith_browser_path": "firefox"
}
```

Fields:

- `base_dir`: root of your bookmark collection.
- `html_dir` / `markdown_dir` / `archive_dir` / `attachment_dir`: override any of the four subdirectories individually; each defaults to `<base_dir>/html`, `<base_dir>/markdown`, `<base_dir>/archive`, `<base_dir>/attachments`.
- `singlefile_cmd`: the executable used for `-a` archiving (default `single-file`).
- `singlefile_browser_path`: browser executable handed to `single-file` as `--browser-executable-path` on every archive run (leave unset to let `single-file` find the browser itself). Useful when `single-file` can't locate e.g. Brave: `"singlefile_browser_path": "/usr/bin/brave"`.
- `archive_backend`: which archiver `-a` uses  `"single-file"`, `"monolith"`, `"native"`, or `"auto"` (the default). `auto` tries them in that order, skipping whichever binary isn't installed and falling back on failure; an explicit backend is strict and errors instead of falling back. `native` is built in and always available, so archiving never dead-ends.
- `monolith_cmd`: the [monolith](https://github.com/Y2Z/monolith) executable for the monolith backend (default `monolith`). Monolith needs no browser and is a good fit for headless machines; it can't render JavaScript-driven pages on its own.
- `monolith_use_browser` + `monolith_browser_path`: when `monolith_use_browser` is `true`, liber renders the page with a headless chromium-family browser (`monolith_browser_path`, else `chromium` / `chromium-browser` / `google-chrome` from `PATH`) and pipes the DOM into monolith:

  ```sh
  chromium --headless --dump-dom <url> | monolith - -I -b <url> -o <out>
  ```

  which archives even JavaScript-rendered pages without a full single-file setup.
- `browser_cmd`: override the command used by `liber -s`'s "open"/"archive" actions (defaults to `xdg-open` / `open` / the Windows shell handler, by OS).
- `editor_cmd`: override the command used by `liber -s`'s "markdown" action (defaults to `$VISUAL`, then `$EDITOR`, then the OS's default file association, in that order).

Run `liber config` to see the resolved paths.

>[!Warning]
>If liber is not archiving bookmarks, make sure all dependencies are installed and `"singlefile_browser_path"` or `monolith` options are set properly in `config.json` 

## Layout Example

```
<base_dir>/
  html/<folder>/0007-my-title.html
  markdown/<folder>/0007-my-title.md
  archive/<folder>/0007-my-title.html
  attachments/0007-paper.pdf
  .liber/index.json
```

Every bookmark's files are prefixed with its numeric id, so `liber -l` / `liber -s` results always line up with what's on disk. Editing a bookmark's folder moves its files; editing anything else just rewrites them in place. Editing (interactively, or with `-md`/`-a`) can also _add_ a markdown copy or archive that wasn't there before — it reuses the bookmark's original id-slug basename (taken from its HTML file) so the new file lines up with the others exactly, even if the title has changed since creation. It only ever adds what's missing — an existing markdown copy or archive is left alone, not regenerated.

## Profiles

A profile is just a subfolder of `base_dir`. `liber --profile work` makes `<base_dir>/work/` the effective base dir for everything (`html/`, `markdown/`, `archive/`, `attachments/`, `.liber/index.json`) until you switch again. Each profile has its own bookmarks, ids, tags, folders, and [automations](#automation), completely independent; there's currently no way to search or move bookmarks across profiles, only to list which ones exist and switch between them. `active_profile` is stored in `config.json`, which is otherwise shared (things like `editor_cmd` / `browser_cmd` apply regardless of which profile is active).

Nothing changes if you never touch `--profile` — the original flat layout (`base_dir/html`, etc.) is exactly what "no active profile" means, so existing collections are completely unaffected.

`--profile delete` only stops tracking a profile in the list shown by `--profile`. It never touches the folder or its bookmarks, and refuses to delete the currently active profile (switch away first). Switching to a name you'd previously deleted-from-tracking picks its existing data back up rather than starting over.

## Reindexing

`liber -r` does two things, in order:

**1. Clean up entries whose files were deleted outside liber.** Every bookmark's Markdown/Archive/Attachment paths are recorded individually on that bookmark's own index entry when it's created (and kept in sync whenever you edit it) — liber never matches files across bookmarks by filename pattern. So:

- If you delete a bookmark's `.html` file yourself (`rm`, a file manager, etc.) instead of through `liber -d` / `liber -s`, the index still points at it and thinks it exists.
- `liber -r` checks every entry's recorded HTML path. If it's gone, that entry is dropped from the index — but its recorded Markdown/Archive/Attachment files (if they're still there) are **moved, not deleted**, into `<base_dir>/unindexed/markdown/...`, `<base_dir>/unindexed/archive/...`, and `<base_dir>/unindexed/attachments/...`, preserving their original relative path.
- Because each move follows that one bookmark's own recorded path rather than a glob/prefix match, a stray `0002-*.md` can never get relocated alongside, or confused with, some other id's `.html`/archive file — even if two bookmarks share a folder or similar-looking filenames.
- Bookmarks whose HTML is still present are left alone; only truly-missing Markdown/Archive/Attachment references on those are cleared (there's nothing to move since they're already fully gone).

**2. Renumber to close id gaps.** If you had ids 1–4 and deleted 3, `liber -l` would otherwise show 1, 2, 4 forever. `liber -r` renumbers the remainder to 1, 2, 3, in their existing order — it's a gap-closing compaction, not an alphabetical or any other kind of sort. Since ids are embedded in filenames (`0004-...` → `0003-...`), this physically renames each affected bookmark's HTML/markdown/archive/attachment files to match. That rename is done in two passes: every affected file is moved to a temporary staging name first, and only once all of them are staged does anything land on its final numbered name — so a bookmark moving into a lower id slot can never collide with, or get confused with, another bookmark's files, no matter how many ids shift in the same run.

Safe to run any time. Step 1 never touches a bookmark whose HTML file is still there and never deletes anything outright, and step 2 only ever renames files, never their content.

## Archive backends

`liber -a` can archive a page through three backends, selected by
`"archive_backend"` in `config.json`:

| Backend | What it does | Needs |
|---|---|---|
| `single-file` | Renders the page in a headless browser and bundles everything | `single-file-cli` + a browser |
| `monolith` | Fetches the page and inlines assets as data URIs; no browser | `monolith` binary |
| `native` | Built-in static snapshot: fetches the page, inlines images/styles/snippets as data URIs, **scripts are stripped** | nothing |

With `"archive_backend": "auto"` (the default), liber tries them in that
order — skipping any whose binary is missing and falling through on
failure — and the built-in `native` backend always exists, so archiving
never dead-ends. Setting an explicit backend makes it strict: an error,
no silent fallback.

`monolith` vs `native`: monolith is more complete (fonts, caching hints,
edge-case handling) while `native` is dependency-free and fully static —
no JavaScript is kept at all (`<script>` elements are removed, `<noscript>`
fallbacks are unwrapped), so it's ideal for housekeeping content like
guides/docs on headless servers.

Monolith can also render JavaScript pages via a headless chromium pipe:

```json
{
  "archive_backend": "monolith",
  "monolith_use_browser": true,
  "monolith_browser_path": "/usr/bin/chromium"
}
```

This runs the equivalent of:

```sh
chromium --headless --window-size=1920,1080 \
         --run-all-compositor-stages-before-draw --virtual-time-budget=9000 \
         --incognito --dump-dom <url> | monolith - -I -b <url> -o <out>
```

If `monolith_use_browser` is true and `monolith_browser_path` is unset,
`chromium`, `chromium-browser`, and `google-chrome` are tried from `PATH`.

Config keys for this: `archive_backend` (default `auto`),
`monolith_cmd`, `monolith_use_browser`, `monolith_browser_path`.

# Design notes

## Why tags and folders both

The main reason is to keep bookmarks organized even outside the use of liber.

Tags are useful for identification and classification of bookmarks; pairing them with folders allows clean management. Folders can be used for per-project or website-based classification, and tags mark them for relevance. This prevents cluttering the bookmarks with too many tags, and a folder can provide a higher-level domain for classification. For example, a folder for a project can classify bookmarks cleanly while still having tags like `study`, `important`, and so on. While it's true you could tag them on a per-project basis as well, liber is meant to be used as a general bookmark manager that allows easy bookmark navigation even when you're exploring your bookmarks outside of liber.

## Markdown Copy

A markdown copy is not meant to be a whole HTML conversion or webpage contents of a URL. It is simply meant to add personal notes, additional description, or your own bookmark-specific content into it. For example, you can explain what you were looking for or what you researched in your markdown copy of that bookmark. For the purpose of archiving a webpage, `single-file` fills that role, so markdown has no need to copy the contents of the webpage.

## Why not a Database for Indexing

The main reason is to keep it simple: plain Go is fully able to fulfill this project's needs. The indexing is fully solvable in plain Go, so there wasn't a technical need to bring in a database. Liber is meant to be a local simple CLI bookmark manager. If the project becomes too large for plain Go to handle indices, or isn't providing reasonable performance with it, then it could be considered in the future; however, the goal is to keep it simple and explorable even outside the use of liber.

Sticking with flat JSON + files also keeps the project at zero external dependencies, keeps the whole collection readable / diffable / greppable, easy to back up or sync with git, and avoids a CGO build dependency.
