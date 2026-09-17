package main

import (
	"fmt"
	"path/filepath"
	"sort"
	"strings"
)

func validateProfileName(name string) (string, error) {
	name = strings.TrimSpace(name)
	if name == "" {
		return "", fmt.Errorf("profile name can't be empty")
	}
	if name == "." || name == ".." {
		return "", fmt.Errorf("invalid profile name %q", name)
	}
	if strings.ContainsAny(name, "/\\") {
		return "", fmt.Errorf("profile name can't contain a path separator")
	}
	return name, nil
}

func runProfile(args []string) error {
	if len(args) == 0 {
		return runProfileList()
	}
	if args[0] == "delete" {
		if len(args) < 2 {
			return fmt.Errorf("usage: liber --profile delete <name>")
		}
		return runProfileDelete(args[1])
	}
	return runProfileSwitch(args[0])
}

type profileEntry struct {
	Name    string
	Path    string
	Active  bool
	Default bool
}

func profileListData() (entries []profileEntry, err error) {
	cfg, _, err := LoadConfig()
	if err != nil {
		return nil, fmt.Errorf("loading config: %w", err)
	}
	base := expandTilde(cfg.BaseDir)
	entries = append(entries, profileEntry{Name: "default", Path: base, Active: cfg.ActiveProfile == "", Default: true})
	names := append([]string{}, cfg.Profiles...)
	sort.Strings(names)
	for _, name := range names {
		entries = append(entries, profileEntry{Name: name, Path: filepath.Join(base, name), Active: cfg.ActiveProfile == name})
	}
	return entries, nil
}
func runProfileList() error {
	entries, err := profileListData()
	if err != nil {
		return err
	}
	mark := func(active bool) string {
		if active {
			return "* "
		}
		return "  "
	}
	for _, e := range entries {
		fmt.Printf("%s%-15s %s\n", mark(e.Active), e.Name, e.Path)
	}
	return nil
}

func runProfileSwitch(rawName string) error {
	msg, err := profileSwitchMsg(rawName)
	if err != nil {
		return err
	}
	fmt.Println(msg)
	return nil
}

func profileSwitchMsg(rawName string) (string, error) {
	rawName = strings.TrimSpace(rawName)

	cfg, _, err := LoadConfig()
	if err != nil {
		return "", fmt.Errorf("loading config: %w", err)
	}

	if rawName == "default" {
		cfg.ActiveProfile = ""
		if err := SaveConfig(cfg); err != nil {
			return "", fmt.Errorf("saving config: %w", err)
		}
		return "Switched to default (no profile).", nil
	}

	name, err := validateProfileName(rawName)
	if err != nil {
		return "", err
	}

	inList := false
	for _, p := range cfg.Profiles {
		if p == name {
			inList = true
			break
		}
	}
	alreadyOnDisk := fileExists(filepath.Join(expandTilde(cfg.BaseDir), name))

	if !inList {
		cfg.Profiles = append(cfg.Profiles, name)
	}
	cfg.ActiveProfile = name

	if err := SaveConfig(cfg); err != nil {
		return "", fmt.Errorf("saving config: %w", err)
	}

	if !inList && !alreadyOnDisk {
		return fmt.Sprintf("Created and switched to profile %q.", name), nil
	}
	return fmt.Sprintf("Switched to profile %q.", name), nil
}

func runProfileDelete(rawName string) error {
	msg, err := profileDeleteMsg(rawName)
	if err != nil {
		return err
	}
	fmt.Println(msg)
	return nil
}

func profileDeleteMsg(rawName string) (string, error) {
	name, err := validateProfileName(rawName)
	if err != nil {
		return "", err
	}

	cfg, _, err := LoadConfig()
	if err != nil {
		return "", fmt.Errorf("loading config: %w", err)
	}
	if cfg.ActiveProfile == name {
		return "", fmt.Errorf("can't delete the active profile -- switch away first (liber --profile default, or liber --profile <other>)")
	}

	idx := -1
	for i, p := range cfg.Profiles {
		if p == name {
			idx = i
			break
		}
	}
	if idx == -1 {
		return "", fmt.Errorf("no profile named %q (see `liber --profile`)", name)
	}
	cfg.Profiles = append(cfg.Profiles[:idx], cfg.Profiles[idx+1:]...)

	if err := SaveConfig(cfg); err != nil {
		return "", fmt.Errorf("saving config: %w", err)
	}
	return fmt.Sprintf("Removed profile %q from the list. Its folder and bookmarks are untouched on disk.", name), nil
}
