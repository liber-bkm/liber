package main

import (
	"bytes"
	"crypto/hmac"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"fmt"
	"html/template"
	"net/http"
	neturl "net/url"
	"strings"
)

const authCookieName = "liber_auth"

func authMAC(token, purpose string) string {
	h := hmac.New(sha256.New, []byte(token))
	h.Write([]byte(purpose))
	return hex.EncodeToString(h.Sum(nil))
}

func checkBearer(r *http.Request, token string) bool {
	got, err := hex.DecodeString(strings.TrimSpace(strings.TrimPrefix(r.Header.Get("Authorization"), "Bearer")))
	if err != nil || len(got) == 0 {
		return false
	}
	return subtle.ConstantTimeCompare(got, mustDecodeHex(authMAC(token, "liber-bearer-v1"))) == 1
}

func mustDecodeHex(s string) []byte {
	b, _ := hex.DecodeString(s)
	return b
}

func checkCookie(r *http.Request, token string) bool {
	c, err := r.Cookie(authCookieName)
	if err != nil || c.Value == "" {
		return false
	}
	got, err := hex.DecodeString(c.Value)
	if err != nil {
		return false
	}
	return subtle.ConstantTimeCompare(got, mustDecodeHex(authMAC(token, "liber-cookie-v1"))) == 1
}

func sameOrigin(r *http.Request) bool {
	if o := strings.TrimSpace(r.Header.Get("Origin")); o != "" {
		u, err := neturl.Parse(o)
		if err != nil || !strings.EqualFold(u.Host, r.Host) {
			return false
		}
	}
	if ref := strings.TrimSpace(r.Header.Get("Referer")); ref != "" {
		u, err := neturl.Parse(ref)
		if err != nil || !strings.EqualFold(u.Host, r.Host) {
			return false
		}
	}
	return true
}

func withAuth(token string, next http.Handler) http.Handler {
	if token == "" {
		return next
	}
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/login" || r.URL.Path == "/logout" {
			next.ServeHTTP(w, r)
			return
		}
		if checkBearer(r, token) {
			next.ServeHTTP(w, r)
			return
		}
		if checkCookie(r, token) {
			switch r.Method {
			case http.MethodPost, http.MethodPut, http.MethodDelete, http.MethodPatch:
				if !sameOrigin(r) {
					http.Error(w, "cross-origin request refused", http.StatusForbidden)
					return
				}
			}
			next.ServeHTTP(w, r)
			return
		}
		if strings.HasPrefix(r.URL.Path, "/api/") {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusUnauthorized)
			fmt.Fprint(w, `{"error":"authentication required"}`+"\n")
			return
		}
		http.Redirect(w, r, "/login?next="+neturl.QueryEscape(r.URL.RequestURI()), http.StatusSeeOther)
	})
}

func handleLogin(token string) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if token == "" {
			http.Redirect(w, r, "/", http.StatusSeeOther)
			return
		}
		if r.Method == http.MethodPost {
			if err := r.ParseForm(); err != nil {
				http.Error(w, "bad form", http.StatusBadRequest)
				return
			}
			got := []byte(r.FormValue("token"))
			want := []byte(token)
			if len(got) == 0 || subtle.ConstantTimeCompare(got, want) != 1 {
				renderLoginPage(w, "Wrong token.")
				return
			}
			http.SetCookie(w, &http.Cookie{
				Name:     authCookieName,
				Value:    authMAC(token, "liber-cookie-v1"),
				Path:     "/",
				HttpOnly: true,
				SameSite: http.SameSiteLaxMode,
			})
			next := r.FormValue("next")
			if next == "" || !strings.HasPrefix(next, "/") {
				next = "/"
			}
			http.Redirect(w, r, next, http.StatusSeeOther)
			return
		}
		renderLoginPage(w, "")
	}
}

func handleLogout(w http.ResponseWriter, r *http.Request) {
	http.SetCookie(w, &http.Cookie{
		Name:     authCookieName,
		Value:    "",
		Path:     "/",
		MaxAge:   -1,
		HttpOnly: true,
		SameSite: http.SameSiteLaxMode,
	})
	http.Redirect(w, r, "/login", http.StatusSeeOther)
}

func renderLoginPage(w http.ResponseWriter, errMsg string) {
	var buf bytes.Buffer
	if err := loginBodyTmpl.Execute(&buf, struct{ Error string }{errMsg}); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	layoutTmpl.Execute(w, struct {
		Title string
		Body  template.HTML
	}{"liber - login", template.HTML(buf.String())})
}

var loginBodyTmpl = template.Must(template.New("loginBody").Parse(`
<h2>Login</h2>
<p class="count">This liber server requires the auth token.</p>
{{if .Error}}<p class="flash error">{{.Error}}</p>{{end}}
<form method="post" action="/login" class="addform">
  <input type="password" name="token" placeholder="Auth token" required autofocus>
  <button type="submit">Log in</button>
</form>
`))
