package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

type Bookmark struct {
	ID          int       `json:"id"`
	URL         string    `json:"url"`
	Title       string    `json:"title"`
	Description string    `json:"description,omitempty"`
	Tags        []string  `json:"tags,omitempty"`
	Folder      string    `json:"folder,omitempty"`
	CreatedAt   time.Time `json:"created_at"`
	UpdatedAt   time.Time `json:"updated_at"`

	HTMLFile     string `json:"html_file,omitempty"`
	MarkdownFile string `json:"markdown_file,omitempty"`
	ArchiveFile  string `json:"archive_file,omitempty"`

	LastOpenedAt *time.Time `json:"last_opened_at,omitempty"`
	OpenCount    int        `json:"open_count,omitempty"`

	AppliedRules []AppliedAutoRule `json:"applied_rules,omitempty"`

	Attachments []Attachment `json:"attachments,omitempty"`
}

type Attachment struct {
	Name string `json:"name"`
	File string `json:"file"`
}

type AppliedAutoRule struct {
	RuleID int    `json:"rule_id"`
	Folder string `json:"folder,omitempty"`
}

type AutoRule struct {
	ID        int       `json:"id"`
	Match     string    `json:"match"`
	Folder    string    `json:"folder,omitempty"`
	Tags      []string  `json:"tags,omitempty"`
	CreatedAt time.Time `json:"created_at"`
}

type Store struct {
	NextID    int         `json:"next_id"`
	Bookmarks []*Bookmark `json:"bookmarks"`

	NextAutoRuleID int         `json:"next_auto_rule_id"`
	AutoRules      []*AutoRule `json:"auto_rules,omitempty"`

	path string // not persisted
}

func LoadStore(path string) (*Store, error) {
	s := &Store{NextID: 1, NextAutoRuleID: 1, path: path}
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return s, nil
	} else if err != nil {
		return nil, err
	}
	if len(strings.TrimSpace(string(data))) == 0 {
		return s, nil
	}
	if err := json.Unmarshal(data, s); err != nil {
		return nil, err
	}
	s.path = path
	if s.NextID == 0 {
		s.NextID = 1
	}
	if s.NextAutoRuleID == 0 {
		s.NextAutoRuleID = 1
	}
	return s, nil
}

func (s *Store) Save() error {
	if err := os.MkdirAll(filepath.Dir(s.path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	tmp := s.path + ".tmp"
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		return err
	}
	return os.Rename(tmp, s.path)
}

func (s *Store) Add(b *Bookmark) {
	b.ID = s.NextID
	s.NextID++
	s.Bookmarks = append(s.Bookmarks, b)
}

func (s *Store) Find(id int) *Bookmark {
	for _, b := range s.Bookmarks {
		if b.ID == id {
			return b
		}
	}
	return nil
}

func (s *Store) Delete(id int) bool {
	for i, b := range s.Bookmarks {
		if b.ID == id {
			s.Bookmarks = append(s.Bookmarks[:i], s.Bookmarks[i+1:]...)
			return true
		}
	}
	return false
}

func (s *Store) AddAutoRule(r *AutoRule) {
	r.ID = s.NextAutoRuleID
	s.NextAutoRuleID++
	s.AutoRules = append(s.AutoRules, r)
}

func (s *Store) FindAutoRule(id int) *AutoRule {
	for _, r := range s.AutoRules {
		if r.ID == id {
			return r
		}
	}
	return nil
}

func (s *Store) DeleteAutoRule(id int) bool {
	for i, r := range s.AutoRules {
		if r.ID == id {
			s.AutoRules = append(s.AutoRules[:i], s.AutoRules[i+1:]...)
			return true
		}
	}
	return false
}

type SearchFields struct {
	Title       bool
	URL         bool
	Tags        bool
	Folder      bool
	Description bool
}

func (f SearchFields) Any() bool {
	return f.Title || f.URL || f.Tags || f.Folder || f.Description
}

func (f SearchFields) Label() string {
	if !f.Any() {
		return "title \u00b7 url \u00b7 tags \u00b7 folder \u00b7 description"
	}
	var parts []string
	if f.Title {
		parts = append(parts, "title")
	}
	if f.URL {
		parts = append(parts, "url")
	}
	if f.Tags {
		parts = append(parts, "tags")
	}
	if f.Folder {
		parts = append(parts, "folder")
	}
	if f.Description {
		parts = append(parts, "description")
	}
	return strings.Join(parts, " \u00b7 ")
}

func (s *Store) Search(cfg Config, query string, fields SearchFields, deep bool) []*Bookmark {
	query = strings.ToLower(strings.TrimSpace(query))
	var results []*Bookmark
	for _, b := range s.Bookmarks {
		if query == "" || bookmarkMatches(b, query, fields) {
			results = append(results, b)
			continue
		}
		if deep && b.ArchiveFile != "" {
			text, err := extractArchiveText(filepath.Join(cfg.archiveDir(), b.ArchiveFile))
			if err == nil && strings.Contains(strings.ToLower(text), query) {
				results = append(results, b)
			}
		}
	}
	sort.Slice(results, func(i, j int) bool { return results[i].ID < results[j].ID })
	return results
}

func (s *Store) All() []*Bookmark {
	out := append([]*Bookmark{}, s.Bookmarks...)
	sort.Slice(out, func(i, j int) bool { return out[i].ID < out[j].ID })
	return out
}

func loadCfgAndStore() (Config, *Store, error) {
	cfg, _, err := LoadConfig()
	if err != nil {
		return Config{}, nil, fmt.Errorf("loading config: %w", err)
	}
	store, err := LoadStore(cfg.indexPath())
	if err != nil {
		return Config{}, nil, fmt.Errorf("loading index: %w", err)
	}
	return cfg, store, nil
}

func bookmarkMatches(b *Bookmark, q string, fields SearchFields) bool {
	all := !fields.Any()
	if (all || fields.Title) && strings.Contains(strings.ToLower(b.Title), q) {
		return true
	}
	if (all || fields.URL) && strings.Contains(strings.ToLower(b.URL), q) {
		return true
	}
	if (all || fields.Folder) && strings.Contains(strings.ToLower(b.Folder), q) {
		return true
	}
	if (all || fields.Description) && strings.Contains(strings.ToLower(b.Description), q) {
		return true
	}
	if all || fields.Tags {
		for _, t := range b.Tags {
			if strings.Contains(strings.ToLower(t), q) {
				return true
			}
		}
	}
	return false
}
