package main

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

const journalRetention = 90 * 24 * time.Hour

type JournalTombstone struct {
	ID   int       `json:"id"`
	URL  string    `json:"url"`
	Time time.Time `json:"time"`
}

type JournalRuleTombstone struct {
	ID    int    `json:"id"`
	Match string `json:"match"`
}

type JournalEntry struct {
	ID           string                 `json:"id"`
	Device       string                 `json:"device"`
	Time         time.Time              `json:"time"`
	Bookmarks    []Bookmark             `json:"bookmarks,omitempty"`
	Deleted      []JournalTombstone     `json:"deleted,omitempty"`
	Rules        []AutoRule             `json:"rules,omitempty"`
	DeletedRules []JournalRuleTombstone `json:"deleted_rules,omitempty"`
}

func (e *JournalEntry) empty() bool {
	return e == nil || (len(e.Bookmarks) == 0 && len(e.Deleted) == 0 && len(e.Rules) == 0 && len(e.DeletedRules) == 0)
}

func snapshotBookmark(b *Bookmark) Bookmark {
	cp := *b
	cp.Tags = append([]string{}, b.Tags...)
	cp.Attachments = append([]Attachment{}, b.Attachments...)
	cp.AppliedRules = append([]AppliedAutoRule{}, b.AppliedRules...)
	return cp
}

func journalUpserts(in []*Bookmark) *JournalEntry {
	e := &JournalEntry{}
	for _, b := range in {
		if b == nil {
			continue
		}
		cp := snapshotBookmark(b)
		e.Bookmarks = append(e.Bookmarks, cp)
	}
	return e
}

func journalDeletes(in []*Bookmark) *JournalEntry {
	e := &JournalEntry{}
	now := time.Now()
	for _, b := range in {
		if b == nil {
			continue
		}
		e.Deleted = append(e.Deleted, JournalTombstone{ID: b.ID, URL: b.URL, Time: now})
	}
	return e
}

func journalRules(in []*AutoRule) *JournalEntry {
	e := &JournalEntry{}
	for _, r := range in {
		if r == nil {
			continue
		}
		e.Rules = append(e.Rules, *r)
	}
	return e
}

func journalRuleDeletes(in []*AutoRule) *JournalEntry {
	e := &JournalEntry{}
	for _, r := range in {
		if r == nil {
			continue
		}
		e.DeletedRules = append(e.DeletedRules, JournalRuleTombstone{ID: r.ID, Match: r.Match})
	}
	return e
}

func (e *JournalEntry) merge(o *JournalEntry) *JournalEntry {
	if o == nil {
		return e
	}
	if e == nil {
		e = &JournalEntry{}
	}
	e.Bookmarks = append(e.Bookmarks, o.Bookmarks...)
	e.Deleted = append(e.Deleted, o.Deleted...)
	e.Rules = append(e.Rules, o.Rules...)
	e.DeletedRules = append(e.DeletedRules, o.DeletedRules...)
	return e
}

func sanitizeDevice(s string) string {
	s = strings.ToLower(strings.TrimSpace(s))
	var sb strings.Builder
	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') || r == '-' {
			sb.WriteRune(r)
		} else {
			sb.WriteRune('-')
		}
	}
	out := strings.Trim(sb.String(), "-")
	if out == "" {
		out = "device"
	}
	if len(out) > 32 {
		out = out[:32]
	}
	return out
}

func ensureDeviceID(cfg Config) (Config, error) {
	if strings.TrimSpace(cfg.DeviceID) != "" {
		return cfg, nil
	}
	host, _ := os.Hostname()
	var rb [3]byte
	if _, err := rand.Read(rb[:]); err != nil {
		return cfg, fmt.Errorf("device id: %w", err)
	}
	cfg.DeviceID = sanitizeDevice(host) + "-" + hex.EncodeToString(rb[:])
	if err := SaveConfig(cfg); err != nil {
		return cfg, fmt.Errorf("saving device id: %w", err)
	}
	return cfg, nil
}

