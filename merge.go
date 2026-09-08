package main

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type mergeReport struct {
	merged     []int
	reassigned [][2]int
	deduped    []*Bookmark
	rulesAdded int
	skipped    []string
}

// findConflictCopies lists sync-tool conflict copies of the index.
func findConflictCopies(dir string) []string {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return nil
	}
	var out []string
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		name := e.Name()
		if name == "index.json" {
			continue
		}
		if !strings.HasSuffix(name, ".json") {
			continue
		}
		if strings.Contains(strings.ToLower(name), "conflict") {
			out = append(out, filepath.Join(dir, name))
		}
	}
	sort.Strings(out)
	return out
}

// mergeStores folds other indexes into base, mutating base.
func mergeStores(base *Store, others []*Store) *mergeReport {
	rep := &mergeReport{}
	byID := map[int]*Bookmark{}
	maxID := 0
	for _, b := range base.Bookmarks {
		byID[b.ID] = b
		if b.ID > maxID {
			maxID = b.ID
		}
	}
	byURL := map[string]*Bookmark{}
	for _, b := range base.Bookmarks {
		byURL[normalizeForDedupe(b.URL)] = b
	}
	maxRuleID := 0
	ruleMatch := map[string]*AutoRule{}
	for _, r := range base.AutoRules {
		ruleMatch[r.Match] = r
		if r.ID > maxRuleID {
			maxRuleID = r.ID
		}
	}

	for _, other := range others {
		for _, ib := range other.Bookmarks {
			key := normalizeForDedupe(ib.URL)
			if eb, ok := byID[ib.ID]; ok {
				if normalizeForDedupe(eb.URL) == key {
					mergeBookmarkFields(eb, ib)
					rep.merged = append(rep.merged, eb.ID)
				} else if dup, ok := byURL[key]; ok {
					dup.Tags = dedupe(append(dup.Tags, ib.Tags...))
					if dup.UpdatedAt.Before(ib.UpdatedAt) {
						dup.UpdatedAt = ib.UpdatedAt
					}
					rep.deduped = append(rep.deduped, ib)
				} else {
					maxID++
					old := ib.ID
					ib.ID = maxID
					reprefixBookmarkFiles(ib, old, maxID)
					base.Bookmarks = append(base.Bookmarks, ib)
					byID[ib.ID] = ib
					byURL[key] = ib
					rep.reassigned = append(rep.reassigned, [2]int{old, maxID})
				}
				continue
			}
			if eb, ok := byURL[key]; ok {
				eb.Tags = dedupe(append(eb.Tags, ib.Tags...))
				if eb.UpdatedAt.Before(ib.UpdatedAt) {
					eb.UpdatedAt = ib.UpdatedAt
				}
				rep.deduped = append(rep.deduped, ib)
				continue
			}
			if ib.ID > maxID {
				maxID = ib.ID
			}
			base.Bookmarks = append(base.Bookmarks, ib)
			byID[ib.ID] = ib
			byURL[key] = ib
		}
		for _, ir := range other.AutoRules {
			if _, ok := ruleMatch[ir.Match]; ok {
				continue
			}
			maxRuleID++
			ir.ID = maxRuleID
			base.AutoRules = append(base.AutoRules, ir)
			ruleMatch[ir.Match] = ir
			rep.rulesAdded++
		}
	}
	base.NextID = maxID + 1
	base.NextAutoRuleID = maxRuleID + 1
	sort.Slice(rep.merged, func(i, j int) bool { return rep.merged[i] < rep.merged[j] })
	return rep
}

func mergeBookmarkFields(dst, src *Bookmark) {
	if src.UpdatedAt.After(dst.UpdatedAt) {
		dst.Title = src.Title
		dst.Description = src.Description
		dst.URL = src.URL
		dst.Folder = src.Folder
		dst.UpdatedAt = src.UpdatedAt
	}
	if src.CreatedAt.Before(dst.CreatedAt) {
		dst.CreatedAt = src.CreatedAt
	}
	dst.Tags = dedupe(append(dst.Tags, src.Tags...))
	if dst.MarkdownFile == "" {
		dst.MarkdownFile = src.MarkdownFile
	}
	if dst.ArchiveFile == "" {
		dst.ArchiveFile = src.ArchiveFile
	}
	seen := map[string]bool{}
	for _, at := range dst.Attachments {
		seen[at.File] = true
	}
	for _, at := range src.Attachments {
		if !seen[at.File] {
			dst.Attachments = append(dst.Attachments, at)
		}
	}
	seenRule := map[int]bool{}
	for _, a := range dst.AppliedRules {
		seenRule[a.RuleID] = true
	}
	for _, a := range src.AppliedRules {
		if !seenRule[a.RuleID] {
			dst.AppliedRules = append(dst.AppliedRules, a)
		}
	}
	if src.OpenCount > dst.OpenCount {
		dst.OpenCount = src.OpenCount
		dst.LastOpenedAt = src.LastOpenedAt
	}
	if src.LastCheckedAt.After(dst.LastCheckedAt) {
		dst.LastCheckedAt = src.LastCheckedAt
		dst.LastCheckStatus = src.LastCheckStatus
	}
}

// reprefixBookmarkFiles rewrites recorded paths after an id reassign.
func reprefixBookmarkFiles(b *Bookmark, oldID, newID int) {
	oldPrefix := fmt.Sprintf("%04d-", oldID)
	newPrefix := fmt.Sprintf("%04d-", newID)
	swap := func(rel string) string {
		base := filepath.Base(rel)
		if !strings.HasPrefix(base, oldPrefix) {
			return rel
		}
		return filepath.Join(filepath.Dir(rel), newPrefix+strings.TrimPrefix(base, oldPrefix))
	}
	b.HTMLFile = swap(b.HTMLFile)
	b.MarkdownFile = swap(b.MarkdownFile)
	b.ArchiveFile = swap(b.ArchiveFile)
	for i := range b.Attachments {
		b.Attachments[i].File = swap(b.Attachments[i].File)
	}
}

// renameCollisionFiles moves reassigned files from the old id prefix.
// Recorded paths already carry the new prefix (see reprefixBookmarkFiles).
func renameCollisionFiles(cfg Config, b *Bookmark, oldID, newID int) {
	oldPrefix := fmt.Sprintf("%04d-", oldID)
	newPrefix := fmt.Sprintf("%04d-", newID)
	move := func(dir, rel string) string {
		if rel == "" {
			return ""
		}
		base := filepath.Base(rel)
		if !strings.HasPrefix(base, newPrefix) {
			return rel
		}
		oldRel := filepath.Join(filepath.Dir(rel), oldPrefix+strings.TrimPrefix(base, newPrefix))
		src, dst := filepath.Join(dir, oldRel), filepath.Join(dir, rel)
		if !fileExists(src) {
			if fileExists(dst) {
				return rel
			}
			return ""
		}
		if fileExists(dst) {
			fmt.Fprintf(os.Stderr, "warning: %s already exists, leaving %s as-is\n", dst, src)
			return oldRel
		}
		if err := moveFile(src, dst); err != nil {
			fmt.Fprintf(os.Stderr, "warning: could not move %s: %v\n", src, err)
			return oldRel
		}
		return rel
	}
	b.HTMLFile = move(cfg.htmlDir(), b.HTMLFile)
	b.MarkdownFile = move(cfg.markdownDir(), b.MarkdownFile)
	b.ArchiveFile = move(cfg.archiveDir(), b.ArchiveFile)
	for i := range b.Attachments {
		b.Attachments[i].File = move(cfg.attachmentsDir(), b.Attachments[i].File)
	}
}
