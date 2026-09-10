package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func journalTestCfg(t *testing.T, device string) (Config, string) {
	t.Helper()
	base := t.TempDir()
	for _, d := range []string{"html", "markdown", "archive", "attachments", ".liber"} {
		if err := os.MkdirAll(filepath.Join(base, d), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	return Config{BaseDir: base, DeviceID: device}, base
}

func copyJournal(t *testing.T, src, dst Config) {
	t.Helper()
	if err := os.MkdirAll(journalDir(dst), 0o755); err != nil {
		t.Fatal(err)
	}
	for _, p := range listJournalFiles(src) {
		data, err := os.ReadFile(p)
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(journalDir(dst), filepath.Base(p)), data, 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func TestEnsureDeviceID(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", t.TempDir())
	cfg := Config{BaseDir: t.TempDir()}
	out, err := ensureDeviceID(cfg)
	if err != nil {
		t.Fatal(err)
	}
	if out.DeviceID == "" {
		t.Fatalf("empty device id")
	}
	again, err := ensureDeviceID(out)
	if err != nil {
		t.Fatal(err)
	}
	if again.DeviceID != out.DeviceID {
		t.Fatalf("device id not stable: %q vs %q", again.DeviceID, out.DeviceID)
	}
	if sanitizeDevice("My Laptop/Work!") == "" {
		t.Fatalf("sanitize empty")
	}
}

func TestJournalAddReplay(t *testing.T) {
	cfgA, baseA := journalTestCfg(t, "dev-a")
	storeA := &Store{NextID: 2, NextAutoRuleID: 1, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}}
	storeA.path = filepath.Join(baseA, ".liber", "index.json")
	if err := saveWithJournal(cfgA, storeA, journalUpserts(storeA.Bookmarks)); err != nil {
		t.Fatal(err)
	}
	files := listJournalFiles(cfgA)
	if len(files) != 1 {
		t.Fatalf("files = %v", files)
	}
	if len(storeA.AppliedJournal) != 1 {
		t.Fatalf("originator should mark own entry applied")
	}
	cfgB, _ := journalTestCfg(t, "dev-b")
	storeB := &Store{NextID: 1, NextAutoRuleID: 1}
	copyJournal(t, cfgA, cfgB)
	rep, skipped, err := replayJournal(cfgB, storeB)
	if err != nil {
		t.Fatal(err)
	}
	if len(skipped) != 0 || len(rep.upserted) != 1 {
		t.Fatalf("rep = %+v skipped = %v", rep, skipped)
	}
	b := storeB.Find(1)
	if b == nil || b.URL != "https://a.com" {
		t.Fatalf("replayed = %+v", b)
	}
	rep2, _, err := replayJournal(cfgB, storeB)
	if err != nil {
		t.Fatal(err)
	}
	if len(rep2.upserted) != 0 || len(storeB.Bookmarks) != 1 {
		t.Fatalf("replay not idempotent: %+v", rep2)
	}
}

func TestJournalCollisionKeepsBoth(t *testing.T) {
	cfgB, baseB := journalTestCfg(t, "dev-b")
	storeB := &Store{NextID: 2, NextAutoRuleID: 1, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://reddit.com", Title: "reddit", HTMLFile: "0001-reddit.html"},
	}}
	incoming := &JournalEntry{Bookmarks: []Bookmark{
		{ID: 1, URL: "https://google.com", Title: "google", HTMLFile: "0001-google.html"},
	}}
	cfgA, _ := journalTestCfg(t, "dev-a")
	storeA := &Store{}
	if err := appendJournalEntry(cfgA, storeA, incoming); err != nil {
		t.Fatal(err)
	}
	copyJournal(t, cfgA, cfgB)
	rep, _, err := replayJournal(cfgB, storeB)
	if err != nil {
		t.Fatal(err)
	}
	if len(rep.reassigned) != 1 {
		t.Fatalf("rep = %+v", rep)
	}
	if len(storeB.Bookmarks) != 2 {
		t.Fatalf("both should survive, got %+v", storeB.Bookmarks)
	}
	_ = baseB
}

func TestJournalTombstone(t *testing.T) {
	cfg, base := journalTestCfg(t, "dev-b")
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	old := time.Now().Add(-time.Hour)
	store := &Store{NextID: 2, NextAutoRuleID: 1, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html", UpdatedAt: old},
	}}
	tomb := &JournalEntry{Deleted: []JournalTombstone{{ID: 1, URL: "https://a.com", Time: time.Now()}}}
	cfgSrc, _ := journalTestCfg(t, "dev-a")
	dummy := &Store{}
	if err := appendJournalEntry(cfgSrc, dummy, tomb); err != nil {
		t.Fatal(err)
	}
	copyJournal(t, cfgSrc, cfg)
	rep, _, err := replayJournal(cfg, store)
	if err != nil {
		t.Fatal(err)
	}
	if len(rep.deleted) != 1 || store.Find(1) != nil {
		t.Fatalf("rep = %+v store = %+v", rep, store.Bookmarks)
	}
	if !fileExists(filepath.Join(base, "unindexed", "html", "0001-a.html")) {
		t.Fatalf("deleted files should quarantine")
	}
}

func TestJournalTombstoneEditWins(t *testing.T) {
	cfg, _ := journalTestCfg(t, "dev-b")
	store := &Store{NextID: 2, NextAutoRuleID: 1, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a new", HTMLFile: "0001-a.html", UpdatedAt: time.Now()},
	}}
	tomb := &JournalEntry{Deleted: []JournalTombstone{{ID: 1, URL: "https://a.com", Time: time.Now().Add(-time.Hour)}}}
	cfgSrc, _ := journalTestCfg(t, "dev-a")
	dummy := &Store{}
	if err := appendJournalEntry(cfgSrc, dummy, tomb); err != nil {
		t.Fatal(err)
	}
	copyJournal(t, cfgSrc, cfg)
	rep, _, err := replayJournal(cfg, store)
	if err != nil {
		t.Fatal(err)
	}
	if len(rep.deleted) != 0 || store.Find(1) == nil {
		t.Fatalf("newer edit should survive tombstone: %+v", rep)
	}
}

