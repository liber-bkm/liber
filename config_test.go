package main

import (
	"os"
	"path/filepath"
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
