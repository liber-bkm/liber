package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

func apiTestSetup(t *testing.T) string {
	t.Helper()
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com/1", Title: "alpha one", Tags: []string{"x"}, Folder: "docs", HTMLFile: "docs/0001-alpha-one.html"},
		{ID: 2, URL: "https://b.com/2", Title: "beta two", HTMLFile: "0002-beta-two.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "docs/0001-alpha-one.html", entries[0].URL, "alpha one")
	writeHTMLFile(t, base, "0002-beta-two.html", entries[1].URL, "beta two")
	return base
}

func apiDo(t *testing.T, h http.Handler, method, target, body string) *httptest.ResponseRecorder {
	t.Helper()
	var r *http.Request
	if body == "" {
		r = httptest.NewRequest(method, target, nil)
	} else {
		r = httptest.NewRequest(method, target, strings.NewReader(body))
		r.Header.Set("Content-Type", "application/json")
	}
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	return w
}

func apiDecode(t *testing.T, w *httptest.ResponseRecorder) map[string]any {
	t.Helper()
	var out map[string]any
	if err := json.Unmarshal(w.Body.Bytes(), &out); err != nil {
		t.Fatalf("bad JSON: %v\n%s", err, w.Body.String())
	}
	return out
}

func TestAPIList(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/bookmarks", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if out["total"].(float64) != 2 || len(out["bookmarks"].([]any)) != 2 {
		t.Fatalf("out = %v", out)
	}
	first := out["bookmarks"].([]any)[0].(map[string]any)
	for _, k := range []string{"id", "url", "title", "created_at", "updated_at", "has_markdown", "has_archive"} {
		if _, ok := first[k]; !ok {
			t.Fatalf("missing key %q in %v", k, first)
		}
	}

	w = apiDo(t, h, "GET", "/api/v1/bookmarks?q=beta&scope=n", "")
	if apiDecode(t, w)["total"].(float64) != 1 {
		t.Fatalf("scoped search wrong: %s", w.Body.String())
	}

	w = apiDo(t, h, "GET", "/api/v1/bookmarks?sort=bogus", "")
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
}