func journalDir(cfg Config) string {
	return filepath.Join(cfg.effectiveBaseDir(), ".liber", "journal")
}

func journalFileName(device string, t time.Time) string {
	var rb [4]byte
	_, _ = rand.Read(rb[:])
	dev := sanitizeDevice(device)
	if dev == "" {
		dev = "device"
	}
	return t.UTC().Format("20060102-150405.000000000") + "-" + dev + "-" + hex.EncodeToString(rb[:]) + ".json"
}

func saveWithJournal(cfg Config, store *Store, e *JournalEntry) error {
	if e != nil && !e.empty() {
		var err error
		cfg, err = ensureDeviceID(cfg)
		if err != nil {
			return err
		}
		if err := appendJournalEntry(cfg, store, e); err != nil {
			return err
		}
	}
	return store.Save()
}

func appendJournalEntry(cfg Config, store *Store, e *JournalEntry) error {
	now := time.Now().UTC()
	e.Time = now
	e.Device = cfg.DeviceID
	if err := os.MkdirAll(journalDir(cfg), 0o755); err != nil {
		return err
	}
	for tries := 0; tries < 3; tries++ {
		e.ID = journalFileName(cfg.DeviceID, now)
		data, err := json.MarshalIndent(e, "", "  ")
		if err != nil {
			return err
		}
		p := filepath.Join(journalDir(cfg), e.ID)
		f, err := os.OpenFile(p, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o644)
		if err != nil {
			if os.IsExist(err) {
				continue
			}
			return err
		}
		_, werr := f.Write(data)
		cerr := f.Close()
		if werr != nil {
			os.Remove(p)
			return werr
		}
		if cerr != nil {
			os.Remove(p)
			return cerr
		}
		store.AppliedJournal = append(store.AppliedJournal, e.ID)
		return nil
	}
	return fmt.Errorf("could not pick a journal filename")
}

func listJournalFiles(cfg Config) []string {
	entries, err := os.ReadDir(journalDir(cfg))
	if err != nil {
		return nil
	}
	var out []string
	for _, e := range entries {
		if e.IsDir() || !strings.HasSuffix(e.Name(), ".json") {
			continue
		}
		out = append(out, filepath.Join(journalDir(cfg), e.Name()))
	}
	sort.Strings(out)
	return out
}

func unappliedJournalCount(cfg Config, store *Store) int {
	seen := map[string]bool{}
	for _, id := range store.AppliedJournal {
		seen[id] = true
	}
	n := 0
	for _, p := range listJournalFiles(cfg) {
		if !seen[filepath.Base(p)] {
			n++
		}
	}
	return n
}

type journalReport struct {
	upserted     []int
	reassigned   [][2]int
	deduped      []int
	deleted      []int
	rulesAdded   int
	rulesDeleted []string
}

