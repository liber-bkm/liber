package main

import (
	"io"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func setupReindexTest(t *testing.T, entries []*Bookmark) (Config, string) {
	t.Helper()
	tmp := t.TempDir()
	base := filepath.Join(tmp, "Bookmarks")
	for _, d := range []string{"html", "markdown", "archive", "attachments", ".liber"} {
		if err := os.MkdirAll(filepath.Join(base, d), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	cfgHome := filepath.Join(tmp, "cfghome")
	if err := os.MkdirAll(filepath.Join(cfgHome, "liber"), 0o755); err != nil {
		t.Fatal(err)
	}
	cfg := Config{BaseDir: base}
	if err := os.WriteFile(filepath.Join(cfgHome, "liber", "config.json"), []byte("{\"base_dir\": \""+base+"\"}"), 0o644); err != nil {
		t.Fatal(err)
	}
	t.Setenv("XDG_CONFIG_HOME", cfgHome)
	maxID := 0
	for _, b := range entries {
		if b.ID > maxID {
			maxID = b.ID
		}
	}
	s := &Store{NextID: maxID + 1, NextAutoRuleID: 1, Bookmarks: entries}
	s.path = filepath.Join(base, ".liber", "index.json")
	if err := s.Save(); err != nil {
		t.Fatal(err)
	}
	return cfg, base
}

func writeHTMLFile(t *testing.T, base, rel, url, title string) {
	t.Helper()
	p := filepath.Join(base, "html", rel)
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		t.Fatal(err)
	}
	body := `<!DOCTYPE html><html><head><title>` + title + `</title><meta name="liber:url" content="` + url + `"></head><body></body></html>`
	if err := os.WriteFile(p, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
}

func loadTestStore(t *testing.T, base string) *Store {
	t.Helper()
	s, err := LoadStore(filepath.Join(base, ".liber", "index.json"))
	if err != nil {
		t.Fatal(err)
	}
	return s
}

func TestAdoptOrphanFiles(t *testing.T) {
	base := t.TempDir()
	cfg := Config{BaseDir: base}
	if err := os.MkdirAll(cfg.htmlDir(), 0o755); err != nil {
		t.Fatal(err)
	}
	store := &Store{NextID: 2, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}}
	if err := os.WriteFile(filepath.Join(cfg.htmlDir(), "0001-a.html"), []byte("<html></html>"), 0o644); err != nil {
		t.Fatal(err)
	}
	orphan := `<!DOCTYPE html><html><head><title>orphan title</title><meta name="liber:url" content="https://orphan.com"></head><body></body></html>`
	if err := os.WriteFile(filepath.Join(cfg.htmlDir(), "0009-orphan.html"), []byte(orphan), 0o644); err != nil {
		t.Fatal(err)
	}
	adopted, _ := adoptOrphanFiles(io.Discard, cfg, store)
	if adopted != 1 {
		t.Fatalf("adopted = %d", adopted)
	}
	if len(store.Bookmarks) != 2 {
		t.Fatalf("bookmarks = %+v", store.Bookmarks)
	}
	nb := store.Find(2)
	if nb == nil || nb.URL != "https://orphan.com" {
		t.Fatalf("new bookmark = %+v", nb)
	}
	if nb.HTMLFile != "0002-orphan.html" {
		t.Fatalf("html = %q", nb.HTMLFile)
	}
	if !fileExists(filepath.Join(cfg.htmlDir(), "0002-orphan.html")) {
		t.Fatalf("renamed file missing")
	}
}

func TestAdoptOrphanDuplicateMovesToUnindexed(t *testing.T) {
	base := t.TempDir()
	cfg := Config{BaseDir: base}
	if err := os.MkdirAll(cfg.htmlDir(), 0o755); err != nil {
		t.Fatal(err)
	}
	store := &Store{NextID: 2, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}}
	if err := os.WriteFile(filepath.Join(cfg.htmlDir(), "0001-a.html"), []byte("<html></html>"), 0o644); err != nil {
		t.Fatal(err)
	}
	dup := `<!DOCTYPE html><html><head><title>a</title><meta name="liber:url" content="https://a.com"></head><body></body></html>`
	if err := os.WriteFile(filepath.Join(cfg.htmlDir(), "0009-a-dup.html"), []byte(dup), 0o644); err != nil {
		t.Fatal(err)
	}
	adopted, dupMoved := adoptOrphanFiles(io.Discard, cfg, store)
	if adopted != 0 {
		t.Fatalf("adopted = %d", adopted)
	}
	if dupMoved != 1 {
		t.Fatalf("dupMoved = %d", dupMoved)
	}
	if len(store.Bookmarks) != 1 {
		t.Fatalf("bookmarks = %d", len(store.Bookmarks))
	}
	if !fileExists(filepath.Join(base, "unindexed", "html", "0009-a-dup.html")) {
		t.Fatalf("duplicate not quarantined")
	}
}

func TestRelinkSiblings(t *testing.T) {
	base := t.TempDir()
	cfg := Config{BaseDir: base}
	for _, d := range []string{cfg.htmlDir(), cfg.markdownDir(), cfg.archiveDir()} {
		if err := os.MkdirAll(d, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	store := &Store{NextID: 2, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}}
	if err := os.WriteFile(filepath.Join(cfg.htmlDir(), "0001-a.html"), []byte("<html></html>"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(cfg.markdownDir(), "0001-a.md"), []byte("# a\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(cfg.archiveDir(), "0001-a.html"), []byte("<html></html>"), 0o644); err != nil {
		t.Fatal(err)
	}
	if n := relinkSiblings(cfg, store); n != 2 {
		t.Fatalf("relinked = %d", n)
	}
	b := store.Find(1)
	if b.MarkdownFile != "0001-a.md" || b.ArchiveFile != "0001-a.html" {
		t.Fatalf("relinked refs = %+v", b)
	}
}

func TestFindMergeCandidatesAll(t *testing.T) {
	dir := t.TempDir()
	for _, n := range []string{"index.json", "index.sync-conflict-a.json", "index (1).json", "notes.txt"} {
		if err := os.WriteFile(filepath.Join(dir, n), []byte("{}"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if got := findMergeCandidates(dir, false); len(got) != 1 {
		t.Fatalf("conflict only = %v", got)
	}
	if got := findMergeCandidates(dir, true); len(got) != 2 {
		t.Fatalf("all = %v", got)
	}
}

func TestSweepContentConflicts(t *testing.T) {
	base := t.TempDir()
	cfg := Config{BaseDir: base}
	for _, d := range []string{cfg.htmlDir(), cfg.markdownDir()} {
		if err := os.MkdirAll(d, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	conflictHTML := "0001-a.sync-conflict-20240101-ABCDEF.html"
	if err := os.WriteFile(filepath.Join(cfg.htmlDir(), conflictHTML), []byte("<html></html>"), 0o644); err != nil {
		t.Fatal(err)
	}
	conflictMD := "0001-a (conflicted copy 2024-01-01).md"
	if err := os.WriteFile(filepath.Join(cfg.markdownDir(), conflictMD), []byte("# a\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	plain := "0001-a.html"
	if err := os.WriteFile(filepath.Join(cfg.htmlDir(), plain), []byte("<html></html>"), 0o644); err != nil {
		t.Fatal(err)
	}
	moved := sweepContentConflicts(cfg, filepath.Join(base, "unindexed"))
	if len(moved) != 2 {
		t.Fatalf("moved = %v", moved)
	}
	if !fileExists(filepath.Join(base, "unindexed", "html", conflictHTML)) {
		t.Fatalf("html conflict not preserved")
	}
	if !fileExists(filepath.Join(base, "unindexed", "markdown", conflictMD)) {
		t.Fatalf("md conflict not preserved")
	}
	if !fileExists(filepath.Join(cfg.htmlDir(), plain)) {
		t.Fatalf("plain file should stay")
	}
}

func TestRelinkOrphanAttachments(t *testing.T) {
	base := t.TempDir()
	cfg := Config{BaseDir: base}
	if err := os.MkdirAll(cfg.attachmentsDir(), 0o755); err != nil {
		t.Fatal(err)
	}
	store := &Store{NextID: 3, Bookmarks: []*Bookmark{
		{ID: 2, URL: "https://b.com", Title: "b"},
	}}
	for _, n := range []string{"0002-report.pdf", "notes.pdf", "0009-ghost.pdf"} {
		if err := os.WriteFile(filepath.Join(cfg.attachmentsDir(), n), []byte("x"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	relinked, quarantined := relinkOrphanAttachments(io.Discard, cfg, store)
	if relinked != 1 {
		t.Fatalf("relinked = %d", relinked)
	}
	if quarantined != 2 {
		t.Fatalf("quarantined = %d", quarantined)
	}
	b := store.Find(2)
	if len(b.Attachments) != 1 || b.Attachments[0].File != "0002-report.pdf" {
		t.Fatalf("attachments = %+v", b.Attachments)
	}
	if !fileExists(filepath.Join(base, "unindexed", "attachments", "notes.pdf")) {
		t.Fatalf("notes.pdf not quarantined")
	}
	if !fileExists(filepath.Join(base, "unindexed", "attachments", "0009-ghost.pdf")) {
		t.Fatalf("ghost not quarantined")
	}
}

func TestAdoptSkipsContentConflicts(t *testing.T) {
	base := t.TempDir()
	cfg := Config{BaseDir: base}
	if err := os.MkdirAll(cfg.htmlDir(), 0o755); err != nil {
		t.Fatal(err)
	}
	store := &Store{NextID: 1}
	name := "0001-a.sync-conflict-20240101-ABCDEF.html"
	if err := os.WriteFile(filepath.Join(cfg.htmlDir(), name), []byte("<html></html>"), 0o644); err != nil {
		t.Fatal(err)
	}
	adopted, _ := adoptOrphanFiles(io.Discard, cfg, store)
	if adopted != 0 {
		t.Fatalf("adopted = %d", adopted)
	}
	if len(store.Bookmarks) != 0 {
		t.Fatalf("bookmarks = %d", len(store.Bookmarks))
	}
	if !fileExists(filepath.Join(cfg.htmlDir(), name)) {
		t.Fatalf("conflict file should be left for sweep")
	}
}

func TestRunReindexPendingKeptThenPruned(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
		{ID: 2, URL: "https://missing.com", Title: "missing", HTMLFile: "0002-missing.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	if err := runReindex(nil); err != nil {
		t.Fatal(err)
	}
	s := loadTestStore(t, base)
	if len(s.Bookmarks) != 2 {
		t.Fatalf("pending entry should be kept, got %d", len(s.Bookmarks))
	}
	if err := runReindex([]string{"--prune"}); err != nil {
		t.Fatal(err)
	}
	s = loadTestStore(t, base)
	if len(s.Bookmarks) != 1 || s.Find(1) == nil {
		t.Fatalf("prune should drop missing entry, got %+v", s.Bookmarks)
	}
}

func TestRunReindexNoCompactByDefault(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
		{ID: 3, URL: "https://c.com", Title: "c", HTMLFile: "0003-c.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	writeHTMLFile(t, base, "0003-c.html", "https://c.com", "c")
	if err := runReindex(nil); err != nil {
		t.Fatal(err)
	}
	s := loadTestStore(t, base)
	if s.Find(1) == nil || s.Find(3) == nil {
		t.Fatalf("ids should stay 1,3 without --compact, got %+v", s.Bookmarks)
	}
	if !fileExists(filepath.Join(base, "html", "0003-c.html")) {
		t.Fatalf("0003 file should stay without --compact")
	}
	if err := runReindex([]string{"--compact"}); err != nil {
		t.Fatal(err)
	}
	s = loadTestStore(t, base)
	if s.Find(1) == nil || s.Find(2) == nil || s.Find(3) != nil {
		t.Fatalf("compact should renumber 3->2, got %+v", s.Bookmarks)
	}
	if !fileExists(filepath.Join(base, "html", "0002-c.html")) {
		t.Fatalf("0002 file missing after --compact")
	}
}

func TestRunReindexMergeUserExample(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://google.com", Title: "google", HTMLFile: "0001-google.html"},
		{ID: 2, URL: "https://reddit.com", Title: "reddit", HTMLFile: "0002-reddit.html"},
		{ID: 3, URL: "https://youtube.com", Title: "youtube", HTMLFile: "0003-youtube.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-google.html", "https://google.com", "google")
	writeHTMLFile(t, base, "0002-reddit.html", "https://reddit.com", "reddit")
	writeHTMLFile(t, base, "0003-youtube.html", "https://youtube.com", "youtube")
	conflict := &Store{NextID: 4, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://reddit.com", Title: "reddit", HTMLFile: "0001-reddit.html"},
		{ID: 2, URL: "https://facebook.com", Title: "facebook", HTMLFile: "0002-facebook.html"},
		{ID: 3, URL: "https://netflix.com", Title: "netflix", HTMLFile: "0003-netflix.html"},
	}}
	conflict.path = filepath.Join(base, ".liber", "index.sync-conflict-test.json")
	if err := conflict.Save(); err != nil {
		t.Fatal(err)
	}
	writeHTMLFile(t, base, "0001-reddit.html", "https://reddit.com", "reddit")
	writeHTMLFile(t, base, "0002-facebook.html", "https://facebook.com", "facebook")
	writeHTMLFile(t, base, "0003-netflix.html", "https://netflix.com", "netflix")
	if err := runReindex([]string{"--merge"}); err != nil {
		t.Fatal(err)
	}
	s := loadTestStore(t, base)
	if len(s.Bookmarks) != 5 {
		t.Fatalf("want 5 bookmarks, got %+v", s.Bookmarks)
	}
	want := map[string]bool{
		"https://google.com": false, "https://reddit.com": false, "https://youtube.com": false,
		"https://facebook.com": false, "https://netflix.com": false,
	}
	counts := map[string]int{}
	for _, b := range s.Bookmarks {
		counts[normalizeForDedupe(b.URL)]++
		if _, ok := want[normalizeForDedupe(b.URL)]; !ok {
			if _, ok2 := want[b.URL]; !ok2 {
				t.Fatalf("unexpected url %q", b.URL)
			}
		}
	}
	for u := range want {
		if counts[normalizeForDedupe(u)] != 1 {
			t.Fatalf("url %s count = %d, want 1", u, counts[normalizeForDedupe(u)])
		}
	}
	if !fileExists(filepath.Join(base, ".liber", "resolved", "index.sync-conflict-test.json")) {
		t.Fatalf("conflict copy should move to resolved")
	}
}

func TestRunReindexMergeTieOldestWins(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
		{ID: 2, URL: "https://b.com", Title: "b", HTMLFile: "0002-b.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	writeHTMLFile(t, base, "0002-b.html", "https://b.com", "b")
	conflict := &Store{NextID: 3, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://x.com", Title: "x", HTMLFile: "0001-x.html"},
		{ID: 2, URL: "https://y.com", Title: "y", HTMLFile: "0002-y.html"},
	}}
	cpath := filepath.Join(base, ".liber", "index.sync-conflict-old.json")
	conflict.path = cpath
	if err := conflict.Save(); err != nil {
		t.Fatal(err)
	}
	writeHTMLFile(t, base, "0001-x.html", "https://x.com", "x")
	writeHTMLFile(t, base, "0002-y.html", "https://y.com", "y")
	old := time.Now().Add(-time.Hour)
	now := time.Now()
	if err := os.Chtimes(cpath, old, old); err != nil {
		t.Fatal(err)
	}
	if err := os.Chtimes(filepath.Join(base, ".liber", "index.json"), now, now); err != nil {
		t.Fatal(err)
	}
	if err := runReindex([]string{"--merge"}); err != nil {
		t.Fatal(err)
	}
	s := loadTestStore(t, base)
	if len(s.Bookmarks) != 4 {
		t.Fatalf("want 4 bookmarks, got %+v", s.Bookmarks)
	}
	if b := s.Find(1); b == nil || normalizeForDedupe(b.URL) != normalizeForDedupe("https://x.com") {
		t.Fatalf("oldest copy should be base, [1] = %+v", b)
	}
}

func TestRunReindexMergeAllDriveCopy(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	drive := &Store{NextID: 3, Bookmarks: []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
		{ID: 2, URL: "https://bb.com", Title: "bb", HTMLFile: "0002-bb.html"},
	}}
	drive.path = filepath.Join(base, ".liber", "index (1).json")
	if err := drive.Save(); err != nil {
		t.Fatal(err)
	}
	writeHTMLFile(t, base, "0002-bb.html", "https://bb.com", "bb")
	if got := findConflictCopies(filepath.Join(base, ".liber")); len(got) != 0 {
		t.Fatalf("drive copy should not match default scan, got %v", got)
	}
	if err := runReindex([]string{"--merge", "--all"}); err != nil {
		t.Fatal(err)
	}
	s := loadTestStore(t, base)
	if len(s.Bookmarks) != 2 || s.Find(2) == nil {
		t.Fatalf("merge --all should fold drive copy, got %+v", s.Bookmarks)
	}
	if !fileExists(filepath.Join(base, ".liber", "resolved", "index (1).json")) {
		t.Fatalf("drive copy should move to resolved")
	}
}
