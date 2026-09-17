package main

import (
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
)

func TestProfilesWebFlow(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")

	post := func(path string, vals url.Values) *httptest.ResponseRecorder {
		r := httptest.NewRequest(http.MethodPost, path, strings.NewReader(vals.Encode()))
		r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		w := httptest.NewRecorder()
		switch path {
		case "/profiles/switch":
			handleProfileSwitch(w, r)
		case "/profiles/delete":
			handleProfileDelete(w, r)
		}
		return w
	}

	w := post("/profiles/switch", url.Values{"name": {"work"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("switch code = %d", w.Code)
	}
	cfg, _, err := LoadConfig()
	if err != nil {
		t.Fatal(err)
	}
	if cfg.ActiveProfile != "work" {
		t.Fatalf("active = %q", cfg.ActiveProfile)
	}

	r := httptest.NewRequest(http.MethodGet, "/profiles", nil)
	w = httptest.NewRecorder()
	handleProfiles(w, r)
	if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "work (active)") {
		t.Fatalf("profiles page missing active work:\n%s", w.Body.String()[:500])
	}

	w = post("/profiles/switch", url.Values{"name": {"default"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("switch back code = %d", w.Code)
	}
	w = post("/profiles/delete", url.Values{"name": {"work"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("delete code = %d", w.Code)
	}
	cfg, _, err = LoadConfig()
	if err != nil {
		t.Fatal(err)
	}
	if len(cfg.Profiles) != 0 {
		t.Fatalf("profiles = %v", cfg.Profiles)
	}

	w = post("/profiles/delete", url.Values{"name": {"missing"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("delete missing code = %d", w.Code)
	}
	if loc := w.Header().Get("Location"); !strings.Contains(loc, "Delete+failed") && !strings.Contains(loc, "Delete%20failed") {
		t.Fatalf("location missing error: %q", loc)
	}
}
