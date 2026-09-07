package main

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

func runReindex() error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	unindexedRoot := filepath.Join(cfg.effectiveBaseDir(), "unindexed")

	var kept []*Bookmark
	removed := 0
	orphansMoved := 0

	for _, b := range store.Bookmarks {
		htmlAbs := ""
		if b.HTMLFile != "" {
			htmlAbs = filepath.Join(cfg.htmlDir(), b.HTMLFile)
		}

		if htmlAbs != "" && fileExists(htmlAbs) {
			if b.MarkdownFile != "" && !fileExists(filepath.Join(cfg.markdownDir(), b.MarkdownFile)) {
				b.MarkdownFile = ""
			}
			if b.ArchiveFile != "" && !fileExists(filepath.Join(cfg.archiveDir(), b.ArchiveFile)) {
				b.ArchiveFile = ""
			}
			var live []Attachment
			for _, at := range b.Attachments {
				if fileExists(filepath.Join(cfg.attachmentsDir(), at.File)) {
					live = append(live, at)
				}
			}
			b.Attachments = live
			kept = append(kept, b)
			continue
		}

		removed++
		if b.MarkdownFile != "" {
			src := filepath.Join(cfg.markdownDir(), b.MarkdownFile)
			if fileExists(src) {
				dst := filepath.Join(unindexedRoot, "markdown", b.MarkdownFile)
				if err := moveFile(src, dst); err != nil {
					fmt.Fprintf(os.Stderr, "warning: could not move %s: %v\n", src, err)
				} else {
					orphansMoved++
				}
			}
		}
		if b.ArchiveFile != "" {
			src := filepath.Join(cfg.archiveDir(), b.ArchiveFile)
			if fileExists(src) {
				dst := filepath.Join(unindexedRoot, "archive", b.ArchiveFile)
				if err := moveFile(src, dst); err != nil {
					fmt.Fprintf(os.Stderr, "warning: could not move %s: %v\n", src, err)
				} else {
					orphansMoved++
				}
			}
		}
		for _, at := range b.Attachments {
			src := filepath.Join(cfg.attachmentsDir(), at.File)
			if fileExists(src) {
				dst := filepath.Join(unindexedRoot, "attachments", at.File)
				if err := moveFile(src, dst); err != nil {
					fmt.Fprintf(os.Stderr, "warning: could not move %s: %v\n", src, err)
				} else {
					orphansMoved++
				}
			}
		}
	}

	renamed, err := compactIDs(cfg, kept)
	if err != nil {
		return fmt.Errorf("renumbering ids: %w", err)
	}

	store.Bookmarks = kept
	store.NextID = len(kept) + 1
	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}

	fmt.Printf("Reindexed: %d bookmark(s) remain.\n", len(kept))
	if removed > 0 {
		fmt.Printf("Dropped %d entr%s whose bookmark file no longer exists.\n", removed, entrySuffix(removed))
	}
	if orphansMoved > 0 {
		fmt.Printf("Moved %d orphaned markdown/archive/attachment file(s) to %s\n", orphansMoved, unindexedRoot)
	}
	if len(renamed) > 0 {
		fmt.Println("Renumbered to close gaps:")
		for _, r := range renamed {
			fmt.Println("  " + r)
		}
	}
	if removed == 0 && orphansMoved == 0 && len(renamed) == 0 {
		fmt.Println("Nothing to clean up -- the index already matches what's on disk.")
	}
	return nil
}

func entrySuffix(n int) string {
	if n == 1 {
		return "y"
	}
	return "ies"
}

type bookmarkFileField struct {
	label string
	dir   func(Config) string
	get   func(*Bookmark) string
	set   func(*Bookmark, string)
}

