package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

type Config struct {
	BaseDir string `json:"base_dir"`

	HTMLDir       string `json:"html_dir,omitempty"`
	MarkdownDir   string `json:"markdown_dir,omitempty"`
	ArchiveDir    string `json:"archive_dir,omitempty"`
	AttachmentDir string `json:"attachment_dir,omitempty"`

	SingleFileCmd string `json:"singlefile_cmd,omitempty"`

	SingleFileBrowserPath string `json:"singlefile_browser_path,omitempty"`

	ArchiveBackend string `json:"archive_backend,omitempty"`

	MonolithCmd string `json:"monolith_cmd,omitempty"`

	MonolithBrowserPath string `json:"monolith_browser_path,omitempty"`

	MonolithUseBrowser bool `json:"monolith_use_browser,omitempty"`

	BrowserCmd string `json:"browser_cmd,omitempty"`

	EditorCmd string `json:"editor_cmd,omitempty"`

	ActiveProfile string   `json:"active_profile,omitempty"`
	Profiles      []string `json:"profiles,omitempty"`
}

func configPath() (string, error) {
	dir, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "liber", "config.json"), nil
}

func defaultConfig() Config {
	home, _ := os.UserHomeDir()
	return Config{
		BaseDir:       filepath.Join(home, "Bookmarks"),
		SingleFileCmd: "single-file",
	}
}

func LoadConfig() (Config, string, error) {
	path, err := configPath()
	if err != nil {
		return Config{}, "", err
	}

	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		cfg := defaultConfig()
		if werr := SaveConfig(cfg); werr != nil {
			fmt.Fprintf(os.Stderr, "warning: could not write default config: %v\n", werr)
		}
		return cfg, path, nil
	} else if err != nil {
		return Config{}, path, err
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return Config{}, path, fmt.Errorf("parsing %s: %w", path, err)
	}
	if cfg.SingleFileCmd == "" {
		cfg.SingleFileCmd = "single-file"
	}
	if cfg.BaseDir == "" {
		cfg.BaseDir = defaultConfig().BaseDir
	}
	return cfg, path, nil
}

func SaveConfig(cfg Config) error {
	path, err := configPath()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0o644)
}

func (c Config) effectiveBaseDir() string {
	base := expandTilde(c.BaseDir)
	if c.ActiveProfile != "" {
		return filepath.Join(base, c.ActiveProfile)
	}
	return base
}

func (c Config) htmlDir() string {
	if c.HTMLDir != "" {
		return expandTilde(c.HTMLDir)
	}
	return filepath.Join(c.effectiveBaseDir(), "html")
}

func (c Config) markdownDir() string {
	if c.MarkdownDir != "" {
		return expandTilde(c.MarkdownDir)
	}
	return filepath.Join(c.effectiveBaseDir(), "markdown")
}

func (c Config) archiveDir() string {
	if c.ArchiveDir != "" {
		return expandTilde(c.ArchiveDir)
	}
	return filepath.Join(c.effectiveBaseDir(), "archive")
}

func (c Config) attachmentsDir() string {
	if c.AttachmentDir != "" {
		return expandTilde(c.AttachmentDir)
	}
	return filepath.Join(c.effectiveBaseDir(), "attachments")
}

func (c Config) indexPath() string {
	return filepath.Join(c.effectiveBaseDir(), ".liber", "index.json")
}

var settableKeys = []string{
	"base_dir", "html_dir", "markdown_dir", "archive_dir", "attachment_dir",
	"singlefile_cmd", "singlefile_browser_path", "archive_backend",
	"monolith_cmd", "monolith_browser_path", "monolith_use_browser",
	"browser_cmd", "editor_cmd",
}

func runConfigSet(args []string) error {
	if len(args) != 2 {
		return fmt.Errorf("usage: liber config set <key> <value> (keys: %s)", strings.Join(settableKeys, ", "))
	}
	key, val := args[0], args[1]
	if strings.TrimSpace(val) == "" {
		return fmt.Errorf("value can't be empty")
	}
	cfg, path, err := LoadConfig()
	if err != nil {
		return err
	}
	switch key {
	case "base_dir":
		cfg.BaseDir = val
	case "html_dir":
		cfg.HTMLDir = val
	case "markdown_dir":
		cfg.MarkdownDir = val
	case "archive_dir":
		cfg.ArchiveDir = val
	case "attachment_dir":
		cfg.AttachmentDir = val
	case "singlefile_cmd":
		cfg.SingleFileCmd = val
	case "singlefile_browser_path":
		cfg.SingleFileBrowserPath = val
	case "archive_backend":
		switch val {
		case "auto", "single-file", "monolith", "native":
			cfg.ArchiveBackend = val
		default:
			return fmt.Errorf("invalid archive_backend %q (expected auto, single-file, monolith, or native)", val)
		}
	case "monolith_cmd":
		cfg.MonolithCmd = val
	case "monolith_browser_path":
		cfg.MonolithBrowserPath = val
	case "monolith_use_browser":
		b, err := strconv.ParseBool(val)
		if err != nil {
			return fmt.Errorf("invalid monolith_use_browser %q (expected true or false)", val)
		}
		cfg.MonolithUseBrowser = b
	case "browser_cmd":
		cfg.BrowserCmd = val
	case "editor_cmd":
		cfg.EditorCmd = val
	default:
		return fmt.Errorf("unknown key %q (keys: %s)", key, strings.Join(settableKeys, ", "))
	}
	if err := SaveConfig(cfg); err != nil {
		return err
	}
	fmt.Printf("Set %s in %s\n", key, path)
	return nil
}
