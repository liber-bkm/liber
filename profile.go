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

func runProfileList() error {
	cfg, _, err := LoadConfig()
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}
	base := expandTilde(cfg.BaseDir)

	mark := func(active bool) string {
		if active {
			return "* "
		}
		return "  "
	}

	fmt.Printf("%s%-15s %s\n", mark(cfg.ActiveProfile == ""), "default", base)

	names := append([]string{}, cfg.Profiles...)
	sort.Strings(names)
	for _, name := range names {
		fmt.Printf("%s%-15s %s\n", mark(cfg.ActiveProfile == name), name, filepath.Join(base, name))
	}
	return nil
}

func runProfileSwitch(rawName string) error {
	rawName = strings.TrimSpace(rawName)

	cfg, _, err := LoadConfig()
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}

	if rawName == "default" {
		cfg.ActiveProfile = ""
		if err := SaveConfig(cfg); err != nil {
			return fmt.Errorf("saving config: %w", err)
		}
		fmt.Println("Switched to default (no profile).")
		return nil
	}

	name, err := validateProfileName(rawName)
	if err != nil {
		return err
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
		return fmt.Errorf("saving config: %w", err)
	}

	if !inList && !alreadyOnDisk {
		fmt.Printf("Created and switched to profile %q.\n", name)
	} else {
		fmt.Printf("Switched to profile %q.\n", name)
	}
	return nil
}

func runProfileDelete(rawName string) error {
	name, err := validateProfileName(rawName)
	if err != nil {
		return err
	}

	cfg, _, err := LoadConfig()
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}
	if cfg.ActiveProfile == name {
		return fmt.Errorf("can't delete the active profile -- switch away first (liber --profile default, or liber --profile <other>)")
	}

	idx := -1
	for i, p := range cfg.Profiles {
		if p == name {
			idx = i
			break
		}
	}
	if idx == -1 {
		return fmt.Errorf("no profile named %q (see `liber --profile`)", name)
	}
	cfg.Profiles = append(cfg.Profiles[:idx], cfg.Profiles[idx+1:]...)

	if err := SaveConfig(cfg); err != nil {
		return fmt.Errorf("saving config: %w", err)
	}
	fmt.Printf("Removed profile %q from the list. Its folder and bookmarks are untouched on disk.\n", name)
	return nil
}
