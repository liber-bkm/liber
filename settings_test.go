package main

import (
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
)

func postSettingsReindex(t *testing.T, vals url.Values) *httptest.ResponseRecorder {
	t.Helper()
	r := httptest.NewRequest(http.MethodPost, "/settings/reindex", strings.NewReader(vals.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleSettingsReindex(w, r)
	return w
}

func TestSettingsReindexPost(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	w := postSettingsReindex(t, url.Values{"merge": {"on"}})
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "Reindex done") || !strings.Contains(body, "Reindexed: 1 bookmark(s) remain.") {
		t.Fatalf("body missing reindex output:\n%s", body)
	}
	if !strings.Contains(body, "Maintenance") {
		t.Fatalf("body missing maintenance section")
	}
}

func TestSettingsReindexAllWithoutMerge(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	w := postSettingsReindex(t, url.Values{"all": {"on"}})
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "--all requires --merge") {
		t.Fatalf("body missing validation error:\n%s", w.Body.String())
	}
}

func TestSettingsPageShowsMaintenance(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	r := httptest.NewRequest(http.MethodGet, "/settings", nil)
	w := httptest.NewRecorder()
	handleSettings(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "Maintenance") || !strings.Contains(body, "/settings/reindex") {
		t.Fatalf("settings page missing maintenance form")
	}
	if !strings.Contains(body, "index matches disk") {
		t.Fatalf("status line missing:\n%s", body)
	}
}
