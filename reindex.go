package main

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"
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

	adopted, dupMoved := adoptOrphanFiles(cfg, store)
	_ = dupMoved

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
	if adopted > 0 {
		fmt.Printf("Indexed %d bookmark file(s) found on disk but missing from the index.\n", adopted)
	}
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
	if removed == 0 && orphansMoved == 0 && len(renamed) == 0 && adopted == 0 {
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
	var otherPaths []string
	var skipped []string
	for _, c := range copies {
		other, err := LoadStore(c)
		if err != nil {
			skipped = append(skipped, c)
			fmt.Fprintf(os.Stderr, "warning: skipping unparseable %s: %v\n", c, err)
			continue
		}
		others = append(others, other)
		otherPaths = append(otherPaths, c)
	}
	if len(others) > 0 {
		mainPath := cfg.indexPath()
		best := -1
		bestCount := len(store.Bookmarks)
		bestTime := mtimeOf(mainPath)
		for i, o := range others {
			mt := mtimeOf(otherPaths[i])
			if len(o.Bookmarks) > bestCount || (len(o.Bookmarks) == bestCount && mt.Before(bestTime)) {
				best = i
				bestCount = len(o.Bookmarks)
				bestTime = mt
			}
		}
		if best >= 0 {
			winner := others[best]
			oldMain := &Store{NextID: store.NextID, Bookmarks: store.Bookmarks, NextAutoRuleID: store.NextAutoRuleID, AutoRules: store.AutoRules}
			store.NextID, store.Bookmarks = winner.NextID, winner.Bookmarks
			store.NextAutoRuleID, store.AutoRules = winner.NextAutoRuleID, winner.AutoRules
			others[best] = oldMain
			fmt.Printf("Using %s as merge base (%d bookmark(s)).\n", filepath.Base(otherPaths[best]), bestCount)
		}
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

func mtimeOf(path string) time.Time {
	fi, err := os.Stat(path)
	if err != nil {
		return time.Now()
	}
	return fi.ModTime()
}

var (
	orphanURLRe    = regexp.MustCompile(`(?i)<meta[^>]+name=["']liber:url["'][^>]+content=["']([^"']+)["']`)
	orphanFolderRe = regexp.MustCompile(`(?i)<meta[^>]+name=["']liber:folder["'][^>]+content=["']([^"']*)["']`)
	orphanTagsRe   = regexp.MustCompile(`(?i)<meta[^>]+name=["']liber:tags["'][^>]+content=["']([^"']*)["']`)
	orphanTitleRe  = regexp.MustCompile(`(?is)<title[^>]*>(.*?)</title>`)
	orphanLinkRe   = regexp.MustCompile(`(?is)<h1[^>]*>\s*<a[^>]+href=["']([^"']+)["'][^>]*>(.*?)</a>`)
	orphanDescRe   = regexp.MustCompile(`(?is)<p[^>]+class=["']desc["'][^>]*>(.*?)</p>`)
	orphanTagRe    = regexp.MustCompile(`(?i)<[^>]+>`)
)

func parseOrphanHTML(abs string) (url, title, folder string, tags []string, desc string) {
	data, err := os.ReadFile(abs)
	if err != nil {
		return "", "", "", nil, ""
	}
	s := string(data)
	if m := orphanURLRe.FindStringSubmatch(s); m != nil {
		url = strings.TrimSpace(m[1])
	}
	if m := orphanFolderRe.FindStringSubmatch(s); m != nil {
		folder = sanitizeFolder(m[1])
	}
	if m := orphanTagsRe.FindStringSubmatch(s); m != nil {
		for _, t := range strings.Split(m[1], ",") {
			if strings.TrimSpace(t) != "" {
				tags = append(tags, strings.TrimSpace(t))
			}
		}
		tags = dedupe(tags)
	}
	if m := orphanTitleRe.FindStringSubmatch(s); m != nil {
		title = strings.TrimSpace(orphanTagRe.ReplaceAllString(m[1], ""))
	}
	if m := orphanLinkRe.FindStringSubmatch(s); m != nil {
		if url == "" {
			url = strings.TrimSpace(m[1])
		}
		if title == "" {
			title = strings.TrimSpace(orphanTagRe.ReplaceAllString(m[2], ""))
		}
	}
	if m := orphanDescRe.FindStringSubmatch(s); m != nil {
		desc = strings.TrimSpace(orphanTagRe.ReplaceAllString(m[1], ""))
	}
	return url, title, folder, tags, desc
}

func adoptOrphanFiles(cfg Config, store *Store) (adopted, dupMoved int) {
	htmlDir := cfg.htmlDir()
	if !fileExists(htmlDir) {
		return 0, 0
	}
	referenced := map[string]bool{}
	for _, b := range store.Bookmarks {
		if b.HTMLFile != "" {
			referenced[b.HTMLFile] = true
		}
	}
	byURL := map[string]*Bookmark{}
	for _, b := range store.Bookmarks {
		byURL[normalizeForDedupe(b.URL)] = b
	}
	var rels []string
	_ = filepath.Walk(htmlDir, func(p string, info os.FileInfo, err error) error {
		if err != nil || info.IsDir() {
			return nil
		}
		if !strings.HasSuffix(strings.ToLower(info.Name()), ".html") {
			return nil
		}
		rel, err := filepath.Rel(htmlDir, p)
		if err != nil {
			return nil
		}
		rels = append(rels, rel)
		return nil
	})
	sort.Strings(rels)
	unindexedRoot := filepath.Join(cfg.effectiveBaseDir(), "unindexed")
	maxID := 0
	for _, b := range store.Bookmarks {
		if b.ID > maxID {
			maxID = b.ID
		}
	}
	if store.NextID <= maxID {
		store.NextID = maxID + 1
	}
	for _, rel := range rels {
		if referenced[rel] {
			continue
		}
		abs := filepath.Join(htmlDir, rel)
		url, title, _, tags, desc := parseOrphanHTML(abs)
		url = strings.TrimSpace(url)
		if url == "" {
			dst := filepath.Join(unindexedRoot, "html", rel)
			if fileExists(abs) {
				if err := moveFile(abs, dst); err == nil {
					dupMoved++
				}
			}
			continue
		}
		if dup, ok := byURL[normalizeForDedupe(url)]; ok && dup != nil {
			dst := filepath.Join(unindexedRoot, "html", rel)
			if fileExists(abs) {
				if err := moveFile(abs, dst); err == nil {
					dupMoved++
				}
			}
			_ = dup
			continue
		}
		fi, _ := os.Stat(abs)
		ts := time.Now()
		if fi != nil {
			ts = fi.ModTime()
		}
		dir := filepath.Dir(rel)
		folder := ""
		if dir != "." && dir != "" {
			folder = sanitizeFolder(dir)
		}
		if title == "" {
			title = url
		}
		maxID++
		nb := &Bookmark{
			ID:          maxID,
			URL:         url,
			Title:       title,
			Description: desc,
			Tags:        tags,
			Folder:      folder,
			CreatedAt:   ts,
			UpdatedAt:   ts,
			HTMLFile:    rel,
		}
		newBase := strings.TrimSuffix(filepath.Base(rel), filepath.Ext(rel))
		if idx := strings.Index(newBase, "-"); idx > 0 {
			rest := newBase[idx+1:]
			if rest == "" {
				rest = slugOrFallback(title, maxID)
			}
			newBase = fmt.Sprintf("%04d-%s", maxID, rest)
		} else {
			newBase = fmt.Sprintf("%04d-%s", maxID, slugOrFallback(title, maxID))
		}
		newRel := filepath.Join(filepath.Dir(rel), newBase+".html")
		if newRel != rel && filepath.Dir(rel) == "." {
			newRel = newBase + ".html"
		}
		if newRel != rel {
			dst := filepath.Join(htmlDir, newRel)
			if !fileExists(dst) {
				if err := moveFile(abs, dst); err == nil {
					nb.HTMLFile = newRel
					rel = newRel
				}
			}
		}
		oldStem := strings.TrimSuffix(filepath.Base(abs), filepath.Ext(abs))
		_ = oldStem
		attachSiblings(cfg, nb, abs, rel)
		store.Bookmarks = append(store.Bookmarks, nb)
		referenced[nb.HTMLFile] = true
		byURL[normalizeForDedupe(url)] = nb
		adopted++
		fmt.Printf("  adopted %s as [%d] %s\n", nb.HTMLFile, nb.ID, nb.Title)
	}
	if adopted > 0 {
		maxID = 0
		for _, b := range store.Bookmarks {
			if b.ID > maxID {
				maxID = b.ID
			}
		}
		store.NextID = maxID + 1
	}
	return adopted, dupMoved
}

func attachSiblings(cfg Config, nb *Bookmark, oldAbs, newRel string) {
	oldBase := strings.TrimSuffix(filepath.Base(oldAbs), filepath.Ext(oldAbs))
	newBase := strings.TrimSuffix(filepath.Base(newRel), filepath.Ext(filepath.Base(newRel)))
	dir := filepath.Dir(newRel)
	if dir == "." {
		dir = ""
	}
	mdOldRel := filepath.Join(dir, oldBase+".md")
	if dir == "" {
		mdOldRel = oldBase + ".md"
	}
	mdOldAbs := filepath.Join(cfg.markdownDir(), mdOldRel)
	if fileExists(mdOldAbs) {
		mdNewRel := filepath.Join(dir, newBase+".md")
		if dir == "" {
			mdNewRel = newBase + ".md"
		}
		mdNewAbs := filepath.Join(cfg.markdownDir(), mdNewRel)
		if !fileExists(mdNewAbs) {
			if err := moveFile(mdOldAbs, mdNewAbs); err == nil {
				nb.MarkdownFile = mdNewRel
			} else {
				nb.MarkdownFile = mdOldRel
			}
		} else {
			nb.MarkdownFile = mdOldRel
		}
	}
	archOldRel := filepath.Join(dir, oldBase+".html")
	if dir == "" {
		archOldRel = oldBase + ".html"
	}
	archOldAbs := filepath.Join(cfg.archiveDir(), archOldRel)
	if fileExists(archOldAbs) {
		archNewRel := filepath.Join(dir, newBase+".html")
		if dir == "" {
			archNewRel = newBase + ".html"
		}
		archNewAbs := filepath.Join(cfg.archiveDir(), archNewRel)
		if !fileExists(archNewAbs) {
			if err := moveFile(archOldAbs, archNewAbs); err == nil {
				nb.ArchiveFile = archNewRel
			} else {
				nb.ArchiveFile = archOldRel
			}
		} else {
			nb.ArchiveFile = archOldRel
		}
	}
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
