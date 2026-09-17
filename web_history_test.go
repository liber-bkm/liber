package main

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestWebOpenTracksHistory(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	r := httptest.NewRequest(http.MethodGet, "/open/1", nil)
	w := httptest.NewRecorder()
	handleOpen(w, r)
	if w.Code != http.StatusFound {
		t.Fatalf("code = %d, want 302", w.Code)
	}
	if loc := w.Header().Get("Location"); loc != "https://a.com" {
		t.Fatalf("location = %q", loc)
	}
	s := loadTestStore(t, base)
	b := s.Find(1)
	if b == nil || b.LastOpenedAt == nil || b.OpenCount != 1 {
		t.Fatalf("history not recorded: %+v", b)
	}
}

func TestWebOpenMissing(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	r := httptest.NewRequest(http.MethodGet, "/open/99", nil)
	w := httptest.NewRecorder()
	handleOpen(w, r)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func TestWebHistoryPage(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
		{ID: 2, URL: "https://b.com", Title: "b", HTMLFile: "0002-b.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	writeHTMLFile(t, base, "0002-b.html", "https://b.com", "b")
	r := httptest.NewRequest(http.MethodGet, "/open/2", nil)
	handleOpen(httptest.NewRecorder(), r)
	r = httptest.NewRequest(http.MethodGet, "/history", nil)
	w := httptest.NewRecorder()
	handleHistory(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "History") || !strings.Contains(body, ">b<") {
		t.Fatalf("history page missing opened bookmark:\n%s", body[:500])
	}
	if strings.Contains(body, ">a<") {
		t.Fatalf("history page should not list unopened bookmark")
	}
}

func TestWebPick(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "alpha", HTMLFile: "0001-a.html"},
		{ID: 2, URL: "https://b.com", Title: "beta", HTMLFile: "0002-b.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "alpha")
	writeHTMLFile(t, base, "0002-b.html", "https://b.com", "beta")
	r := httptest.NewRequest(http.MethodGet, "/pick?q=alpha", nil)
	w := httptest.NewRecorder()
	handlePick(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if strings.TrimSpace(w.Body.String()) != "https://a.com" {
		t.Fatalf("body = %q", w.Body.String())
	}
	r = httptest.NewRequest(http.MethodGet, "/pick?q=nomatchxyz", nil)
	w = httptest.NewRecorder()
	handlePick(w, r)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	r = httptest.NewRequest(http.MethodGet, "/pick", nil)
	w = httptest.NewRecorder()
	handlePick(w, r)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
}
