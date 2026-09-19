package main

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

var benchWords = []string{
	"guide", "reference", "tutorial", "notes", "docs", "manual", "cookbook",
	"linux", "network", "server", "desktop", "kernel", "shell", "terminal",
	"python", "golang", "rust", "database", "index", "search", "sync",
	"backup", "config", "deploy", "monitor", "docker", "git", "editor",
	"recipe", "travel", "photo", "music", "book", "paper", "study",
}

var benchTags = []string{"reading", "work", "study", "important", "later", "code", "docs", "media"}
var benchFolders = []string{"", "work", "work/urgent", "personal", "personal/media", "study", "archive"}

func genBenchmarkStore(n int) *Store {
	rng := rand.New(rand.NewSource(42))
	now := time.Now()
	s := &Store{NextID: n + 1, NextAutoRuleID: 1}
	for i := 1; i <= n; i++ {
		title := fmt.Sprintf("%s %s %d", benchWords[rng.Intn(len(benchWords))], benchWords[rng.Intn(len(benchWords))], i)
		s.Bookmarks = append(s.Bookmarks, &Bookmark{
			ID:          i,
			URL:         fmt.Sprintf("https://site%d.example.com/page/%d", i%500, i),
			Title:       title,
			Description: fmt.Sprintf("notes on %s and %s", benchWords[rng.Intn(len(benchWords))], benchWords[rng.Intn(len(benchWords))]),
			Tags:        []string{benchTags[rng.Intn(len(benchTags))], benchTags[rng.Intn(len(benchTags))]},
			Folder:      benchFolders[rng.Intn(len(benchFolders))],
			CreatedAt:   now,
			UpdatedAt:   now,
			HTMLFile:    fmt.Sprintf("%04d-page-%d.html", i, i),
		})
	}
	return s
}

var benchStore10k = sync.OnceValue(func() *Store { return genBenchmarkStore(10000) })

var benchQueries = []string{"guide", "server", "site42", "study", "zz-no-match", "docs", "page"}

func BenchmarkLoad10k(b *testing.B) {
	data, err := json.Marshal(benchStore10k())
	if err != nil {
		b.Fatal(err)
	}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		var s Store
		if err := json.Unmarshal(data, &s); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkSave10k(b *testing.B) {
	s := benchStore10k()
	s.path = filepath.Join(b.TempDir(), ".liber", "index.json")
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if err := s.Save(); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkSearch10k(b *testing.B) {
	s := benchStore10k()
	cfg := Config{BaseDir: b.TempDir()}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		s.Search(cfg, benchQueries[i%len(benchQueries)], SearchFields{}, false, SortRelevance)
	}
}

func TestPerfBudgets(t *testing.T) {
	s := benchStore10k()

	data, err := json.Marshal(s)
	if err != nil {
		t.Fatal(err)
	}
	start := time.Now()
	var loaded Store
	if err := json.Unmarshal(data, &loaded); err != nil {
		t.Fatal(err)
	}
	loadDur := time.Since(start)
	t.Logf("load 10k: %v (%d bytes)", loadDur, len(data))
	if loadDur > 2*time.Second {
		t.Errorf("load 10k took %v, budget 2s", loadDur)
	}

	s.path = filepath.Join(t.TempDir(), ".liber", "index.json")
	start = time.Now()
	if err := s.Save(); err != nil {
		t.Fatal(err)
	}
	saveDur := time.Since(start)
	t.Logf("save 10k: %v", saveDur)
	if saveDur > 2*time.Second {
		t.Errorf("save 10k took %v, budget 2s", saveDur)
	}

	cfg := Config{BaseDir: t.TempDir()}
	start = time.Now()
	const nq = 20
	for i := 0; i < nq; i++ {
		s.Search(cfg, benchQueries[i%len(benchQueries)], SearchFields{}, false, SortRelevance)
	}
	avg := time.Since(start) / nq
	t.Logf("search 10k avg over %d queries: %v", nq, avg)
	if avg > 100*time.Millisecond {
		t.Errorf("search 10k avg took %v, budget 100ms", avg)
	}
}