var bookmarkFileFields = []bookmarkFileField{
	{"html", Config.htmlDir, func(b *Bookmark) string { return b.HTMLFile }, func(b *Bookmark, s string) { b.HTMLFile = s }},
	{"markdown", Config.markdownDir, func(b *Bookmark) string { return b.MarkdownFile }, func(b *Bookmark, s string) { b.MarkdownFile = s }},
	{"archive", Config.archiveDir, func(b *Bookmark) string { return b.ArchiveFile }, func(b *Bookmark, s string) { b.ArchiveFile = s }},
}

func compactIDs(cfg Config, kept []*Bookmark) ([]string, error) {
	sort.Slice(kept, func(i, j int) bool { return kept[i].ID < kept[j].ID })

	type pendingMove struct {
		field     bookmarkFileField
		b         *Bookmark
		stagedAbs string
		finalRel  string
		attIdx    int // >= 0 means this move is an attachment (field is unused)
	}
	var pending []pendingMove
	var renamed []string

	stagingRoot := filepath.Join(cfg.effectiveBaseDir(), ".liber", "restage")
	defer os.RemoveAll(stagingRoot)

	nextID := 1
	for _, b := range kept {
		oldID := b.ID
		newID := nextID
		nextID++
		if oldID == newID {
			continue
		}
		oldPrefix := fmt.Sprintf("%04d-", oldID)
		newPrefix := fmt.Sprintf("%04d-", newID)

		for _, f := range bookmarkFileFields {
			rel := f.get(b)
			if rel == "" {
				continue
			}
			base := filepath.Base(rel)
			if !strings.HasPrefix(base, oldPrefix) {
				fmt.Fprintf(os.Stderr, "warning: %s file for bookmark %d doesn't match the expected 0000- naming, leaving it as-is\n", f.label, oldID)
				continue
			}
			newBase := newPrefix + strings.TrimPrefix(base, oldPrefix)
			finalRel := filepath.Join(filepath.Dir(rel), newBase)

			srcAbs := filepath.Join(f.dir(cfg), rel)
			stagedAbs := filepath.Join(stagingRoot, f.label, rel)
			if err := moveFile(srcAbs, stagedAbs); err != nil {
				return renamed, fmt.Errorf("staging %s: %w", srcAbs, err)
			}
			pending = append(pending, pendingMove{field: f, b: b, stagedAbs: stagedAbs, finalRel: finalRel, attIdx: -1})
		}

		for i, at := range b.Attachments {
			base := filepath.Base(at.File)
			if !strings.HasPrefix(base, oldPrefix) {
				fmt.Fprintf(os.Stderr, "warning: attachment file for bookmark %d doesn't match the expected 0000- naming, leaving it as-is\n", oldID)
				continue
			}
			newBase := newPrefix + strings.TrimPrefix(base, oldPrefix)
			finalRel := filepath.Join(filepath.Dir(at.File), newBase)

			srcAbs := filepath.Join(cfg.attachmentsDir(), at.File)
			stagedAbs := filepath.Join(stagingRoot, "attachments", at.File)
			if err := moveFile(srcAbs, stagedAbs); err != nil {
				return renamed, fmt.Errorf("staging %s: %w", srcAbs, err)
			}
			pending = append(pending, pendingMove{b: b, stagedAbs: stagedAbs, finalRel: finalRel, attIdx: i})
		}

		renamed = append(renamed, fmt.Sprintf("[%d] -> [%d]", oldID, newID))
		b.ID = newID
	}

	for _, p := range pending {
		var finalAbs string
		if p.attIdx >= 0 {
			finalAbs = filepath.Join(cfg.attachmentsDir(), p.finalRel)
		} else {
			finalAbs = filepath.Join(p.field.dir(cfg), p.finalRel)
		}
		if err := moveFile(p.stagedAbs, finalAbs); err != nil {
			return renamed, fmt.Errorf("finalizing %s: %w", finalAbs, err)
		}
		if p.attIdx >= 0 {
			p.b.Attachments[p.attIdx].File = p.finalRel
		} else {
			p.field.set(p.b, p.finalRel)
		}
	}

	return renamed, nil
}
