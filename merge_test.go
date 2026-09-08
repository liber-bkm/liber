package main

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func mergeBase() *Store {
	now := time.Now()
	s := &Store{NextID: 3, NextAutoRuleID: 1}
	s.Bookmarks = []*Bookmark{
		{ID: 1, URL: "https://a.com/x", Title: "Ax", Tags: []string{"t1"}, UpdatedAt: now},
		{ID: 2, URL: "https://b.com/y", Title: "By", Tags: []string{"t2"}, UpdatedAt: now},
	}
	return s
}

func TestMergeSameIDSameURL(t *testing.T) {
	base := mergeBase()
	other := &Store{NextID: 2}
	later := time.Now().Add(time.Hour)
	other.Bookmarks = []*Bookmark{
		{ID: 1, URL: "https://a.com/x", Title: "Ax new", Tags: []string{"t3"}, UpdatedAt: later},
	}
	rep := mergeStores(base, []*Store{other})
	if len(rep.merged) != 1 || rep.merged[0] != 1 {
		t.Fatalf("merged = %+v", rep)
	}
	b := base.Find(1)
	if b.Title != "Ax new" {
		t.Fatalf("title = %q", b.Title)
	}
	for _, want := range []string{"t1", "t3"} {
		found := false
		for _, tag := range b.Tags {
			if tag == want {
				found = true
			}
		}
		if !found {
			t.Fatalf("tags = %v, want %q", b.Tags, want)
		}
	}
}

func TestMergeCollisionReassign(t *testing.T) {
	base := mergeBase()
	other := &Store{NextID: 2}
	other.Bookmarks = []*Bookmark{
		{ID: 2, URL: "https://c.com/z", Title: "Cz", HTMLFile: "0002-cz.html", UpdatedAt: time.Now()},
	}
	rep := mergeStores(base, []*Store{other})
	if len(rep.reassigned) != 1 || rep.reassigned[0] != [2]int{2, 3} {
		t.Fatalf("reassigned = %+v", rep)
	}
	b := base.Find(3)
	if b == nil || b.URL != "https://c.com/z" {
		t.Fatalf("reassigned entry = %+v", b)
	}
	if b.HTMLFile != "0003-cz.html" {
		t.Fatalf("html = %q", b.HTMLFile)
	}
	if base.NextID != 4 {
		t.Fatalf("next = %d", base.NextID)
	}
}

func TestMergeDedupeURL(t *testing.T) {
	base := mergeBase()
	other := &Store{NextID: 2}
	other.Bookmarks = []*Bookmark{
		{ID: 9, URL: "https://a.com/x/", Title: "Ax dup", Tags: []string{"t9"}, UpdatedAt: time.Now()},
	}
	rep := mergeStores(base, []*Store{other})
	if len(rep.deduped) != 1 {
		t.Fatalf("deduped = %+v", rep)
	}
	if len(base.Bookmarks) != 2 {
		t.Fatalf("bookmarks = %d", len(base.Bookmarks))
	}
}

func TestMergeNewEntriesAndRules(t *testing.T) {
	base := mergeBase()
	other := &Store{NextID: 2, NextAutoRuleID: 2}
	other.Bookmarks = []*Bookmark{
		{ID: 7, URL: "https://n.com/", Title: "N", UpdatedAt: time.Now()},
	}
	other.AutoRules = []*AutoRule{
		{ID: 1, Match: "host:n.com", Folder: "n"},
		{ID: 2, Match: "host:n.com", Folder: "n2"},
	}
	rep := mergeStores(base, []*Store{other})
	if rep.rulesAdded != 1 {
		t.Fatalf("rulesAdded = %+v", rep)
	}
	if len(base.Bookmarks) != 3 || base.Find(7) == nil {
		t.Fatalf("bookmarks = %+v", base.Bookmarks)
	}
	if base.NextID != 8 || base.NextAutoRuleID != 2 {
		t.Fatalf("next = %d rule next = %d", base.NextID, base.NextAutoRuleID)
	}
}

func TestFindConflictCopies(t *testing.T) {
	dir := t.TempDir()
	for _, n := range []string{"index.json", "index.sync-conflict-20240101-abc.json", "index (conflicted copy 2024).json", "notes.txt"} {
		if err := os.WriteFile(filepath.Join(dir, n), []byte("{}"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	got := findConflictCopies(dir)
	if len(got) != 2 {
		t.Fatalf("copies = %v", got)
	}
}
