package main

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

func runReindex(args []string) error {
	merge := false
	for _, a := range args {
		if a == "--merge" {
			merge = true
			continue
		}
		return fmt.Errorf("unknown flag %q (usage: liber -r [--merge])", a)
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	unindexedRoot := filepath.Join(cfg.effectiveBaseDir(), "unindexed")

	copies := findConflictCopies(filepath.Join(cfg.effectiveBaseDir(), ".liber"))
	if len(copies) > 0 && !merge {
		fmt.Println("Sync conflict copies found (nothing changed):")
		for _, c := range copies {
			fmt.Println("  " + c)
		}
		fmt.Println("Re-run with --merge to fold them into the index.")
		return nil
	}
	if merge {
		if err := runMerge(cfg, store, copies); err != nil {
			return err
		}
	}

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
		orphansMoved += quarantineOrphans(cfg, unindexedRoot, b)
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

// runMerge folds sync conflict copies into the store.
func runMerge(cfg Config, store *Store, copies []string) error {
	if len(copies) == 0 {
		fmt.Println("No conflict copies to merge.")
		return nil
	}
	var others []*Store
	var skipped []string
	for _, c := range copies {
		other, err := LoadStore(c)
		if err != nil {
			skipped = append(skipped, c)
			fmt.Fprintf(os.Stderr, "warning: skipping unparseable %s: %v\n", c, err)
			continue
		}
		others = append(others, other)
	}
	rep := mergeStores(store, others)

	unindexedRoot := filepath.Join(cfg.effectiveBaseDir(), "unindexed")
	quarantined := 0
	for _, pair := range rep.reassigned {
		if b := store.Find(pair[1]); b != nil {
			renameCollisionFiles(cfg, b, pair[0], pair[1])
		}
	}
	for _, b := range rep.deduped {
		if b.HTMLFile != "" {
			src := filepath.Join(cfg.htmlDir(), b.HTMLFile)
			dst := filepath.Join(unindexedRoot, "html", b.HTMLFile)
			if fileExists(src) {
				if err := moveFile(src, dst); err != nil {
					fmt.Fprintf(os.Stderr, "warning: could not move %s: %v\n", src, err)
				} else {
					quarantined++
				}
			}
		}
		quarantined += quarantineOrphans(cfg, unindexedRoot, b)
	}

	resolvedDir := filepath.Join(cfg.effectiveBaseDir(), ".liber", "resolved")
	for _, c := range copies {
		skip := false
		for _, s := range skipped {
			if s == c {
				skip = true
			}
		}
		if skip {
			continue
		}
		dst := filepath.Join(resolvedDir, filepath.Base(c))
		if err := moveFile(c, dst); err != nil {
			return fmt.Errorf("moving resolved %s: %w", c, err)
		}
	}

	fmt.Printf("Merged %d conflict cop%s:\n", len(others), entrySuffix(len(others)))
	for _, id := range rep.merged {
		fmt.Printf("  merged [%d]\n", id)
	}
	for _, pair := range rep.reassigned {
		fmt.Printf("  collision [%d] reassigned to [%d]\n", pair[0], pair[1])
	}
	for _, b := range rep.deduped {
		fmt.Printf("  duplicate [%d] %s folded away\n", b.ID, b.Title)
	}
	if rep.rulesAdded > 0 {
		fmt.Printf("  %d automation rule(s) added\n", rep.rulesAdded)
	}
	for _, s := range skipped {
		fmt.Printf("  skipped %s\n", s)
	}
	if quarantined > 0 {
		fmt.Printf("Moved %d duplicate file(s) to %s\n", quarantined, unindexedRoot)
	}
	return nil
}

func quarantineOrphans(cfg Config, unindexedRoot string, b *Bookmark) int {
	moved := 0
	move := func(src, dst string) {
		if !fileExists(src) {
			return
		}
		if err := moveFile(src, dst); err != nil {
			fmt.Fprintf(os.Stderr, "warning: could not move %s: %v\n", src, err)
		} else {
			moved++
		}
	}
	if b.MarkdownFile != "" {
		move(filepath.Join(cfg.markdownDir(), b.MarkdownFile),
			filepath.Join(unindexedRoot, "markdown", b.MarkdownFile))
	}
	if b.ArchiveFile != "" {
		move(filepath.Join(cfg.archiveDir(), b.ArchiveFile),
			filepath.Join(unindexedRoot, "archive", b.ArchiveFile))
	}
	for _, at := range b.Attachments {
		move(filepath.Join(cfg.attachmentsDir(), at.File),
			filepath.Join(unindexedRoot, "attachments", at.File))
	}
	return moved
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
