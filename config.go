package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
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
