package main

import (
	"fmt"
	"path/filepath"
	"strings"
	"time"
)

// CreateOptions holds the flags parsed from `liber <url> [flags]`.
type CreateOptions struct {
	Interactive bool
	Markdown    bool
	Archive     bool
	Tags        []string
	Folder      string
	Attach      []string
}

func runCreate(rawURL string, opt CreateOptions) error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	url := normalizeURL(rawURL)
	folder := sanitizeFolder(opt.Folder)
	tags := append([]string{}, opt.Tags...)

	if dup := findDuplicate(store, url); dup != nil {
		fmt.Printf("This looks like it's already bookmarked: [%d] %s (folder: %s)\n", dup.ID, dup.Title, displayFolder(dup.Folder))
		if !confirm("Add it anyway?", false) {
			fmt.Println("Cancelled.")
			return nil
		}
	}

	fmt.Printf("Fetching title for %s ...\n", url)
	title := fetchTitle(url)
	if strings.TrimSpace(title) == "" {
		title = url
	}

	// Rules resolve against the fetched title too, so "title:" matches work here.
	folder, tags, appliedRuleIDs := resolveAutoRulesForNew(store, url, title, folder, tags)

	description := ""

	if opt.Interactive {
		title = promptDefault("Title", title)
		description = promptLine("Description")
		tagsLine := promptDefault("Tags (space separated)", strings.Join(tags, " "))
		tags = strings.Fields(tagsLine)
		folder = sanitizeFolder(promptDefault("Folder", folder))
	}

	if strings.TrimSpace(title) == "" {
		title = url
	}

	b, err := addBookmarkToStore(cfg, store, url, title, description, tags, folder, opt.Markdown, opt.Archive)
	if err != nil {
		return err
	}
	b.AppliedRules = appliedRuleIDs

	for _, p := range opt.Attach {
		attachOrWarn(cfg, b, p)
	}
	if opt.Interactive {
		attachmentsMenu(cfg, b)
	}

	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}

	fmt.Printf("\nSaved [%d] %s\n", b.ID, b.Title)
	fmt.Printf("  html:     %s\n", filepath.Join(cfg.htmlDir(), b.HTMLFile))
	if b.MarkdownFile != "" {
		fmt.Printf("  markdown: %s\n", filepath.Join(cfg.markdownDir(), b.MarkdownFile))
	}
	if b.ArchiveFile != "" {
		fmt.Printf("  archive:  %s\n", filepath.Join(cfg.archiveDir(), b.ArchiveFile))
	}
	for _, at := range b.Attachments {
		fmt.Printf("  attach:   %s\n", filepath.Join(cfg.attachmentsDir(), at.File))
	}
	if len(appliedRuleIDs) > 0 {
		fmt.Printf("  auto:     matched automation %s\n", joinInts(ruleIDsOf(appliedRuleIDs)))
	}
	return nil
}

// addBookmarkToStore writes html/markdown/archive; shared by create and import.
func addBookmarkToStore(cfg Config, store *Store, url, title, description string, tags []string, folder string, addMarkdown, addArchive bool) (*Bookmark, error) {
	b := &Bookmark{
		URL:         url,
		Title:       title,
		Description: description,
		Tags:        dedupe(tags),
		Folder:      folder,
		CreatedAt:   time.Now(),
		UpdatedAt:   time.Now(),
	}
	store.Add(b) // assigns b.ID

	base := fmt.Sprintf("%04d-%s", b.ID, slugOrFallback(title, b.ID))

	htmlRel := filepath.Join(folder, base+".html")
	if err := writeHTMLBookmark(filepath.Join(cfg.htmlDir(), htmlRel), b); err != nil {
		return nil, fmt.Errorf("writing html bookmark: %w", err)
	}
	b.HTMLFile = htmlRel

	if addMarkdown {
		mdRel := filepath.Join(folder, base+".md")
		if err := writeMarkdownBookmark(filepath.Join(cfg.markdownDir(), mdRel), b); err != nil {
			fmt.Printf("warning: could not write markdown for %s: %v\n", url, err)
		} else {
			b.MarkdownFile = mdRel
		}
	}

	if addArchive {
		archRel := filepath.Join(folder, base+".html")
		archAbs := filepath.Join(cfg.archiveDir(), archRel)
		fmt.Printf("Archiving %s ...\n", url)
		if err := runArchive(cfg, url, archAbs); err != nil {
			fmt.Printf("warning: archive failed for %s: %v\n", url, err)
		} else {
			b.ArchiveFile = archRel
		}
	}

	// If markdown was written before the archive path was known, note it now.
	if addMarkdown && addArchive && b.ArchiveFile != "" {
		_ = writeMarkdownBookmark(filepath.Join(cfg.markdownDir(), b.MarkdownFile), b)
	}

	return b, nil
}

func slugOrFallback(title string, id int) string {
	s := slugify(title)
	if s == "" {
		return fmt.Sprintf("bookmark-%d", id)
	}
	return s
}
