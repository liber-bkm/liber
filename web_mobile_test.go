package main

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestWebViewportMeta(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	r := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()
	handleSearch(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, `name="viewport" content="width=device-width, initial-scale=1"`) {
		t.Fatalf("viewport meta missing")
	}
}

func TestWebMobileBreakpoints(t *testing.T) {
	for _, want := range []string{
		"@media (max-width:",
		".setform { grid-template-columns: 1fr; }",
		"position: sticky",
		"overflow-wrap: anywhere",
	} {
		if !strings.Contains(pageCSS, want) {
			t.Errorf("pageCSS missing %q", want)
		}
	}
}
