package main

import (
	"bytes"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"net/url"
	"path/filepath"
	"strings"
	"testing"
)

const libraryExportSample = `<!DOCTYPE NETSCAPE-Bookmark-file-1>
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
	w, r := postMultipart(t, "/settings/import", "bookmark_file", "bookmarks.html", libraryExportSample, nil)
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
