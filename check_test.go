package main

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
)

func checkFixture() *httptest.Server {
	mux := http.NewServeMux()
	mux.HandleFunc("/ok", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(200)
	})
	mux.HandleFunc("/gone", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(404)
	})
	mux.HandleFunc("/moved", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Location", "/ok")
		w.WriteHeader(301)
	})
	mux.HandleFunc("/denied", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(403)
	})
	mux.HandleFunc("/notice", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Location", "http://foreign.invalid/x")
		w.WriteHeader(302)
	})
	return httptest.NewServer(mux)
}

func TestClassifyURL(t *testing.T) {
	srv := checkFixture()
	defer srv.Close()
	client := checkClient()
	cases := []struct {
		path   string
		status checkStatus
		target string
	}{
		{"/ok", checkOK, ""},
		{"/gone", checkDead, ""},
		{"/moved", checkMoved, srv.URL + "/ok"},
		{"/denied", checkUncertain, ""},
		{"/notice", checkUncertain, ""},
	}
	for _, c := range cases {
		r := classifyURL(client, srv.URL+c.path)
		if r.status != c.status {
			t.Errorf("classifyURL(%s) status = %d, want %d (%s)", c.path, r.status, c.status, r.detail)
		}
		if r.target != c.target {
			t.Errorf("classifyURL(%s) target = %q, want %q", c.path, r.target, c.target)
		}
	}
}

func TestClassifyURLError(t *testing.T) {
	client := checkClient()
	r := classifyURL(client, "http://127.0.0.1:1/none")
	if r.status != checkUncertain {
		t.Errorf("refused connection status = %d, want uncertain", r.status)
	}
}

func TestResolveRef(t *testing.T) {
	if got := resolveRef("https://h.com/a/b", "/c"); got != "https://h.com/c" {
		t.Errorf("relative resolve = %q", got)
	}
	if got := resolveRef("https://h.com/a", "https://x.com/y"); got != "https://x.com/y" {
		t.Errorf("absolute resolve = %q", got)
	}
}

func TestParseCheckArgs(t *testing.T) {
	spec, workers, _, err := parseCheckArgs([]string{"1-3", "--workers", "4"})
	if err != nil || spec != "1-3" || workers != 4 {
		t.Errorf("parseCheckArgs = %q %d %v", spec, workers, err)
	}
	if _, _, _, err := parseCheckArgs([]string{"--workers", "0"}); err == nil {
		t.Error("workers 0 should fail")
	}
	if _, _, _, err := parseCheckArgs([]string{"--bogus"}); err == nil {
		t.Error("unknown flag should fail")
	}
}

func TestShortErr(t *testing.T) {
	err := fmt.Errorf("Get %q: dial tcp: lookup x: no such host", "http://x")
	if got := shortErr(err); got == "" || len(got) > 120 {
		t.Errorf("shortErr = %q", got)
	}
}
