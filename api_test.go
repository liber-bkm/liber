package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
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
