package main

import (
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
)

func postBulk(t *testing.T, vals url.Values) *httptest.ResponseRecorder {
	t.Helper()
	r := httptest.NewRequest(http.MethodPost, "/bulk", strings.NewReader(vals.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleBulk(w, r)
	return w
}

func bulkSetup(t *testing.T) string {
	t.Helper()
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com/1", Title: "a1", HTMLFile: "0001-a1.html"},
		{ID: 2, URL: "https://b.com/2", Title: "b2", HTMLFile: "0002-b2.html"},
		{ID: 3, URL: "https://c.com/3", Title: "c3", HTMLFile: "0003-c3.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a1.html", "https://a.com/1", "a1")
	writeHTMLFile(t, base, "0002-b2.html", "https://b.com/2", "b2")
	writeHTMLFile(t, base, "0003-c3.html", "https://c.com/3", "c3")
	return base
}

func TestWebAddCustomTitle(t *testing.T) {
	entries := []*Bookmark{}
	_, base := setupReindexTest(t, entries)
	_ = base
	vals := url.Values{
		"url":   {"https://example.invalid/some-page"},
		"title": {"My Custom Title"},
	}
	r := httptest.NewRequest(http.MethodPost, "/add", strings.NewReader(vals.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleAdd(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d, body:\n%s", w.Code, w.Body.String())
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		t.Fatal(err)
	}
	_ = cfg
	if len(store.Bookmarks) != 1 || store.Bookmarks[0].Title != "My Custom Title" {
		t.Fatalf("bookmarks = %+v", store.Bookmarks)
	}
}

func TestWebBulkDeleteFlow(t *testing.T) {
	base := bulkSetup(t)
	w := postBulk(t, url.Values{"action": {"delete"}, "ids": {"1", "3"}})
	if w.Code != http.StatusOK {
		t.Fatalf("confirm code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "Yes, delete all") || !strings.Contains(body, "a1") || !strings.Contains(body, "c3") {
		t.Fatalf("confirm page wrong:\n%s", body[:800])
	}
	w = postBulk(t, url.Values{"action": {"delete"}, "ids": {"1", "3"}, "confirm": {"1"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("delete code = %d", w.Code)
	}
	s := loadTestStore(t, base)
	if len(s.Bookmarks) != 1 || s.Find(2) == nil {
		t.Fatalf("bookmarks = %+v", s.Bookmarks)
	}
}

func TestWebBulkTagsFolder(t *testing.T) {
	base := bulkSetup(t)
	w := postBulk(t, url.Values{"action": {"tags"}, "ids": {"1", "2"}, "bulk_tags": {"x y"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("retag code = %d", w.Code)
	}
	s := loadTestStore(t, base)
	if len(s.Find(1).Tags) != 2 || len(s.Find(2).Tags) != 2 || len(s.Find(3).Tags) != 0 {
		t.Fatalf("tags wrong: %+v", s.Bookmarks)
	}
	w = postBulk(t, url.Values{"action": {"folder"}, "ids": {"2", "3"}, "bulk_folder": {"proj"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("move code = %d", w.Code)
	}
	s = loadTestStore(t, base)
	if s.Find(2).Folder != "proj" || s.Find(3).Folder != "proj" || s.Find(1).Folder != "" {
		t.Fatalf("folders wrong: %+v", s.Bookmarks)
	}
}

func TestWebBulkNoSelection(t *testing.T) {
	base := bulkSetup(t)
	_ = base
	w := postBulk(t, url.Values{"action": {"delete"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d", w.Code)
	}
	if loc := w.Header().Get("Location"); !strings.Contains(loc, "msg=") {
		t.Fatalf("location = %q", loc)
	}
	w = postBulk(t, url.Values{"action": {"delete"}, "ids": {"99", "abc"}})
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d", w.Code)
	}
}

func TestWebDetachByName(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		t.Fatal(err)
	}
	b := store.Find(1)
	if err := attachReader(cfg, b, "doc.pdf", strings.NewReader("x")); err != nil {
		t.Fatal(err)
	}
	if err := store.Save(); err != nil {
		t.Fatal(err)
	}
	vals := url.Values{
		"title": {"a"}, "url": {"https://a.com"}, "delattname": {"doc.pdf"},
	}
	r := httptest.NewRequest(http.MethodPost, "/edit/1", strings.NewReader(vals.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleEdit(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d, body:\n%s", w.Code, w.Body.String())
	}
	s := loadTestStore(t, base)
	if len(s.Find(1).Attachments) != 0 {
		t.Fatalf("attachments = %+v", s.Find(1).Attachments)
	}
}

func TestWebEditNotes(t *testing.T) {
	base := apiContentSetup(t)
	_ = base
	r := httptest.NewRequest(http.MethodGet, "/edit/1", nil)
	w := httptest.NewRecorder()
	handleEdit(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), `name="markdown_body"`) || !strings.Contains(w.Body.String(), "notes here") {
		t.Fatalf("notes textarea missing:\n%s", w.Body.String()[:1000])
	}

	vals := url.Values{
		"title": {"alpha"}, "url": {"https://a.com/1"}, "markdown_body": {"# edited\n\nweb notes\n"},
	}
	r = httptest.NewRequest(http.MethodPost, "/edit/1", strings.NewReader(vals.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w = httptest.NewRecorder()
	handleEdit(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d, body:\n%s", w.Code, w.Body.String())
	}
	cfg, _, err := loadCfgAndStore()
	if err != nil {
		t.Fatal(err)
	}
	body, err := readMarkdownBody(cfg, loadTestStore(t, base).Find(1))
	if err != nil || !strings.Contains(body, "web notes") {
		t.Fatalf("body = %q, err = %v", body, err)
	}
}

func learnSetup(t *testing.T) string {
	t.Helper()
	entries := []*Bookmark{
		{ID: 1, URL: "https://h.com/1", Title: "h1", Folder: "docs", HTMLFile: "0001-h1.html"},
		{ID: 2, URL: "https://h.com/2", Title: "h2", Folder: "docs", HTMLFile: "0002-h2.html"},
		{ID: 3, URL: "https://o.com/3", Title: "o3", HTMLFile: "0003-o3.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-h1.html", "https://h.com/1", "h1")
	writeHTMLFile(t, base, "0002-h2.html", "https://h.com/2", "h2")
	writeHTMLFile(t, base, "0003-o3.html", "https://o.com/3", "o3")
	return base
}

func TestWebLearnMinAndCreateAll(t *testing.T) {
	base := learnSetup(t)
	_ = base
	r := httptest.NewRequest(http.MethodGet, "/tags", nil)
	w := httptest.NewRecorder()
	handleTags(w, r)
	if strings.Contains(w.Body.String(), "h.com") {
		t.Fatalf("default min=3 should not suggest h.com")
	}
	r = httptest.NewRequest(http.MethodGet, "/tags?min=2", nil)
	w = httptest.NewRecorder()
	handleTags(w, r)
	body := w.Body.String()
	if !strings.Contains(body, "h.com") || !strings.Contains(body, `value="2"`) {
		t.Fatalf("min=2 should suggest h.com:\n%s", body[:800])
	}
	r = httptest.NewRequest(http.MethodPost, "/tags/rule/learn-all", strings.NewReader(url.Values{"min": {"2"}}.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w = httptest.NewRecorder()
	handleTaxonomy(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d", w.Code)
	}
	if loc := w.Header().Get("Location"); !strings.Contains(loc, "Created") {
		t.Fatalf("location = %q", loc)
	}
	_, store, err := loadCfgAndStore()
	if err != nil {
		t.Fatal(err)
	}
	if len(store.AutoRules) != 1 || store.AutoRules[0].Match != "host:h.com" {
		t.Fatalf("rules = %+v", store.AutoRules)
	}
}

func TestWebPerRuleApply(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://z.com/1", Title: "z1", HTMLFile: "0001-z1.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-z1.html", "https://z.com/1", "z1")
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		t.Fatal(err)
	}
	rule := &AutoRule{Match: "host:z.com", Folder: "zed"}
	store.AddAutoRule(rule)
	if err := store.Save(); err != nil {
		t.Fatal(err)
	}
	_ = cfg
	_ = base
	r := httptest.NewRequest(http.MethodPost, "/settings/auto/apply", strings.NewReader(url.Values{"id": {"1"}}.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleSettingsAuto(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d", w.Code)
	}
	if loc := w.Header().Get("Location"); !strings.Contains(loc, "Applied+to+1") {
		t.Fatalf("location = %q", loc)
	}
	s := loadTestStore(t, base)
	if s.Find(1).Folder != "zed" {
		t.Fatalf("rule not applied: %+v", s.Find(1))
	}
	r = httptest.NewRequest(http.MethodGet, "/settings", nil)
	w = httptest.NewRecorder()
	handleSettings(w, r)
	if !strings.Contains(w.Body.String(), `name="id" value="1"`) {
		t.Fatalf("settings page missing per-rule apply button")
	}
}