func replayJournal(cfg Config, store *Store) (journalReport, []string, error) {
	var rep journalReport
	var skipped []string
	seen := map[string]bool{}
	for _, id := range store.AppliedJournal {
		seen[id] = true
	}
	byID := map[int]*Bookmark{}
	maxID := 0
	for _, b := range store.Bookmarks {
		byID[b.ID] = b
		if b.ID > maxID {
			maxID = b.ID
		}
	}
	byURL := map[string]*Bookmark{}
	for _, b := range store.Bookmarks {
		byURL[normalizeForDedupe(b.URL)] = b
	}
	maxRuleID := 0
	ruleMatch := map[string]*AutoRule{}
	for _, r := range store.AutoRules {
		ruleMatch[r.Match] = r
		if r.ID > maxRuleID {
			maxRuleID = r.ID
		}
	}
	unindexedRoot := filepath.Join(cfg.effectiveBaseDir(), "unindexed")
	for _, p := range listJournalFiles(cfg) {
		id := filepath.Base(p)
		if seen[id] {
			continue
		}
		data, err := os.ReadFile(p)
		if err != nil {
			skipped = append(skipped, p)
			fmt.Fprintf(os.Stderr, "warning: skipping unreadable %s: %v\n", p, err)
			continue
		}
		var e JournalEntry
		if err := json.Unmarshal(data, &e); err != nil {
			skipped = append(skipped, p)
			fmt.Fprintf(os.Stderr, "warning: skipping unparseable %s: %v\n", p, err)
			continue
		}
		for i := range e.Bookmarks {
			ib := &e.Bookmarks[i]
			key := normalizeForDedupe(ib.URL)
			if eb, ok := byID[ib.ID]; ok {
				if normalizeForDedupe(eb.URL) == key {
					mergeBookmarkFields(eb, ib)
					rep.upserted = append(rep.upserted, eb.ID)
				} else if dup, ok := byURL[key]; ok {
					dup.Tags = dedupe(append(dup.Tags, ib.Tags...))
					if dup.UpdatedAt.Before(ib.UpdatedAt) {
						dup.UpdatedAt = ib.UpdatedAt
					}
					rep.deduped = append(rep.deduped, ib.ID)
					quarantineSnapshotFiles(cfg, unindexedRoot, ib)
				} else {
					old := ib.ID
					maxID++
					ib.ID = maxID
					reprefixBookmarkFiles(ib, old, maxID)
					nb := *ib
					base := &nb
					store.Bookmarks = append(store.Bookmarks, base)
					byID[base.ID] = base
					byURL[key] = base
					renameCollisionFiles(cfg, base, old, maxID, false)
					rep.reassigned = append(rep.reassigned, [2]int{old, maxID})
				}
				continue
			}
			if eb, ok := byURL[key]; ok {
				eb.Tags = dedupe(append(eb.Tags, ib.Tags...))
				if eb.UpdatedAt.Before(ib.UpdatedAt) {
					eb.UpdatedAt = ib.UpdatedAt
				}
				rep.deduped = append(rep.deduped, ib.ID)
				quarantineSnapshotFiles(cfg, unindexedRoot, ib)
				continue
			}
			if ib.ID > maxID {
				maxID = ib.ID
			}
			nb := *ib
			base := &nb
			store.Bookmarks = append(store.Bookmarks, base)
			byID[base.ID] = base
			byURL[key] = base
			rep.upserted = append(rep.upserted, base.ID)
		}
		for _, t := range e.Deleted {
			if did, ok := applyTombstone(cfg, store, byID, byURL, unindexedRoot, t); ok {
				rep.deleted = append(rep.deleted, did)
			}
		}
		for i := range e.Rules {
			ir := &e.Rules[i]
			if er, ok := ruleMatch[ir.Match]; ok {
				er.Folder = ir.Folder
				er.Tags = dedupe(ir.Tags)
				if ir.CreatedAt.Before(er.CreatedAt) {
					er.CreatedAt = ir.CreatedAt
				}
				continue
			}
			maxRuleID++
			nr := *ir
			nr.ID = maxRuleID
			npc := nr
			store.AutoRules = append(store.AutoRules, &npc)
			ruleMatch[npc.Match] = &npc
			rep.rulesAdded++
		}
		for _, t := range e.DeletedRules {
			if er, ok := ruleMatch[t.Match]; ok {
				store.DeleteAutoRule(er.ID)
				delete(ruleMatch, t.Match)
				rep.rulesDeleted = append(rep.rulesDeleted, t.Match)
			}
		}
		store.AppliedJournal = append(store.AppliedJournal, id)
		seen[id] = true
	}
	store.NextID = maxID + 1
	if store.NextAutoRuleID <= maxRuleID {
		store.NextAutoRuleID = maxRuleID + 1
	}
	sort.Ints(rep.upserted)
	sort.Ints(rep.deduped)
	sort.Ints(rep.deleted)
	return rep, skipped, nil
}

