package main

import (
	"net/http"
	"net/http/httptest"
	"net/url"
	"strconv"
	"strings"
	"testing"
)

func checkWebSetup(t *testing.T, srv *httptest.Server) string {
	t.Helper()
	entries := []*Bookmark{
		{ID: 1, URL: srv.URL + "/ok", Title: "okb", HTMLFile: "0001-okb.html"},
		{ID: 2, URL: srv.URL + "/gone", Title: "goneb", HTMLFile: "0002-goneb.html"},
		{ID: 3, URL: srv.URL + "/moved", Title: "movedb", HTMLFile: "0003-movedb.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-okb.html", entries[0].URL, "okb")
	writeHTMLFile(t, base, "0002-goneb.html", entries[1].URL, "goneb")
	writeHTMLFile(t, base, "0003-movedb.html", entries[2].URL, "movedb")
	return base
}

func checkApplyVals(rows []checkRowView, op string) url.Values {
	v := url.Values{"op": {op}, "rok": {"1"}, "rchecked": {"3"}, "rfresh": {"0"}, "rmissing": {""}}
	for _, row := range rows {
		v.Add("rid", strconv.Itoa(row.ID))
		v.Add("rtitle", row.Title)
		v.Add("rurl", row.URL)
		v.Add("rstatus", row.Status)
		v.Add("rdetail", row.Detail)
		v.Add("rtarget", row.Target)
	}
	return v
}

func postCheck(t *testing.T, path string, vals url.Values) *httptest.ResponseRecorder {
	t.Helper()
	r := httptest.NewRequest(http.MethodPost, path, strings.NewReader(vals.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w := httptest.NewRecorder()
	switch path {
	case "/check/run":
		handleCheckRun(w, r)
	case "/check/apply":
		handleCheckApply(w, r)
	default:
		t.Fatalf("unknown path %s", path)
	}
	return w
}

func TestWebCheckForm(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	_ = base
	r := httptest.NewRequest(http.MethodGet, "/check", nil)
	w := httptest.NewRecorder()
	handleCheck(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "Run check") || !strings.Contains(body, `value="12"`) {
		t.Fatalf("form missing:\n%s", body[:500])
	}
}

func TestWebCheckRunBuckets(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	_ = base
	w := postCheck(t, "/check/run", url.Values{"workers": {"2"}})
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "1 ok, 1 moved, 1 dead, 0 uncertain (of 3 checked)") {
		t.Fatalf("summary wrong:\n%s", body[:1000])
	}
	if !strings.Contains(body, "goneb") || !strings.Contains(body, "movedb") {
		t.Fatalf("flagged rows missing")
	}
	if !strings.Contains(body, `value="update:3"`) || !strings.Contains(body, `value="delete:2"`) {
		t.Fatalf("action buttons missing")
	}
	s := loadTestStore(t, base)
	for _, b := range s.Bookmarks {
		if b.LastCheckStatus == "" {
			t.Fatalf("[%d] missing check stamp", b.ID)
		}
	}
}

func TestWebCheckApplyDelete(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	rows := []checkRowView{
		{ID: 2, Title: "goneb", URL: srv.URL + "/gone", Detail: "404", Status: "dead"},
		{ID: 3, Title: "movedb", URL: srv.URL + "/moved", Detail: "301", Target: srv.URL + "/ok", Status: "moved"},
	}
	w := postCheck(t, "/check/apply", checkApplyVals(rows, "delete:2"))
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "Deleted [2]") || strings.Contains(body, "goneb</div>") && strings.Contains(body, `value="delete:2"`) {
		t.Fatalf("row not dropped:\n%s", body[:1000])
	}
	s := loadTestStore(t, base)
	if s.Find(2) != nil {
		t.Fatalf("bookmark not deleted")
	}
	if fileExists(base + "/html/0002-goneb.html") {
		t.Fatalf("deleted files not removed")
	}
}

func TestWebCheckApplyUpdate(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	rows := []checkRowView{
		{ID: 3, Title: "movedb", URL: srv.URL + "/moved", Detail: "301", Target: srv.URL + "/ok", Status: "moved"},
	}
	w := postCheck(t, "/check/apply", checkApplyVals(rows, "update:3"))
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "Updated [3]") {
		t.Fatalf("flash missing:\n%s", w.Body.String()[:500])
	}
	s := loadTestStore(t, base)
	if got := s.Find(3).URL; got != srv.URL+"/ok" {
		t.Fatalf("url = %q", got)
	}
}

func TestWebCheckApplyRetitleUnchanged(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	_ = base
	rows := []checkRowView{
		{ID: 3, Title: "movedb", URL: srv.URL + "/moved", Detail: "301", Target: srv.URL + "/ok", Status: "moved"},
	}
	w := postCheck(t, "/check/apply", checkApplyVals(rows, "retitle:3"))
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "title unchanged") {
		t.Fatalf("flash missing:\n%s", w.Body.String()[:500])
	}
}

func TestWebCheckApplyQuarantine(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	rows := []checkRowView{
		{ID: 2, Title: "goneb", URL: srv.URL + "/gone", Detail: "404", Status: "dead"},
	}
	w := postCheck(t, "/check/apply", checkApplyVals(rows, "quarantine:2"))
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	s := loadTestStore(t, base)
	if s.Find(2).Folder != "quarantine" {
		t.Fatalf("not quarantined: %+v", s.Find(2))
	}
}

func TestWebCheckRunValidation(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	_ = base
	w := postCheck(t, "/check/run", url.Values{"workers": {"0"}})
	if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "workers") {
		t.Fatalf("workers error missing:\n%s", w.Body.String()[:500])
	}
	w = postCheck(t, "/check/run", url.Values{"spec": {"abc"}})
	if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "flash error") {
		t.Fatalf("spec error missing:\n%s", w.Body.String()[:500])
	}
}

func TestWebCheckApplyGoneAndUnknown(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	_ = base
	rows := []checkRowView{
		{ID: 99, Title: "ghost", URL: srv.URL + "/gone", Detail: "404", Status: "dead"},
	}
	w := postCheck(t, "/check/apply", checkApplyVals(rows, "delete:99"))
	if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "already gone") {
		t.Fatalf("gone flash missing:\n%s", w.Body.String()[:500])
	}
	rows = []checkRowView{
		{ID: 2, Title: "goneb", URL: srv.URL + "/gone", Detail: "404", Status: "dead"},
	}
	w = postCheck(t, "/check/apply", checkApplyVals(rows, "bogus:2"))
	if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "Unknown action") {
		t.Fatalf("unknown action flash missing:\n%s", w.Body.String()[:500])
	}
	s := loadTestStore(t, base)
	if s.Find(2) == nil {
		t.Fatalf("unknown action should not delete")
	}
}
