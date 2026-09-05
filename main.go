// Command liber is a small, dependency-free CLI bookmark manager; run `liber -h` for usage.
package main

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

// Version is overridden via -ldflags "-X main.Version=x.y.z" (see Makefile/flake.nix).
var Version = "dev"

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, "liber:", err)
		os.Exit(1)
	}
}

func run(args []string) error {
	if len(args) == 0 {
		printUsage()
		return nil
	}

	if opt, legacy, ok := parseSearchFlag(args[0]); ok {
		deep := false
		if len(args) > 1 {
			if args[1] != "--deep" {
				return fmt.Errorf("unknown flag %q (only --deep is supported after a search flag)", args[1])
			}
			deep = true
		}
		if legacy {
			return runSearchLegacy(opt, deep)
		}
		return runSearch(opt, deep)
	}

	switch args[0] {
	case "-h", "--help", "help":
		printUsage()
		return nil
	case "-v", "--version":
		fmt.Println("liber", Version)
		return nil
	case "-l", "--list":
		return runList()
	case "-o", "--open":
		if len(args) < 2 {
			return fmt.Errorf("--open requires a bookmark id or range, e.g. liber -o 3 or liber -o 1-3")
		}
		spec, rest := consumeIDSpec(args[1:])
		if len(rest) > 0 {
			return fmt.Errorf("-o takes only ids; unknown extra arguments: %s", strings.Join(rest, " "))
		}
		ids, err := parseIDSpec(spec)
		if err != nil {
			return fmt.Errorf("%w (use `liber -l` to see valid ids)", err)
		}
		return runOpen(ids)
	case "--export-site":
		return runExportSite(args[1:])
	case "completion":
		return runCompletion(args[1:])
	case "-e", "--edit":
		if len(args) < 2 {
			return fmt.Errorf("-e requires a bookmark id or range, e.g. liber -e 3 or liber -e 1-3 -md")
		}
		spec, rest := consumeIDSpec(args[1:])
		ids, err := parseIDSpec(spec)
		if err != nil {
			return fmt.Errorf("%w (use `liber -l` to see valid ids)", err)
		}
		return runEdit(ids, rest)
	case "-d", "--delete":
		if len(args) < 2 {
			return fmt.Errorf("-d requires a bookmark id or range, e.g. liber -d 3 or liber -d 1-4,7-9")
		}
		spec, rest := consumeIDSpec(args[1:])
		ids, err := parseIDSpec(spec)
		if err != nil {
			return fmt.Errorf("%w (use `liber -l` to see valid ids)", err)
		}
		return runDelete(ids, rest)
	case "-r", "--reindex":
		return runReindex()
	case "--profile":
		return runProfile(args[1:])
	case "--serve":
		return runServe(args[1:])
	case "--auto":
		if len(args) == 1 {
			return runAutoList()
		}
		switch args[1] {
		case "add":
			return runAutoAdd(args[2:])
		case "list":
			return runAutoList()
		case "edit":
			return runAutoEdit(args[2:])
		case "delete":
			return runAutoDelete(args[2:])
		case "apply":
			return runAutoApply(args[2:])
		default:
			return fmt.Errorf("unknown --auto subcommand %q (expected add, list, edit, delete, or apply)", args[1])
		}
	case "--history":
		return runHistory()
	case "--sync":
		push, err := parseSyncFlags(args[1:])
		if err != nil {
			return err
		}
		return runSync(push)
	case "--import":
		if len(args) < 2 {
			return fmt.Errorf("--import requires a path, e.g. liber --import bookmarks.html")
		}
		impOpt, err := parseImportFlags(args[2:])
		if err != nil {
			return err
		}
		return runImport(args[1], impOpt)
	case "--tags":
		if len(args) == 1 {
			return runTagsList()
		}
		switch args[1] {
		case "rename":
			if len(args) != 4 {
				return fmt.Errorf("usage: liber --tags rename <old> <new>")
			}
			return runTagsRename(args[2], args[3])
		case "delete":
			if len(args) != 3 {
				return fmt.Errorf("usage: liber --tags delete <tag>")
			}
			return runTagsDelete(args[2])
		default:
			return fmt.Errorf("unknown --tags subcommand %q (expected rename or delete)", args[1])
		}
	case "--folders":
		if len(args) == 1 {
			return runFoldersList()
		}
		switch args[1] {
		case "rename":
			if len(args) != 4 {
				return fmt.Errorf("usage: liber --folders rename <old> <new>")
			}
			return runFoldersRename(args[2], args[3])
		case "delete":
			if len(args) != 3 {
				return fmt.Errorf("usage: liber --folders delete <folder>")
			}
			return runFoldersDelete(args[2])
		default:
			return fmt.Errorf("unknown --folders subcommand %q (expected rename or delete)", args[1])
		}
	case "__preview": // internal fzf --preview callback; not in -h on purpose
		if len(args) < 2 {
			return nil
		}
		id, err := strconv.Atoi(args[1])
		if err != nil {
			return nil
		}
		return runPreview(id)
	case "config":
		return runConfigCmd(args[1:])
	}

	if strings.HasPrefix(args[0], "-") {
		return fmt.Errorf("unknown flag %q (expected a URL first, or -s/-sl/-l/-e/-d/-r/-v/-h)", args[0])
	}

	opt, err := parseCreateFlags(args[1:])
	if err != nil {
		return err
	}
	return runCreate(args[0], opt)
}

