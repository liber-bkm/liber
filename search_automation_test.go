package main

import (
	"testing"
	"time"
)

func searchStore() *Store {
	s := &Store{NextID: 1}
	s.Bookmarks = []*Bookmark{
		{ID: 1, URL: "https://example.com/alpha", Title: "Alpha guide", Tags: []string{"ref"}, Folder: "docs"},
		{ID: 2, URL: "https://example.com/beta", Title: "Beta notes", Tags: []string{"todo"}, Folder: "work", Description: "alpha mention"},
		{ID: 3, URL: "https://other.com/alpha", Title: "Other page", Folder: "docs"},
	}
	s.NextID = 4
	return s
}

func idsOf(list []*Bookmark) []int {
	out := make([]int, len(list))
	for i, b := range list {
		out[i] = b.ID
	}
	return out
}

func equalInts(a, b []int) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func TestSearchMatches(t *testing.T) {
	s := searchStore()
	var cfg Config
	if got := idsOf(s.Search(cfg, "alpha", SearchFields{}, false, SortRelevance)); !equalInts(got, []int{1, 2, 3}) {
		t.Errorf("full search = %v", got)
	}
	if got := idsOf(s.Search(cfg, "alpha", SearchFields{Title: true}, false, SortRelevance)); !equalInts(got, []int{1}) {
		t.Errorf("title search = %v", got)
	}
	if got := idsOf(s.Search(cfg, "todo", SearchFields{}, false, SortRelevance)); !equalInts(got, []int{2}) {
		t.Errorf("tag search = %v", got)
	}
	if got := idsOf(s.Search(cfg, "", SearchFields{}, false, SortRelevance)); !equalInts(got, []int{1, 2, 3}) {
		t.Errorf("empty search = %v", got)
	}
}

func TestSearchRelevance(t *testing.T) {
	s := &Store{NextID: 1}
	s.Bookmarks = []*Bookmark{
		{ID: 1, URL: "https://example.com/guide", Title: "Something else", Tags: []string{"alpha"}},
		{ID: 2, URL: "https://example.com/other", Title: "Alpha manual"},
		{ID: 3, URL: "https://alpha.example.com/x", Title: "Unrelated"},
	}
	var cfg Config
	if got := idsOf(s.Search(cfg, "alpha", SearchFields{}, false, SortRelevance)); !equalInts(got, []int{2, 1, 3}) {
		t.Errorf("relevance = %v, want prefix-title first", got)
	}
}

func TestSearchSortModes(t *testing.T) {
	s := &Store{NextID: 1}
	old := time.Now().Add(-48 * time.Hour)
	mid := time.Now().Add(-24 * time.Hour)
	now := time.Now()
	s.Bookmarks = []*Bookmark{
		{ID: 1, URL: "https://x/1", Title: "Charlie", CreatedAt: mid, LastOpenedAt: &old},
		{ID: 2, URL: "https://x/2", Title: "Alpha", CreatedAt: now},
		{ID: 3, URL: "https://x/3", Title: "Bravo", CreatedAt: old, LastOpenedAt: &now},
	}
	var cfg Config
	if got := idsOf(s.Search(cfg, "", SearchFields{}, false, SortNewest)); !equalInts(got, []int{2, 1, 3}) {
		t.Errorf("newest = %v", got)
	}
	if got := idsOf(s.Search(cfg, "", SearchFields{}, false, SortOldest)); !equalInts(got, []int{3, 1, 2}) {
		t.Errorf("oldest = %v", got)
	}
	if got := idsOf(s.Search(cfg, "", SearchFields{}, false, SortVisited)); !equalInts(got, []int{3, 1, 2}) {
		t.Errorf("visited = %v", got)
	}
	if got := idsOf(s.Search(cfg, "", SearchFields{}, false, SortTitle)); !equalInts(got, []int{2, 3, 1}) {
		t.Errorf("title = %v", got)
	}
}

func TestParseSortMode(t *testing.T) {
	if m, err := ParseSortMode(""); err != nil || m != SortRelevance {
		t.Errorf("empty sort = %q %v", m, err)
	}
	if m, err := ParseSortMode("visited"); err != nil || m != SortVisited {
		t.Errorf("visited sort = %q %v", m, err)
	}
	if _, err := ParseSortMode("nope"); err == nil {
		t.Error("bad sort should fail")
	}
}

func TestSuggestRules(t *testing.T) {
	s := &Store{NextID: 1}
	add := func(url, folder string) {
		s.Bookmarks = append(s.Bookmarks, &Bookmark{ID: s.NextID, URL: url, Folder: folder})
		s.NextID++
	}
	add("https://a.com/1", "code")
	add("https://a.com/2", "code")
	add("https://a.com/3", "code")
	add("https://b.com/1", "misc")
	add("https://b.com/2", "misc")
	add("https://c.com/1", "solo")
	add("https://d.com/1", "")

	got := suggestRules(s, 3)
	if len(got) != 1 || got[0].host != "a.com" || got[0].folder != "code" || got[0].count != 3 {
		t.Fatalf("suggestRules(3) = %+v", got)
	}
	got = suggestRules(s, 2)
	if len(got) != 2 {
		t.Fatalf("suggestRules(2) = %+v", got)
	}
	s.AutoRules = []*AutoRule{{ID: 1, Match: "host:a.com", Folder: "code"}}
	if got := suggestRules(s, 2); len(got) != 1 || got[0].host != "b.com" {
		t.Fatalf("covered host not skipped: %+v", got)
	}
}

func TestSuggestRulesTieBreak(t *testing.T) {
	mk := func() *Store {
		s := &Store{NextID: 1}
		for i, f := range []string{"misc", "code", "misc", "code"} {
			s.Bookmarks = append(s.Bookmarks, &Bookmark{ID: i + 1, URL: "https://t.com/x", Folder: f})
		}
		return s
	}
	first := suggestRules(mk(), 2)
	for i := 0; i < 5; i++ {
		got := suggestRules(mk(), 2)
		if len(got) != 1 || got[0].folder != first[0].folder {
			t.Fatalf("tie-break unstable: %+v vs %+v", first, got)
		}
	}
	if first[0].folder != "code" {
		t.Fatalf("tie-break = %q, want code", first[0].folder)
	}
}
