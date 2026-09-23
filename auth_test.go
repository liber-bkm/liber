package main

import (
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
)

const testAuthToken = "test-secret-token"

func authTestMux(t *testing.T) (*Store, string) {
	t.Helper()
	entries := []*Bookmark{
		{ID: 1, URL: "https://a.com", Title: "a", HTMLFile: "0001-a.html"},
	}
	_, base := setupReindexTest(t, entries)
	writeHTMLFile(t, base, "0001-a.html", "https://a.com", "a")
	return loadTestStore(t, base), base
}

func TestAuthDisabled(t *testing.T) {
	_, _ = authTestMux(t)
	r := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()
	newWebMux("").ServeHTTP(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}
}

func TestAuthRedirectAndAPI401(t *testing.T) {
	_, _ = authTestMux(t)
	h := newWebMux(testAuthToken)

	r := httptest.NewRequest(http.MethodGet, "/", nil)
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d, want redirect", w.Code)
	}
	if loc := w.Header().Get("Location"); !strings.HasPrefix(loc, "/login?next=") {
		t.Fatalf("location = %q", loc)
	}

	r = httptest.NewRequest(http.MethodGet, "/api/bookmarks", nil)
	w = httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusUnauthorized {
		t.Fatalf("code = %d, want 401", w.Code)
	}
	if ct := w.Header().Get("Content-Type"); ct != "application/json" {
		t.Fatalf("content-type = %q", ct)
	}
}

func TestAuthBearer(t *testing.T) {
	_, _ = authTestMux(t)
	h := newWebMux(testAuthToken)

	r := httptest.NewRequest(http.MethodGet, "/", nil)
	r.Header.Set("Authorization", "Bearer "+authMAC(testAuthToken, "liber-bearer-v1"))
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}

	r = httptest.NewRequest(http.MethodGet, "/", nil)
	r.Header.Set("Authorization", "Bearer wrong")
	w = httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d, want redirect", w.Code)
	}
}

func authCookie() *http.Cookie {
	return &http.Cookie{Name: authCookieName, Value: authMAC(testAuthToken, "liber-cookie-v1")}
}

func TestAuthLoginFlow(t *testing.T) {
	_, _ = authTestMux(t)
	h := newWebMux(testAuthToken)

	r := httptest.NewRequest(http.MethodGet, "/login", nil)
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "Auth token") {
		t.Fatalf("login page code = %d", w.Code)
	}

	form := url.Values{"token": {testAuthToken}, "next": {"/history"}}
	r = httptest.NewRequest(http.MethodPost, "/login", strings.NewReader(form.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w = httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d", w.Code)
	}
	if loc := w.Header().Get("Location"); loc != "/history" {
		t.Fatalf("location = %q", loc)
	}
	setCookie := w.Header().Get("Set-Cookie")
	if !strings.Contains(setCookie, authCookieName+"=") || !strings.Contains(setCookie, "HttpOnly") {
		t.Fatalf("set-cookie = %q", setCookie)
	}

	r = httptest.NewRequest(http.MethodGet, "/history", nil)
	r.AddCookie(authCookie())
	w = httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusOK {
		t.Fatalf("code = %d", w.Code)
	}

	form = url.Values{"token": {"wrong"}}
	r = httptest.NewRequest(http.MethodPost, "/login", strings.NewReader(form.Encode()))
	r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	w = httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusOK || !strings.Contains(w.Body.String(), "Wrong token") {
		t.Fatalf("code = %d", w.Code)
	}

	bad := &http.Cookie{Name: authCookieName, Value: "deadbeef"}
	r = httptest.NewRequest(http.MethodGet, "/", nil)
	r.AddCookie(bad)
	w = httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d, want redirect", w.Code)
	}
}

func TestAuthLogout(t *testing.T) {
	_, _ = authTestMux(t)
	h := newWebMux(testAuthToken)

	r := httptest.NewRequest(http.MethodGet, "/logout", nil)
	r.AddCookie(authCookie())
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusSeeOther {
		t.Fatalf("code = %d", w.Code)
	}
	if sc := w.Header().Get("Set-Cookie"); !strings.Contains(sc, "Max-Age=0") {
		t.Fatalf("set-cookie = %q", sc)
	}
}

func TestAuthCSRF(t *testing.T) {
	_, _ = authTestMux(t)
	h := newWebMux(testAuthToken)
	post := func(origin, referer, auth string, cookie bool) *httptest.ResponseRecorder {
		r := httptest.NewRequest(http.MethodPost, "/delete", strings.NewReader(url.Values{"id": {"abc"}}.Encode()))
		r.Header.Set("Content-Type", "application/x-www-form-urlencoded")
		if origin != "" {
			r.Header.Set("Origin", origin)
		}
		if referer != "" {
			r.Header.Set("Referer", referer)
		}
		if auth != "" {
			r.Header.Set("Authorization", "Bearer "+auth)
		}
		if cookie {
			r.AddCookie(authCookie())
		}
		w := httptest.NewRecorder()
		h.ServeHTTP(w, r)
		return w
	}

	// Cross-origin cookie POST is refused before reaching the handler.
	if w := post("https://evil.com", "", "", true); w.Code != http.StatusForbidden {
		t.Fatalf("cross-origin code = %d, want 403", w.Code)
	}
	if w := post("", "https://evil.com/x", "", true); w.Code != http.StatusForbidden {
		t.Fatalf("cross-referer code = %d, want 403", w.Code)
	}
	// Same-origin cookie POST passes auth (handler 404s on bad id, which proves passage).
	if w := post("http://example.com", "", "", true); w.Code != http.StatusNotFound {
		t.Fatalf("same-origin code = %d, want 404", w.Code)
	}
	// No origin headers: allowed through (curl-style clients).
	if w := post("", "", "", true); w.Code != http.StatusNotFound {
		t.Fatalf("no-origin code = %d, want 404", w.Code)
	}
	// Bearer skips origin checks entirely.
	if w := post("https://evil.com", "", authMAC(testAuthToken, "liber-bearer-v1"), false); w.Code != http.StatusNotFound {
		t.Fatalf("bearer code = %d, want 404", w.Code)
	}
}

func TestAuthLoginDisabled(t *testing.T) {
	_, _ = authTestMux(t)
	h := newWebMux("")
	r := httptest.NewRequest(http.MethodGet, "/login", nil)
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	if w.Code != http.StatusSeeOther || w.Header().Get("Location") != "/" {
		t.Fatalf("code = %d loc = %q", w.Code, w.Header().Get("Location"))
	}
}

func TestParseServeFlagsAuth(t *testing.T) {
	opt, err := parseServeFlags([]string{"--addr", "127.0.0.1:9000", "--auth-token", "s3cret"})
	if err != nil {
		t.Fatal(err)
	}
	if opt.addr != "127.0.0.1:9000" || opt.token != "s3cret" {
		t.Fatalf("opt = %+v", opt)
	}
	if _, err := parseServeFlags([]string{"--auth-token"}); err == nil {
		t.Fatalf("missing value accepted")
	}
	if _, err := parseServeFlags([]string{"--bogus"}); err == nil {
		t.Fatalf("unknown flag accepted")
	}
}
