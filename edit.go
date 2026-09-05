package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// syncBookmarkFiles moves files if the folder changed, then rewrites content to match b.
func syncBookmarkFiles(cfg Config, b *Bookmark, folderChanged bool) {
	if b.HTMLFile != "" {
		rel := b.HTMLFile
		if folderChanged {
			newRel := filepath.Join(b.Folder, filepath.Base(b.HTMLFile))
			if err := moveFile(filepath.Join(cfg.htmlDir(), rel), filepath.Join(cfg.htmlDir(), newRel)); err != nil {
				fmt.Fprintf(os.Stderr, "warning: could not move html file: %v\n", err)
			}
			rel = newRel
			b.HTMLFile = rel
		}
		if err := writeHTMLBookmark(filepath.Join(cfg.htmlDir(), rel), b); err != nil {
			fmt.Fprintf(os.Stderr, "warning: could not rewrite html file: %v\n", err)
		}
	}

	if b.MarkdownFile != "" {
		rel := b.MarkdownFile
		if folderChanged {
			newRel := filepath.Join(b.Folder, filepath.Base(b.MarkdownFile))
			if err := moveFile(filepath.Join(cfg.markdownDir(), rel), filepath.Join(cfg.markdownDir(), newRel)); err != nil {
				fmt.Fprintf(os.Stderr, "warning: could not move markdown file: %v\n", err)
			}
			rel = newRel
			b.MarkdownFile = rel
		}
		if err := writeMarkdownBookmark(filepath.Join(cfg.markdownDir(), rel), b); err != nil {
			fmt.Fprintf(os.Stderr, "warning: could not rewrite markdown file: %v\n", err)
		}
	}

	if b.ArchiveFile != "" && folderChanged {
		newRel := filepath.Join(b.Folder, filepath.Base(b.ArchiveFile))
		if err := moveFile(filepath.Join(cfg.archiveDir(), b.ArchiveFile), filepath.Join(cfg.archiveDir(), newRel)); err != nil {
			fmt.Fprintf(os.Stderr, "warning: could not move archive file: %v\n", err)
		}
		b.ArchiveFile = newRel
	}
}

func deleteBookmarkFiles(cfg Config, b *Bookmark) {
	if b.HTMLFile != "" {
		os.Remove(filepath.Join(cfg.htmlDir(), b.HTMLFile))
	}
	if b.MarkdownFile != "" {
		os.Remove(filepath.Join(cfg.markdownDir(), b.MarkdownFile))
	}
	if b.ArchiveFile != "" {
		os.Remove(filepath.Join(cfg.archiveDir(), b.ArchiveFile))
	}
	for _, at := range b.Attachments {
		os.Remove(filepath.Join(cfg.attachmentsDir(), at.File))
	}
}

// sharedBase returns the id-slug basename shared across a bookmark's files; see dev-docs.md#data-model.
func sharedBase(b *Bookmark) string {
	base := filepath.Base(b.HTMLFile)
	return strings.TrimSuffix(base, filepath.Ext(base))
}

// addMarkdownCopy adds a markdown copy if missing; no-op if one already exists.
func addMarkdownCopy(cfg Config, b *Bookmark) {
	if b.MarkdownFile != "" {
		fmt.Println("Already has a markdown copy -- skipping.")
		return
	}
	rel := filepath.Join(b.Folder, sharedBase(b)+".md")
	if err := writeMarkdownBookmark(filepath.Join(cfg.markdownDir(), rel), b); err != nil {
		fmt.Printf("warning: could not write markdown copy: %v\n", err)
		return
	}
	b.MarkdownFile = rel
	fmt.Println("Added markdown copy.")
}

// addArchiveCopy adds an archive if missing; no-op if one already exists.
func addArchiveCopy(cfg Config, b *Bookmark) {
	if b.ArchiveFile != "" {
		fmt.Println("Already has an archive -- skipping.")
		return
	}
	rel := filepath.Join(b.Folder, sharedBase(b)+".html")
	if err := runArchive(cfg, b.URL, filepath.Join(cfg.archiveDir(), rel)); err != nil {
		fmt.Printf("warning: archive failed: %v\n", err)
		return
	}
	b.ArchiveFile = rel
	fmt.Println("Added archive.")
}