func quarantineSnapshotFiles(cfg Config, unindexedRoot string, ib *Bookmark) {
	move := func(src, dst string) {
		if !fileExists(src) {
			return
		}
		if err := moveFile(src, dst); err != nil {
			fmt.Fprintf(os.Stderr, "warning: could not move %s: %v\n", src, err)
		}
	}
	if ib.HTMLFile != "" {
		move(filepath.Join(cfg.htmlDir(), ib.HTMLFile),
			filepath.Join(unindexedRoot, "html", ib.HTMLFile))
	}
	if ib.MarkdownFile != "" {
		move(filepath.Join(cfg.markdownDir(), ib.MarkdownFile),
			filepath.Join(unindexedRoot, "markdown", ib.MarkdownFile))
	}
	if ib.ArchiveFile != "" {
		move(filepath.Join(cfg.archiveDir(), ib.ArchiveFile),
			filepath.Join(unindexedRoot, "archive", ib.ArchiveFile))
	}
	for _, at := range ib.Attachments {
		move(filepath.Join(cfg.attachmentsDir(), at.File),
			filepath.Join(unindexedRoot, "attachments", at.File))
	}
}

func applyTombstone(cfg Config, store *Store, byID map[int]*Bookmark, byURL map[string]*Bookmark, unindexedRoot string, t JournalTombstone) (int, bool) {
	var target *Bookmark
	if b, ok := byID[t.ID]; ok {
		target = b
	} else if b, ok := byURL[normalizeForDedupe(t.URL)]; ok {
		target = b
	}
	if target == nil {
		return 0, false
	}
	if target.UpdatedAt.After(t.Time) {
		return 0, false
	}
	did := target.ID
	b := &Bookmark{HTMLFile: target.HTMLFile, MarkdownFile: target.MarkdownFile, ArchiveFile: target.ArchiveFile, Attachments: target.Attachments}
	if target.HTMLFile != "" {
		src := filepath.Join(cfg.htmlDir(), target.HTMLFile)
		dst := filepath.Join(unindexedRoot, "html", target.HTMLFile)
		if fileExists(src) {
			if err := moveFile(src, dst); err != nil {
				fmt.Fprintf(os.Stderr, "warning: could not move %s: %v\n", src, err)
			}
		}
	}
	quarantineOrphans(cfg, unindexedRoot, b)
	store.Delete(did)
	delete(byID, did)
	if cur, ok := byURL[normalizeForDedupe(target.URL)]; ok && cur.ID == did {
		delete(byURL, normalizeForDedupe(target.URL))
	}
	return did, true
}

func pruneOldJournal(cfg Config, store *Store, olderThan time.Duration) (deleted, kept int, skipped []string) {
	seen := map[string]bool{}
	for _, id := range store.AppliedJournal {
		seen[id] = true
	}
	now := time.Now()
	for _, p := range listJournalFiles(cfg) {
		id := filepath.Base(p)
		ts, ok := journalEntryTime(p)
		if !ok {
			skipped = append(skipped, p)
			continue
		}
		if now.Sub(ts) <= olderThan {
			continue
		}
		if !seen[id] {
			kept++
			continue
		}
		if err := os.Remove(p); err != nil {
			skipped = append(skipped, p)
			fmt.Fprintf(os.Stderr, "warning: could not prune %s: %v\n", p, err)
			continue
		}
		deleted++
	}
	return deleted, kept, skipped
}

func journalEntryTime(p string) (time.Time, bool) {
	data, err := os.ReadFile(p)
	if err != nil {
		return time.Time{}, false
	}
	var e JournalEntry
	if err := json.Unmarshal(data, &e); err != nil || e.Time.IsZero() {
		if fi, serr := os.Stat(p); serr == nil {
			return fi.ModTime(), true
		}
		return time.Time{}, false
	}
	return e.Time, true
}
