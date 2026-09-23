package main

import (
	"bytes"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestConfigPathEnvOverride(t *testing.T) {
	dir := t.TempDir()
	custom := filepath.Join(dir, "custom.json")
	t.Setenv("LIBER_CONFIG", custom)
	got, err := configPath()
	if err != nil {
		t.Fatal(err)
	}
	if got != custom {
		t.Fatalf("configPath = %q, want %q", got, custom)
	}
}

func TestLoadConfigBaseDirEnvOverride(t *testing.T) {
	dir := t.TempDir()
	cfgPath := filepath.Join(dir, "config.json")
	base := filepath.Join(dir, "Collection")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cfgPath, []byte(`{"base_dir": "/should/be/overridden"}`), 0o644); err != nil {
		t.Fatal(err)
	}
	t.Setenv("LIBER_CONFIG", cfgPath)
	t.Setenv("LIBER_BASE_DIR", base)
	cfg, _, err := LoadConfig()
	if err != nil {
		t.Fatal(err)
	}
	if cfg.BaseDir != base {
		t.Fatalf("BaseDir = %q, want %q", cfg.BaseDir, base)
	}
	if got := cfg.effectiveBaseDir(); got != base {
		t.Fatalf("effectiveBaseDir = %q, want %q", got, base)
	}
}

func TestResolveAuthTokenPrecedence(t *testing.T) {
	cfg := Config{AuthToken: "file-token"}
	t.Setenv("LIBER_AUTH_TOKEN", "env-token")
	if got := resolveAuthToken("", cfg); got != "env-token" {
		t.Fatalf("env should beat file, got %q", got)
	}
	if got := resolveAuthToken("flag-token", cfg); got != "flag-token" {
		t.Fatalf("flag should beat env, got %q", got)
	}
	t.Setenv("LIBER_AUTH_TOKEN", "")
	if got := resolveAuthToken("", cfg); got != "file-token" {
		t.Fatalf("file should apply when nothing above, got %q", got)
	}
	if got := resolveAuthToken("", Config{}); got != "" {
		t.Fatalf("unset should stay unset, got %q", got)
	}
}

func TestConfigSetAuthTokenRoundTrip(t *testing.T) {
	dir := t.TempDir()
	t.Setenv("XDG_CONFIG_HOME", dir)
	if err := runConfigSet([]string{"auth_token", "s3cret-token"}); err != nil {
		t.Fatalf("set rejected: %v", err)
	}
	cfg, path, err := LoadConfig()
	if err != nil {
		t.Fatal(err)
	}
	if cfg.AuthToken != "s3cret-token" {
		t.Fatalf("AuthToken = %q", cfg.AuthToken)
	}
	fi, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if fi.Mode().Perm() != 0o600 {
		t.Fatalf("config mode = %o, want 600", fi.Mode().Perm())
	}
	out := captureStdout(t, func() {
		if err := runConfigCmd(nil); err != nil {
			t.Errorf("runConfigCmd: %v", err)
		}
	})
	if strings.Contains(out, "s3cret-token") {
		t.Fatalf("token leaked into config output")
	}
	if !strings.Contains(out, "auth_token") || !strings.Contains(out, "set") {
		t.Fatalf("missing redacted auth_token line:\n%s", out)
	}
}

func captureStdout(t *testing.T, fn func()) string {
	t.Helper()
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	os.Stdout = w
	fn()
	w.Close()
	os.Stdout = old
	var buf bytes.Buffer
	if _, err := io.Copy(&buf, r); err != nil {
		t.Fatal(err)
	}
	return buf.String()
}