// editBookmarkInteractive prompts for every field and offers to add missing markdown/archive.
func editBookmarkInteractive(cfg Config, b *Bookmark) {
	newURL := normalizeURL(promptDefault("URL", b.URL))
	newTitle := promptDefault("Title", b.Title)
	newDesc := promptDefault("Description", b.Description)
	newTagsLine := promptDefault("Tags (space separated)", strings.Join(b.Tags, " "))
	newFolder := sanitizeFolder(promptDefault("Folder", b.Folder))

	b.URL = newURL
	b.Title = newTitle
	b.Description = newDesc
	if strings.TrimSpace(newTagsLine) == "" {
		b.Tags = nil
	} else {
		b.Tags = dedupe(strings.Fields(newTagsLine))
	}
	folderChanged := newFolder != b.Folder
	b.Folder = newFolder
	b.UpdatedAt = time.Now()

	syncBookmarkFiles(cfg, b, folderChanged)

	if b.MarkdownFile == "" && confirm("Add a markdown copy?", false) {
		addMarkdownCopy(cfg, b)
	}
	if b.ArchiveFile == "" && confirm("Add an archive copy?", false) {
		addArchiveCopy(cfg, b)
	}
	attachmentsMenu(cfg, b)

	fmt.Println("Updated.")
}

// editFlags are the flags for `liber -e <id> [-t ...] [-f folder] [-md] [-a] [-at file] [-dt match]`.
type editFlags struct {
	tagsSet     bool
	tags        []string
	folderSet   bool
	folder      string
	urlSet      bool
	url         string
	addMarkdown bool
	addArchive  bool
	attach      []string
	detach      []string
}

func parseEditFlags(args []string) (editFlags, error) {
	var ef editFlags
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "-t", "--tags":
			ef.tagsSet = true
			i++
			for i < len(args) && !strings.HasPrefix(args[i], "-") {
				ef.tags = append(ef.tags, args[i])
				i++
			}
			i--
		case "-f", "--folder":
			if i+1 >= len(args) {
				return ef, fmt.Errorf("-f requires a folder argument")
			}
			ef.folderSet = true
			ef.folder = args[i+1]
			i++
		case "-u", "--url":
			if i+1 >= len(args) {
				return ef, fmt.Errorf("-u requires a URL")
			}
			ef.urlSet = true
			ef.url = args[i+1]
			i++
		case "-md", "--markdown":
			ef.addMarkdown = true
		case "-a", "--archive":
			ef.addArchive = true
		case "-at", "--attach":
			if i+1 >= len(args) {
				return ef, fmt.Errorf("-at requires a file path")
			}
			ef.attach = append(ef.attach, args[i+1])
			i++
		case "-dt", "--detach":
			if i+1 >= len(args) {
				return ef, fmt.Errorf("-dt requires an attachment number or name")
			}
			ef.detach = append(ef.detach, args[i+1])
			i++
		default:
			return ef, fmt.Errorf("unknown flag for -e: %s", args[i])
		}
	}
	return ef, nil
}

func runEdit(ids []int, rest []string) error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	var targets []*Bookmark
	var missing []int
	for _, id := range ids {
		b := store.Find(id)
		if b == nil {
			missing = append(missing, id)
			continue
		}
		targets = append(targets, b)
	}
	if len(targets) == 0 {
		return fmt.Errorf("no matching bookmarks found (see `liber -l`)")
	}

	if len(rest) == 0 {
		for i, b := range targets {
			if len(targets) > 1 {
				fmt.Printf("\n--- Editing %d of %d: [%d] %s ---\n", i+1, len(targets), b.ID, b.Title)
			}
			editBookmarkInteractive(cfg, b)
		}
	} else {
		ef, err := parseEditFlags(rest)
		if err != nil {
			return err
		}
		for _, b := range targets {
			folderChanged := false
			if ef.tagsSet {
				b.Tags = dedupe(ef.tags)
			}
			if ef.urlSet {
				b.URL = normalizeURL(ef.url)
			}
			if ef.folderSet {
				newFolder := sanitizeFolder(ef.folder)
				folderChanged = newFolder != b.Folder
				b.Folder = newFolder
			}
			b.UpdatedAt = time.Now()
			syncBookmarkFiles(cfg, b, folderChanged)

			if ef.addMarkdown {
				addMarkdownCopy(cfg, b)
			}
			if ef.addArchive {
				addArchiveCopy(cfg, b)
			}
			for _, match := range ef.detach {
				i, err := findAttachment(b, match)
				if err != nil {
					fmt.Printf("warning: %v\n", err)
					continue
				}
				name := b.Attachments[i].Name
				detachAttachment(cfg, b, i)
				fmt.Printf("Detached %s\n", name)
			}
			for _, p := range ef.attach {
				attachOrWarn(cfg, b, p)
			}

			fmt.Printf("Updated [%d] %s\n", b.ID, b.Title)
		}
	}

	if len(missing) > 0 {
		fmt.Printf("No bookmark with id(s): %s\n", joinInts(missing))
	}

	return store.Save()
}