// parseSearchFlag parses -s/-sl and field-restriction combos; see dev-docs.md#search-scoping.
func parseSearchFlag(flag string) (fields SearchFields, legacy bool, ok bool) {
	switch flag {
	case "--search":
		return SearchFields{}, false, true
	case "--search-legacy":
		return SearchFields{}, true, true
	}
	if !strings.HasPrefix(flag, "-s") {
		return SearchFields{}, false, false
	}
	for _, ch := range flag[2:] {
		switch ch {
		case 'l':
			legacy = true
		case 'n':
			fields.Title = true
		case 'u':
			fields.URL = true
		case 't':
			fields.Tags = true
		case 'd':
			fields.Description = true
		case 'f':
			fields.Folder = true
		default:
			return SearchFields{}, false, false // not a search flag at all
		}
	}
	return fields, legacy, true
}

func parseCreateFlags(args []string) (CreateOptions, error) {
	var opt CreateOptions
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "-i", "--interactive":
			opt.Interactive = true
		case "-md", "--markdown":
			opt.Markdown = true
		case "-a", "--archive":
			opt.Archive = true
		case "-t", "--tags":
			i++
			for i < len(args) && !strings.HasPrefix(args[i], "-") {
				opt.Tags = append(opt.Tags, args[i])
				i++
			}
			i--
		case "-f", "--folder":
			if i+1 >= len(args) {
				return opt, fmt.Errorf("-f requires a folder name")
			}
			opt.Folder = args[i+1]
			i++
		case "-at", "--attach":
			if i+1 >= len(args) {
				return opt, fmt.Errorf("-at requires a file path")
			}
			opt.Attach = append(opt.Attach, args[i+1])
			i++
		default:
			return opt, fmt.Errorf("unknown flag %q", args[i])
		}
	}
	return opt, nil
}

func runConfigCmd(args []string) error {
	cfg, path, err := LoadConfig()
	if err != nil {
		return err
	}
	fmt.Println("Config file:", path)
	fmt.Println()
	activeProfile := "default"
	if cfg.ActiveProfile != "" {
		activeProfile = cfg.ActiveProfile
	}
	fmt.Println("active_profile ", activeProfile)
	fmt.Println("base_dir       ", cfg.BaseDir)
	fmt.Println("html_dir       ", cfg.htmlDir())
	fmt.Println("markdown_dir   ", cfg.markdownDir())
	fmt.Println("archive_dir    ", cfg.archiveDir())
	fmt.Println("attachment_dir ", cfg.attachmentsDir())
	fmt.Println("singlefile_cmd ", cfg.SingleFileCmd)
	if cfg.SingleFileBrowserPath != "" {
		fmt.Println("singlefile_browser_path ", cfg.SingleFileBrowserPath)
	}
	backend := cfg.ArchiveBackend
	if backend == "" {
		backend = "auto"
	}
	fmt.Println("archive_backend  ", backend)
	if cfg.MonolithCmd != "" {
		fmt.Println("monolith_cmd     ", cfg.MonolithCmd)
	}
	if cfg.MonolithBrowserPath != "" {
		fmt.Println("monolith_browser_path ", cfg.MonolithBrowserPath)
	}
	if cfg.MonolithUseBrowser {
		fmt.Println("monolith_use_browser true")
	}
	if cfg.BrowserCmd != "" {
		fmt.Println("browser_cmd   ", cfg.BrowserCmd)
	}
	if cfg.EditorCmd != "" {
		fmt.Println("editor_cmd    ", cfg.EditorCmd)
	}
	fmt.Println("\nEdit the JSON file above to change these.")
	return nil
}

