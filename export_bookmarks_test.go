package main

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func exportFixture(t *testing.T) (Config, string) {
	t.Helper()
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com/1", Title: "A One", Description: "first desc", Tags: []string{"x", "y"}, HTMLFile: "0001-a-one.html"},
		{ID: 2, URL: "https://b.com/2?x=1&y=2", Title: "B <Two>", Description: "line one\nline two", Tags: []string{"z"}, Folder: "work", HTMLFile: "work/0002-b-two.html"},
		{ID: 3, URL: "https://c.com/3", Title: "C Three", Folder: "work/deep", HTMLFile: "work/deep/0003-c-three.html"},
	}
	cfg, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a-one.html", entries[0].URL, "A One")
	writeHTMLFile(t, base, "work/0002-b-two.html", entries[1].URL, "B Two")
	writeHTMLFile(t, base, "work/deep/0003-c-three.html", entries[2].URL, "C Three")
	return cfg, base
}

func TestNetscapeRoundTrip(t *testing.T) {
	cfg, base := exportFixture(t)
	store := loadTestStore(t, base)

	var sb strings.Builder
	if err := writeNetscapeExport(&sb, store); err != nil {
		t.Fatal(err)
	}
	entries := parseNetscapeBookmarks(sb.String())
	if len(entries) != 3 {
		t.Fatalf("parsed %d entries, want 3:\n%s", len(entries), sb.String())
	}
	byURL := map[string]importedEntry{}
	for _, e := range entries {
		byURL[e.href] = e
	}
	a := byURL["https://a.com/1"]
	if a.title != "A One" || a.desc != "first desc" || a.tagsRaw != "x,y" || a.folder != "" {
		t.Fatalf("entry A = %+v", a)
	}
	b := byURL["https://b.com/2?x=1&y=2"]
	if b.title != "B <Two>" || b.desc != "line one line two" || b.tagsRaw != "z" || b.folder != "work" {
		t.Fatalf("entry B = %+v", b)
	}
	c := byURL["https://c.com/3"]
	if c.title != "C Three" || c.folder != "work/deep" {
		t.Fatalf("entry C = %+v", c)
	}
	_ = cfg
}

func TestNetscapeReimport(t *testing.T) {
	_, base := exportFixture(t)
	store := loadTestStore(t, base)

	var sb strings.Builder
	if err := writeNetscapeExport(&sb, store); err != nil {
		t.Fatal(err)
	}
	cfg, _, err := loadCfgAndStore()
	if err != nil {
		t.Fatal(err)
	}
	fresh := &Store{NextID: 1, NextAutoRuleID: 1}
	added, dup, bad, warnings := importData(cfg, fresh, []byte(sb.String()), importOptions{})
	for _, w := range warnings {
		t.Log(w)
	}
	if len(added) != 3 || dup != 0 || bad != 0 {
		t.Fatalf("added=%d dup=%d bad=%d", len(added), dup, bad)
	}
	urls := map[string]bool{}
	for _, b := range added {
		urls[b.URL] = true
	}
	for _, want := range []string{"https://a.com/1", "https://b.com/2?x=1&y=2", "https://c.com/3"} {
		if !urls[want] {
			t.Fatalf("missing %s", want)
		}
	}
}

func TestExportBookmarksHandler(t *testing.T) {
	_, base := exportFixture(t)
	_ = base
	r := httptest.NewRequest(http.MethodGet, "/export-bookmarks", nil)
	w := httptest.NewRecorder()
	handleExportBookmarks(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if ct := w.Header().Get("Content-Type"); ct != "text/html; charset=utf-8" {
		t.Fatalf("content-type = %q", ct)
	}
	if cd := w.Header().Get("Content-Disposition"); !strings.Contains(cd, "liber-bookmarks.html") {
		t.Fatalf("disposition = %q", cd)
	}
	entries := parseNetscapeBookmarks(w.Body.String())
	if len(entries) != 3 {
		t.Fatalf("parsed %d entries, want 3", len(entries))
	}
	r = httptest.NewRequest(http.MethodPost, "/export-bookmarks", nil)
	w = httptest.NewRecorder()
	handleExportBookmarks(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("POST code = %d, want redirect", w.Code)
	}
}