func TestAPIGet(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/bookmarks/1", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if apiDecode(t, w)["title"] != "alpha one" {
		t.Fatalf("body = %s", w.Body.String())
	}

	w = apiDo(t, h, "GET", "/api/v1/bookmarks/99", "")
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "GET", "/api/v1/bookmarks/abc", "")
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func TestAPIAdd(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/bookmarks", `{"url":"https://c.com/3","title":"gamma","tags":["y"],"folder":"docs"}`)
	if w.Code != http.StatusCreated {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	out := apiDecode(t, w)
	if out["title"] != "gamma" || out["folder"] != "docs" {
		t.Fatalf("out = %v", out)
	}
	if loadTestStore(t, base).Find(3) == nil {
		t.Fatalf("not persisted")
	}

	w = apiDo(t, h, "POST", "/api/v1/bookmarks", `{"title":"no url"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/bookmarks", `not json`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
}

func TestAPIDuplicateFlow(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/bookmarks", `{"url":"https://a.com/1","title":"dup"}`)
	if w.Code != http.StatusConflict {
		t.Fatalf("code = %d, want 409", w.Code)
	}
	out := apiDecode(t, w)
	if out["error"] == nil || out["duplicate"] == nil || out["hint"] == nil {
		t.Fatalf("out = %v", out)
	}

	w = apiDo(t, h, "POST", "/api/v1/bookmarks", `{"url":"https://a.com/1","title":"dup","confirm_dup":true}`)
	if w.Code != http.StatusCreated {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
}

func TestAPIEdit(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "PUT", "/api/v1/bookmarks/1", `{"tags":["new"],"folder":"moved"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	b := loadTestStore(t, base).Find(1)
	if len(b.Tags) != 1 || b.Tags[0] != "new" || b.Folder != "moved" {
		t.Fatalf("bookmark = %+v", b)
	}
	if b.Title != "alpha one" || b.URL != "https://a.com/1" {
		t.Fatalf("untouched fields changed: %+v", b)
	}

	w = apiDo(t, h, "PUT", "/api/v1/bookmarks/1", `{"title":""}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "PUT", "/api/v1/bookmarks/99", `{"title":"x"}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func TestAPIDelete(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "DELETE", "/api/v1/bookmarks/1", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if out["confirm_required"] != true || out["bookmark"] == nil {
		t.Fatalf("out = %v", out)
	}
	if loadTestStore(t, base).Find(1) == nil {
		t.Fatalf("deleted without confirm")
	}

	w = apiDo(t, h, "DELETE", "/api/v1/bookmarks/1?confirm=true", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if apiDecode(t, w)["deleted"].(float64) != 1 {
		t.Fatalf("out = %s", w.Body.String())
	}
	if loadTestStore(t, base).Find(1) != nil {
		t.Fatalf("not deleted")
	}

	w = apiDo(t, h, "DELETE", "/api/v1/bookmarks/99?confirm=true", "")
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func TestAPITagsList(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/tags", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	tags := out["tags"].([]any)
	if len(tags) != 1 {
		t.Fatalf("tags = %v", tags)
	}
	first := tags[0].(map[string]any)
	if first["name"] != "x" || first["count"].(float64) != 1 {
		t.Fatalf("first = %v", first)
	}

	w = apiDo(t, h, "POST", "/api/v1/tags", "")
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func TestAPITagsRename(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/tags/rename", `{"old":"x","new":"y"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if apiDecode(t, w)["renamed"].(float64) != 1 {
		t.Fatalf("out = %s", w.Body.String())
	}
	b := loadTestStore(t, base).Find(1)
	if len(b.Tags) != 1 || b.Tags[0] != "y" {
		t.Fatalf("tags = %v", b.Tags)
	}

	w = apiDo(t, h, "POST", "/api/v1/tags/rename", `{"old":"missing","new":"y"}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/tags/rename", `{"old":"y","new":"y"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/tags/rename", `not json`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
}

func TestAPITagsDelete(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/tags/delete", `{"tag":"x"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if out["confirm_required"] != true || out["count"].(float64) != 1 {
		t.Fatalf("out = %v", out)
	}
	if len(loadTestStore(t, base).Find(1).Tags) != 1 {
		t.Fatalf("deleted without confirm")
	}

	w = apiDo(t, h, "POST", "/api/v1/tags/delete", `{"tag":"x","confirm":true}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if apiDecode(t, w)["deleted"].(float64) != 1 {
		t.Fatalf("out = %s", w.Body.String())
	}
	if len(loadTestStore(t, base).Find(1).Tags) != 0 {
		t.Fatalf("not deleted")
	}

	w = apiDo(t, h, "POST", "/api/v1/tags/delete", `{"tag":"missing","confirm":true}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/tags/delete", `{"tag":""}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/tags/bogus", `{"tag":"x"}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func TestAPIFoldersList(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/folders", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	folders := out["folders"].([]any)
	if len(folders) != 2 {
		t.Fatalf("folders = %v", folders)
	}
	seen := map[string]float64{}
	displays := map[string]string{}
	for _, f := range folders {
		m := f.(map[string]any)
		seen[m["name"].(string)] = m["count"].(float64)
		displays[m["name"].(string)] = m["display"].(string)
	}
	if seen["docs"] != 1 || seen[""] != 1 {
		t.Fatalf("seen = %v", seen)
	}
	if displays[""] != "/" || displays["docs"] != "docs" {
		t.Fatalf("displays = %v", displays)
	}

	w = apiDo(t, h, "POST", "/api/v1/folders", "")
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func TestAPIFoldersRename(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/folders/rename", `{"old":"docs","new":"work"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if apiDecode(t, w)["moved"].(float64) != 1 {
		t.Fatalf("out = %s", w.Body.String())
	}
	if b := loadTestStore(t, base).Find(1); b.Folder != "work" {
		t.Fatalf("folder = %q", b.Folder)
	}

	w = apiDo(t, h, "POST", "/api/v1/folders/rename", `{"old":"missing","new":"work"}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/folders/rename", `{"old":"","new":"work"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/folders/rename", `{"old":"work","new":"work"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
}

func TestAPIFoldersDelete(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/folders/delete", `{"folder":"docs"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if out["confirm_required"] != true || out["count"].(float64) != 1 {
		t.Fatalf("out = %v", out)
	}
	if loadTestStore(t, base).Find(1).Folder != "docs" {
		t.Fatalf("moved without confirm")
	}

	w = apiDo(t, h, "POST", "/api/v1/folders/delete", `{"folder":"docs","confirm":true}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if apiDecode(t, w)["moved"].(float64) != 1 {
		t.Fatalf("out = %s", w.Body.String())
	}
	if loadTestStore(t, base).Find(1).Folder != "" {
		t.Fatalf("not moved to root")
	}

	w = apiDo(t, h, "POST", "/api/v1/folders/delete", `{"folder":"missing","confirm":true}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/folders/delete", `{"folder":""}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/folders/bogus", `{"folder":"docs"}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func apiRulesSetup(t *testing.T) string {
	t.Helper()
	entries := []*Bookmark{
		{ID: 1, URL: "https://h.com/1", Title: "one", Folder: "docs", HTMLFile: "docs/0001-one.html"},
		{ID: 2, URL: "https://h.com/2", Title: "two", Folder: "docs", HTMLFile: "docs/0002-two.html"},
		{ID: 3, URL: "https://h.com/3", Title: "three", Folder: "docs", HTMLFile: "docs/0003-three.html"},
	}
	_, base := setupReindexTest(t, entries)
	for _, e := range entries {
		writeHTMLFile(t, base, e.HTMLFile, e.URL, e.Title)
	}
	return base
}

func TestAPIRulesListCreate(t *testing.T) {
	_ = apiRulesSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/rules", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if len(apiDecode(t, w)["rules"].([]any)) != 0 {
		t.Fatalf("out = %s", w.Body.String())
	}

	w = apiDo(t, h, "POST", "/api/v1/rules", `{"match":"host:h.com","folder":"docs"}`)
	if w.Code != http.StatusCreated {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	out := apiDecode(t, w)
	if out["match"] != "host:h.com" || out["applied_count"].(float64) != 3 {
		t.Fatalf("out = %v (backfill records the ledger on all three)", out)
	}
	id := out["id"].(float64)

	w = apiDo(t, h, "GET", "/api/v1/rules", "")
	rules := apiDecode(t, w)["rules"].([]any)
	if len(rules) != 1 || rules[0].(map[string]any)["id"].(float64) != id {
		t.Fatalf("rules = %v", rules)
	}

	w = apiDo(t, h, "POST", "/api/v1/rules", `{"folder":"docs"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/rules", `{"match":"x"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "PUT", "/api/v1/rules", `{"match":"x"}`)
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func TestAPIRulesSuggestLearn(t *testing.T) {
	_ = apiRulesSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/rules/suggestions?min=2", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	sugs := out["suggestions"].([]any)
	if len(sugs) != 1 {
		t.Fatalf("suggestions = %v", sugs)
	}
	s := sugs[0].(map[string]any)
	if s["host"] != "h.com" || s["folder"] != "docs" || s["count"].(float64) != 3 {
		t.Fatalf("suggestion = %v", s)
	}

	w = apiDo(t, h, "GET", "/api/v1/rules/suggestions?min=1", "")
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}

	w = apiDo(t, h, "POST", "/api/v1/rules/learn", `{"min":2}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	out = apiDecode(t, w)
	if len(out["created"].([]any)) != 1 {
		t.Fatalf("out = %v", out)
	}

	w = apiDo(t, h, "POST", "/api/v1/rules/learn", `{"min":2}`)
	if len(apiDecode(t, w)["created"].([]any)) != 0 {
		t.Fatalf("second learn should create nothing: %s", w.Body.String())
	}
}

func TestAPIRulesApplyDelete(t *testing.T) {
	base := apiRulesSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/rules", `{"match":"nomatch-xyz","folder":"auto"}`)
	ruleID := int(apiDecode(t, w)["id"].(float64))

	w = apiDo(t, h, "PUT", "/api/v1/bookmarks/1", `{"url":"https://nomatch-xyz.com/1"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	w = apiDo(t, h, "POST", "/api/v1/rules/apply", `{"id":999}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/rules/apply", fmt.Sprintf(`{"id":%d}`, ruleID))
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if apiDecode(t, w)["applied"].(float64) != 1 {
		t.Fatalf("out = %s", w.Body.String())
	}
	if b := loadTestStore(t, base).Find(1); b.Folder == "auto" {
		t.Fatalf("folder overwritten by apply despite existing folder: %+v", b)
	}

	w = apiDo(t, h, "DELETE", fmt.Sprintf("/api/v1/rules/%d", ruleID), "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if out["confirm_required"] != true || out["rule"] == nil {
		t.Fatalf("out = %v", out)
	}
	w = apiDo(t, h, "DELETE", fmt.Sprintf("/api/v1/rules/%d?confirm=true", ruleID), "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if apiDecode(t, w)["deleted"].(float64) != 1 {
		t.Fatalf("out = %s", w.Body.String())
	}
	w = apiDo(t, h, "DELETE", "/api/v1/rules/1?confirm=true", "")
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "DELETE", "/api/v1/rules/abc?confirm=true", "")
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func TestAPICheckRun(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	_ = base
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/check/run", `{"workers":2}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	out := apiDecode(t, w)
	if out["ok"].(float64) != 1 || out["checked"].(float64) != 3 {
		t.Fatalf("out = %v", out)
	}
	if len(out["moved"].([]any)) != 1 || len(out["dead"].([]any)) != 1 || len(out["uncertain"].([]any)) != 0 {
		t.Fatalf("buckets = %v", out)
	}
	moved := out["moved"].([]any)[0].(map[string]any)
	for _, k := range []string{"id", "title", "url", "detail", "target", "status"} {
		if _, ok := moved[k]; !ok {
			t.Fatalf("missing key %q in %v", k, moved)
		}
	}
	if moved["status"] != "moved" {
		t.Fatalf("moved = %v", moved)
	}

	w = apiDo(t, h, "POST", "/api/v1/check/run", `{"workers":0,"stale":"bogus"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/check/run", `{"spec":"abc"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "GET", "/api/v1/check/run", "")
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func TestAPICheckApply(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	base := checkWebSetup(t, srv)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/check/apply",
		`{"id":3,"action":"update","target":"`+srv.URL+`/ok"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if got := loadTestStore(t, base).Find(3).URL; got != srv.URL+"/ok" {
		t.Fatalf("url = %q", got)
	}

	w = apiDo(t, h, "POST", "/api/v1/check/apply", `{"id":2,"action":"delete"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if out["confirm_required"] != true {
		t.Fatalf("out = %v", out)
	}
	if loadTestStore(t, base).Find(2) == nil {
		t.Fatalf("deleted without confirm")
	}
	w = apiDo(t, h, "POST", "/api/v1/check/apply", `{"id":2,"action":"delete","confirm":true}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if loadTestStore(t, base).Find(2) != nil {
		t.Fatalf("not deleted")
	}

	w = apiDo(t, h, "POST", "/api/v1/check/apply", `{"id":1,"action":"quarantine"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if loadTestStore(t, base).Find(1).Folder != "quarantine" {
		t.Fatalf("not quarantined")
	}

	w = apiDo(t, h, "POST", "/api/v1/check/apply", `{"id":99,"action":"delete","confirm":true}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/check/apply", `{"id":1,"action":"bogus"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/check/apply", `{"id":1,"action":"update"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
}

func TestAPISettings(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/settings", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	for _, k := range []string{"base_dir", "active_profile", "archive_backend", "bookmarks", "tags", "folders", "rules", "maintenance_status"} {
		if _, ok := out[k]; !ok {
			t.Fatalf("missing key %q in %v", k, out)
		}
	}
	if out["bookmarks"].(float64) != 2 || out["tags"].(float64) != 1 {
		t.Fatalf("out = %v", out)
	}

	w = apiDo(t, h, "POST", "/api/v1/settings", `{"archive_backend":"native"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if apiDecode(t, w)["archive_backend"] != "native" {
		t.Fatalf("out = %s", w.Body.String())
	}
	w = apiDo(t, h, "GET", "/api/v1/settings", "")
	if apiDecode(t, w)["archive_backend"] != "native" {
		t.Fatalf("not persisted: %s", w.Body.String())
	}

	w = apiDo(t, h, "POST", "/api/v1/settings", `{"archive_backend":"bogus"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "PUT", "/api/v1/settings", `{}`)
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func apiContentSetup(t *testing.T) string {
	t.Helper()
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com/1", Title: "alpha", HTMLFile: "0001-alpha.html",
			MarkdownFile: "0001-alpha.md", ArchiveFile: "0001-alpha-archive.html",
			Attachments: []Attachment{{Name: "paper.pdf", File: "0001-paper.pdf"}}},
		{ID: 2, URL: "https://b.com/2", Title: "beta", HTMLFile: "0002-beta.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-alpha.html", entries[0].URL, "alpha")
	writeHTMLFile(t, base, "0002-beta.html", entries[1].URL, "beta")
	md := "# alpha\n\nnotes here\n"
	if err := os.MkdirAll(filepath.Join(base, "markdown"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(base, "markdown", "0001-alpha.md"), []byte(md), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(base, "archive"), 0o755); err != nil {
		t.Fatal(err)
	}
	arch := "<!DOCTYPE html><html><head><title>alpha</title></head><body>archived</body></html>"
	if err := os.WriteFile(filepath.Join(base, "archive", "0001-alpha-archive.html"), []byte(arch), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(base, "attachments"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(base, "attachments", "0001-paper.pdf"), []byte("%PDF-1.4 fake"), 0o644); err != nil {
		t.Fatal(err)
	}
	return base
}

func TestAPIContent(t *testing.T) {
	_ = apiContentSetup(t)
	h := newWebMux("")

	for _, tc := range []struct{ path, wantCT, wantBody string }{
		{"/api/v1/bookmarks/1/card", "text/html", "<title>alpha</title>"},
		{"/api/v1/bookmarks/1/archive", "text/html", "archived"},
		{"/api/v1/bookmarks/1/markdown", "text/html", "notes here"},
	} {
		w := apiDo(t, h, "GET", tc.path, "")
		if w.Code != http.StatusOK {
			t.Fatalf("%s: code = %d", tc.path, w.Code)
		}
		if ct := w.Header().Get("Content-Type"); !strings.Contains(ct, tc.wantCT) {
			t.Fatalf("%s: content-type = %q", tc.path, ct)
		}
		if !strings.Contains(w.Body.String(), tc.wantBody) {
			t.Fatalf("%s: body missing %q", tc.path, tc.wantBody)
		}
	}

	w := apiDo(t, h, "GET", "/api/v1/bookmarks/1/attachments/1", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	if !strings.Contains(w.Body.String(), "%PDF-1.4 fake") {
		t.Fatalf("attachment body wrong")
	}
	if cd := w.Header().Get("Content-Disposition"); !strings.Contains(cd, "paper.pdf") {
		t.Fatalf("content-disposition = %q", cd)
	}

	for _, path := range []string{
		"/api/v1/bookmarks/2/archive",
		"/api/v1/bookmarks/2/markdown",
		"/api/v1/bookmarks/1/attachments/2",
		"/api/v1/bookmarks/99/card",
		"/api/v1/bookmarks/abc/card",
	} {
		w := apiDo(t, h, "GET", path, "")
		if w.Code != http.StatusNotFound {
			t.Fatalf("%s: code = %d, want 404", path, w.Code)
		}
	}
	w = apiDo(t, h, "POST", "/api/v1/bookmarks/1/card", "")
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func TestAPIRulesEdit(t *testing.T) {
	_ = apiRulesSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/rules", `{"match":"host:h.com","folder":"docs"}`)
	ruleID := int(apiDecode(t, w)["id"].(float64))

	w = apiDo(t, h, "PUT", fmt.Sprintf("/api/v1/rules/%d", ruleID), `{"folder":"work","reapply":true}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	out := apiDecode(t, w)
	if out["folder"] != "work" {
		t.Fatalf("out = %v", out)
	}
	if out["reapplied"].(float64) != 0 {
		t.Fatalf("reapplied = %v, want 0 (all already classified)", out)
	}

	w = apiDo(t, h, "PUT", "/api/v1/rules/999", `{"folder":"work"}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "PUT", "/api/v1/rules/abc", `{"folder":"work"}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "PUT", fmt.Sprintf("/api/v1/rules/%d", ruleID), `not json`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
}

func TestAPIProfiles(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/profiles", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if out["active"] != "default" || len(out["profiles"].([]any)) != 1 {
		t.Fatalf("out = %v", out)
	}

	w = apiDo(t, h, "POST", "/api/v1/profiles/switch", `{"name":"work"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if apiDecode(t, w)["active"] != "work" {
		t.Fatalf("out = %s", w.Body.String())
	}
	w = apiDo(t, h, "GET", "/api/v1/profiles", "")
	names := map[string]bool{}
	for _, p := range apiDecode(t, w)["profiles"].([]any) {
		m := p.(map[string]any)
		names[m["name"].(string)] = m["active"].(bool)
	}
	if !names["work"] || len(names) != 2 {
		t.Fatalf("profiles = %v", names)
	}

	w = apiDo(t, h, "POST", "/api/v1/profiles/delete", `{"name":"work"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400 (active)", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/profiles/switch", `{"name":"default"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	w = apiDo(t, h, "POST", "/api/v1/profiles/delete", `{"name":"work"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	w = apiDo(t, h, "POST", "/api/v1/profiles/delete", `{"name":"missing"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/profiles/switch", `{"name":"bad/name"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/profiles/bogus", `{}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func TestAPIHistory(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "GET", "/api/v1/history", "")
	if len(apiDecode(t, w)["history"].([]any)) != 0 {
		t.Fatalf("history should start empty: %s", w.Body.String())
	}

	apiDo(t, h, "POST", "/api/v1/bookmarks/2/open", "")
	apiDo(t, h, "POST", "/api/v1/bookmarks/1/open", "")
	w = apiDo(t, h, "GET", "/api/v1/history", "")
	rows := apiDecode(t, w)["history"].([]any)
	if len(rows) != 2 {
		t.Fatalf("history = %v", rows)
	}
	if rows[0].(map[string]any)["id"].(float64) != 1 || rows[1].(map[string]any)["id"].(float64) != 2 {
		t.Fatalf("order = %v, want most recent first", rows)
	}
	first := rows[0].(map[string]any)
	for _, k := range []string{"id", "title", "url", "open_count", "last_opened_at"} {
		if _, ok := first[k]; !ok {
			t.Fatalf("missing key %q in %v", k, first)
		}
	}

	w = apiDo(t, h, "POST", "/api/v1/history", "")
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func TestAPIBulk(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/bulk", `{"ids":[1,2],"action":"tags","tags":["z"]}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	for _, id := range []int{1, 2} {
		if tags := loadTestStore(t, base).Find(id).Tags; len(tags) != 1 || tags[0] != "z" {
			t.Fatalf("[%d] tags = %v", id, tags)
		}
	}

	w = apiDo(t, h, "POST", "/api/v1/bulk", `{"ids":[1,2],"action":"folder","folder":"bulk"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if f := loadTestStore(t, base).Find(2).Folder; f != "bulk" {
		t.Fatalf("folder = %q", f)
	}

	w = apiDo(t, h, "POST", "/api/v1/bulk", `{"ids":[1,2],"action":"delete"}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if out["confirm_required"] != true || out["count"].(float64) != 2 || len(out["titles"].([]any)) != 2 {
		t.Fatalf("out = %v", out)
	}
	if loadTestStore(t, base).Find(1) == nil {
		t.Fatalf("deleted without confirm")
	}
	w = apiDo(t, h, "POST", "/api/v1/bulk", `{"ids":[1,2],"action":"delete","confirm":true}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if apiDecode(t, w)["count"].(float64) != 2 {
		t.Fatalf("out = %s", w.Body.String())
	}
	if s := loadTestStore(t, base); s.Find(1) != nil || s.Find(2) != nil {
		t.Fatalf("not deleted")
	}

	w = apiDo(t, h, "POST", "/api/v1/bulk", `{"ids":[99],"action":"tags","tags":["z"]}`)
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/bulk", `{"ids":[1],"action":"bogus"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "GET", "/api/v1/bulk", "")
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func TestAPILibraryImportExport(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")
	doc := `<!DOCTYPE NETSCAPE-Bookmark-file-1><DL><p><DT><A HREF="https://n.com/1" TAGS="imp">New One</A></DL><p>`
	payload := `{"content":` + strconv.Quote(doc) + `}`

	w := apiDo(t, h, "POST", "/api/v1/library/import", payload)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	out := apiDecode(t, w)
	if out["imported"].(float64) != 1 {
		t.Fatalf("out = %v", out)
	}

	w = apiDo(t, h, "POST", "/api/v1/library/import", `{"content":"no bookmarks here"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/library/import", `{"content":""}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}

	w = apiDo(t, h, "GET", "/api/v1/library/export", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	body := w.Body.String()
	if !strings.Contains(body, "New One") || !strings.Contains(body, "alpha one") {
		t.Fatalf("export missing entries")
	}
	if cd := w.Header().Get("Content-Disposition"); !strings.Contains(cd, "liber-bookmarks.html") {
		t.Fatalf("content-disposition = %q", cd)
	}

	w = apiDo(t, h, "POST", "/api/v1/library/site", `{}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if !fileExists(filepath.Join(base, "site", "index.html")) {
		t.Fatalf("site not written")
	}

	w = apiDo(t, h, "GET", "/api/v1/library/bogus", "")
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}

func TestAPISyncReindex(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/sync", `{}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
	out := apiDecode(t, w)
	if _, ok := out["output"]; !ok {
		t.Fatalf("out = %v", out)
	}

	w = apiDo(t, h, "POST", "/api/v1/reindex", `{"merge":true}`)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	if _, ok := apiDecode(t, w)["output"]; !ok {
		t.Fatalf("no output: %s", w.Body.String())
	}
	w = apiDo(t, h, "POST", "/api/v1/reindex", `{"all":true}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("code = %d, want 400", w.Code)
	}
	w = apiDo(t, h, "GET", "/api/v1/sync", "")
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
}

func TestAPIMethodAndAuth(t *testing.T) {
	_ = apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "PUT", "/api/v1/bookmarks", `{"title":"x"}`)
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}

	authed := newWebMux(testAuthToken)
	w = apiDo(t, authed, "GET", "/api/v1/bookmarks", "")
	if w.Code != http.StatusUnauthorized {
		t.Fatalf("code = %d, want 401", w.Code)
	}
	r := httptest.NewRequest(http.MethodPost, "/api/v1/bookmarks", strings.NewReader(`{"url":"https://z.com/9","title":"zeta"}`))
	r.Header.Set("Content-Type", "application/json")
	r.Header.Set("Authorization", "Bearer "+authMAC(testAuthToken, "liber-bearer-v1"))
	w = httptest.NewRecorder()
	authed.ServeHTTP(w, r)
	if w.Code != http.StatusCreated {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
}

func TestAPIOpen(t *testing.T) {
	base := apiTestSetup(t)
	h := newWebMux("")

	w := apiDo(t, h, "POST", "/api/v1/bookmarks/1/open", "")
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d: %s", w.Code, w.Body.String())
	}
	out := apiDecode(t, w)
	if out["url"] != "https://a.com/1" {
		t.Fatalf("out = %v", out)
	}
	b := loadTestStore(t, base).Find(1)
	if b.OpenCount != 1 || b.LastOpenedAt == nil {
		t.Fatalf("history not recorded: %+v", b)
	}

	w = apiDo(t, h, "GET", "/api/v1/bookmarks/1/open", "")
	if w.Code != http.StatusMethodNotAllowed {
		t.Fatalf("code = %d, want 405", w.Code)
	}
	w = apiDo(t, h, "POST", "/api/v1/bookmarks/99/open", "")
	if w.Code != http.StatusNotFound {
		t.Fatalf("code = %d, want 404", w.Code)
	}
}
