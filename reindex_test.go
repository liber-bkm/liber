package main

import (
	"os"
	"path/filepath"
	"testing"
)

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
	adopted, _ := adoptOrphanFiles(cfg, store)
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
	adopted, dupMoved := adoptOrphanFiles(cfg, store)
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
