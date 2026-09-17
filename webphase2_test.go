package main

import (
	"bytes"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

const phase2ExportSample = `<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<DL><p>
<DT><H3>Work</H3>
<DL><p>
<DT><A HREF="https://x.com/a" TAGS="t1">Title A</A>
<DD>desc a
</DL>
<DT><A HREF="https://y.com/b">Title B</A>
</DL>
`

func postMultipart(t *testing.T, path, field, filename, content string, vals url.Values) (*httptest.ResponseRecorder, *http.Request) {
	t.Helper()
	var buf bytes.Buffer
	mw := multipart.NewWriter(&buf)
	fw, err := mw.CreateFormFile(field, filename)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := fw.Write([]byte(content)); err != nil {
		t.Fatal(err)
	}
	for k, vs := range vals {
		for _, v := range vs {
			if err := mw.WriteField(k, v); err != nil {
				t.Fatal(err)
			}
		}
	}
	if err := mw.Close(); err != nil {
		t.Fatal(err)
	}
	r := httptest.NewRequest(http.MethodPost, path, &buf)
	r.Header.Set("Content-Type", mw.FormDataContentType())
	return httptest.NewRecorder(), r
}

func TestSettingsImportUpload(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	w, r := postMultipart(t, "/settings/import", "bookmark_file", "bookmarks.html", phase2ExportSample, nil)
	handleSettingsImport(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "Import done") || !strings.Contains(body, "Imported 2 bookmark(s)") {
		t.Fatalf("body missing import report:\n%s", body)
	}
	s := loadTestStore(t, base)
	if len(s.Bookmarks) != 3 {
		t.Fatalf("bookmarks = %d, want 3", len(s.Bookmarks))
	}
	if findDuplicate(s, "https://x.com/a") == nil {
		t.Fatalf("imported bookmark missing")
	}
}

func TestSettingsImportNoFile(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	r := httptest.NewRequest(http.MethodPost, "/settings/import", strings.NewReader(""))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleSettingsImport(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "Import failed") {
		t.Fatalf("body missing failure:\n%s", w.Body.String())
	}
}

func TestSettingsExportDefault(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	r := httptest.NewRequest(http.MethodPost, "/settings/export", strings.NewReader(""))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleSettingsExport(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "Export done") {
		t.Fatalf("body missing export report:\n%s", w.Body.String())
	}
	if !fileExists(filepath.Join(base, "site", "index.html")) {
		t.Fatalf("site/index.html not written")
	}
}

func TestSettingsSyncNoRepo(t *testing.T) {
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	r := httptest.NewRequest(http.MethodPost, "/settings/sync", strings.NewReader(""))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleSettingsSync(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "doesn&#39;t look like") && !strings.Contains(w.Body.String(), "doesn't look like") {
		t.Fatalf("body missing no-repo message:\n%s", w.Body.String())
	}
}

func TestSettingsSyncGitRepo(t *testing.T) {
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git not available")
	}
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	for _, args := range [][]string{{"init"}, {"config", "user.email", "t@t.t"}, {"config", "user.name", "t"}, {"add", "-A"}, {"commit", "-m", "init"}} {
		cmd := exec.Command("git", args...)
		cmd.Dir = base
		if out, err := cmd.CombinedOutput(); err != nil {
			t.Fatalf("git %v: %v\n%s", args, err, out)
		}
	}
	r := httptest.NewRequest(http.MethodPost, "/settings/sync", strings.NewReader(""))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	handleSettingsSync(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "Sync done") {
		t.Fatalf("body missing sync report:\n%s", w.Body.String())
	}
}

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