func TestJournalRules(t *testing.T) {
	cfg, _ := journalTestCfg(t, "dev-b")
	store := &Store{NextID: 1, NextAutoRuleID: 1}
	cfgSrc, _ := journalTestCfg(t, "dev-a")
	dummy := &Store{}
	add := &JournalEntry{Rules: []AutoRule{{ID: 1, Match: "host:x.com", Folder: "x"}}}
	if err := appendJournalEntry(cfgSrc, dummy, add); err != nil {
		t.Fatal(err)
	}
	copyJournal(t, cfgSrc, cfg)
	rep, _, err := replayJournal(cfg, store)
	if err != nil {
		t.Fatal(err)
	}
	if rep.rulesAdded != 1 || len(store.AutoRules) != 1 {
		t.Fatalf("rep = %+v rules = %+v", rep, store.AutoRules)
	}
	del := &JournalEntry{DeletedRules: []JournalRuleTombstone{{ID: 9, Match: "host:x.com"}}}
	dummy2 := &Store{}
	if err := appendJournalEntry(cfgSrc, dummy2, del); err != nil {
		t.Fatal(err)
	}
	copyJournal(t, cfgSrc, cfg)
	rep, _, err = replayJournal(cfg, store)
	if err != nil {
		t.Fatal(err)
	}
	if len(rep.rulesDeleted) != 1 || len(store.AutoRules) != 0 {
		t.Fatalf("rep = %+v rules = %+v", rep, store.AutoRules)
	}
}

func TestJournalPrune(t *testing.T) {
	cfg, _ := journalTestCfg(t, "dev-a")
	store := &Store{NextID: 1, NextAutoRuleID: 1}
	oldEntry := &JournalEntry{Bookmarks: []Bookmark{{ID: 1, URL: "https://a.com", Title: "a"}}}
	if err := appendJournalEntry(cfg, store, oldEntry); err != nil {
		t.Fatal(err)
	}
	files := listJournalFiles(cfg)
	if len(files) != 1 {
		t.Fatalf("files = %v", files)
	}
	old := time.Now().Add(-100 * 24 * time.Hour)
	if err := os.Chtimes(files[0], old, old); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(files[0])
	if err != nil {
		t.Fatal(err)
	}
	var e JournalEntry
	if err := json.Unmarshal(data, &e); err != nil {
		t.Fatal(err)
	}
	e.Time = old
	redata, err := json.Marshal(e)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(files[0], redata, 0o644); err != nil {
		t.Fatal(err)
	}
	deleted, kept, skipped := pruneOldJournal(cfg, store, journalRetention)
	if deleted != 1 || kept != 0 || len(skipped) != 0 {
		t.Fatalf("deleted=%d kept=%d skipped=%v", deleted, kept, skipped)
	}
	fresh := &JournalEntry{Bookmarks: []Bookmark{{ID: 2, URL: "https://b.com", Title: "b"}}}
	if err := appendJournalEntry(cfg, store, fresh); err != nil {
		t.Fatal(err)
	}
	deleted, _, _ = pruneOldJournal(cfg, store, journalRetention)
	if deleted != 0 {
		t.Fatalf("fresh entry should survive prune")
	}
}