func printUsage() {
	fmt.Print(`liber - a small CLI bookmark manager

Usage:
  liber <url>                    save a bookmark
  liber <url> -i                 save interactively (prompts for description, tags, folder)
  liber <url> -md                also write a markdown copy
  liber <url> -a                 also write a full-page archive; backend is chosen by
                                   config archive_backend: auto (default) tries single-file,
                                   then monolith, then the built-in static snapshot
  liber <url> -md -a             both markdown and archive
  liber <url> -t tag-a tag-b     attach tags at creation time
  liber <url> -f subfold         save into a subfolder of the base directory
  liber <url> -at report.pdf     attach a file (repeatable; copied into the collection)
  liber -s                       search/browse bookmarks, open or edit them
  liber -sn / -su / -st / -sd / -sf
                                  same, but restricted to one field: title / url / tags /
                                  description / folder (combine freely, e.g. -sdf = folder+description)
  liber -sl                      force the plain prompt (skip fzf even if installed)
  liber -sld                     legacy prompt restricted to descriptions (mix -l with any of n/u/t/d/f)
  liber -s --deep                also full-text search inside archived pages (asks for a query
                                  first, then browses matches; combine with -sn/-sd/etc as usual)
  liber -sl --deep               same, forced to the plain prompt
  liber -l                       list all bookmarks with their ids
  liber -e <id>                  edit a bookmark interactively (also offers to add a
                                  markdown copy or archive if either is missing)
  liber -e <id> -t tag-a tag-b   set a bookmark's tags directly
  liber -e <id> -f subfold       move a bookmark to a different folder
  liber -e <id> -u <url>         change a bookmark's URL (the saved html/material
                                   is rewritten; filenames stay as they are)
  liber -e <id> -md              add a markdown copy if it doesn't have one yet
  liber -e <id> -a               add an archive if it doesn't have one yet
  liber -e <id> -at file.pdf     attach a file (repeatable)
  liber -e <id> -dt notes.pdf    detach an attachment by number or name (its saved
                                   copy is deleted; interactive edit lists the numbers)
  liber -e <ids> ...             <id> can also be a range/list: 1-3, 2,5,3, or 1-4,7-9 --
                                  applies the same flags (or interactive edit) to each
  liber -d <id>                  delete a bookmark (asks for confirmation)
  liber -d <id> -y               delete without confirmation
  liber -d <ids>                 <id> can also be a range/list, same as -e (one combined
                                   confirmation listing everything that will be deleted)
  liber -o <id>                  open a bookmark in the browser without the search menu
                                   (accepts ranges like -e/-d; counts as an open for
                                   --history, same as the search menu's (o) action)
  liber -r                       reindex: drop entries whose files were deleted
                                  outside liber (quarantining any surviving
                                  markdown/archive copy into <base_dir>/unindexed/),
                                  and renumber remaining ids to close gaps
  liber --import <path>          import a browser bookmark export (Netscape HTML format)
  liber --import <path> -md -a   same, also generating markdown/archives for each (slow)
  liber --tags                   list all tags with counts
  liber --tags rename <a> <b>    rename a tag everywhere (renaming onto an existing
                                  tag merges into it -- no separate merge command)
  liber --tags delete <tag>      remove a tag from every bookmark that has it
  liber --folders                list all folders with counts
  liber --folders rename <a> <b> rename a folder (and its subfolders) everywhere;
                                  physically moves each bookmark's files
  liber --folders delete <f>     move a folder's bookmarks back to the root
  liber --history                list bookmarks by most recently opened (via -s's (o) action)
  liber --auto add --match <str> --folder <f> --tag <t1 t2>
                                   auto-classify new bookmarks whose url contains <str> (folder
                                   and/or tags; also applied once, immediately, to matching
                                   existing bookmarks -- a later manual move/edit always sticks)
                                   <str> may be prefixed: "host:example.com" matches only the
                                   host, "title:docs" only the title; bare is url as before
  liber --auto                   list automations and how many bookmarks each has classified
  liber --auto edit <id> [--match x] [--folder y] [--tag t1 t2] [--reapply]
                                  change a rule; --reapply re-syncs bookmarks it already
                                  classified (skipping any since manually moved/retagged)
  liber --auto delete <id>       remove a rule (bookmarks it already classified are untouched)
  liber --auto apply [<id>]      re-run one rule, or all of them, against existing bookmarks
  liber --sync                   commit the collection, if <base_dir> is inside a jj or git repo
  liber --sync -p                same, then push
  liber --profile                list profiles (base_dir subfolders that isolate a whole
                                  collection: bookmarks, tags, folders, automations, everything),
                                  marking the active one
  liber --profile <name>         switch to <name>, creating it first if it's new
  liber --profile default        switch back to using <base_dir> directly (no profile)
  liber --profile delete <name>  stop tracking a profile (its folder and data are untouched)
  liber config                   show the active config file and its path
  liber -v                       print the version
  liber --serve                  local web UI at http://127.0.0.1:8080 -- search (with the
                                  same scoping/deep options as -s), plus add, edit, and delete
  liber --serve --addr <host:port>
                                   use a different address (non-loopback prints a warning:
                                   it exposes read/add/edit/delete access, no login)
  liber --export-site [dir]      write a static, browsable index.html of the whole
                                   collection (default <base_dir>/site); links point at
                                   your existing html/markdown/archive/attachment files
  liber completion bash|zsh|fish print the completion script for your shell

Flags may be combined, e.g.:
  liber https://example.com -i -t news reading -f articles -md -a

liber -s uses fzf for picking if it's installed on PATH (title/url/tags/folder
on the left, a markdown/archive/attachment presence badge, and a full detail
preview -- including description -- on the right), otherwise falls back to a
plain numbered prompt. Use -sl to force the plain prompt regardless of whether
fzf is installed. Either can be narrowed to specific fields with the n/u/t/d/f
letters shown above.

Attachments are local files copied into <base_dir>/attachments (any type: PDFs,
extra snapshots, downloads); "-i" prompts for them, the search menu's
attachmen(t)s action opens/manages them, and badge "att"/"attN" shows the count.

Adding a bookmark whose URL normalizes to one you already have (ignoring
trailing slashes, tracking params, and default ports) asks before adding a
duplicate; liber --import skips likely duplicates automatically instead.

Config lives at $XDG_CONFIG_HOME/liber/config.json (created on first run).
`)
}
