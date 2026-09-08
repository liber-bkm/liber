package main

import "testing"

func taxonomyStore() (*Store, Config) {
	cfg := Config{BaseDir: "/tmp/taxonomy-test-unused"}
	s := &Store{NextID: 1}
	add := func(tags []string, folder string) {
		s.Bookmarks = append(s.Bookmarks, &Bookmark{ID: s.NextID, Tags: tags, Folder: folder})
		s.NextID++
	}
	add([]string{"a", "b"}, "work")
	add([]string{"b", "c"}, "work/urgent")
	add([]string{"c"}, "")
	return s, cfg
}

func TestRenameTag(t *testing.T) {
	s, cfg := taxonomyStore()
	n, err := renameTag(cfg, s, "b", "d")
	if err != nil || n != 2 {
		t.Fatalf("renameTag = %d %v", n, err)
	}
	if got := tagCounts(s); got["d"] != 2 || got["b"] != 0 {
		t.Fatalf("counts = %v", got)
	}
}

func TestRenameTagMerge(t *testing.T) {
	s, cfg := taxonomyStore()
	if _, err := renameTag(cfg, s, "a", "b"); err != nil {
		t.Fatal(err)
	}
	for _, b := range s.Bookmarks {
		seen := map[string]bool{}
		for _, tag := range b.Tags {
			if seen[tag] {
				t.Fatalf("duplicate tag %q in %v", tag, b.Tags)
			}
			seen[tag] = true
		}
	}
}

func TestRenameTagErrors(t *testing.T) {
	s, cfg := taxonomyStore()
	if _, err := renameTag(cfg, s, "a", "A"); err == nil {
		t.Error("same tag should fail")
	}
	if _, err := renameTag(cfg, s, "", "x"); err == nil {
		t.Error("empty tag should fail")
	}
	if n, err := renameTag(cfg, s, "missing", "x"); err != nil || n != 0 {
		t.Errorf("missing tag = %d %v", n, err)
	}
}

func TestDeleteTag(t *testing.T) {
	s, cfg := taxonomyStore()
	n, err := deleteTag(cfg, s, "c")
	if err != nil || n != 2 {
		t.Fatalf("deleteTag = %d %v", n, err)
	}
	if got := tagCounts(s); got["c"] != 0 {
		t.Fatalf("counts = %v", got)
	}
}

func TestRenameFolder(t *testing.T) {
	s, cfg := taxonomyStore()
	n, err := renameFolder(cfg, s, "work", "play")
	if err != nil || n != 2 {
		t.Fatalf("renameFolder = %d %v", n, err)
	}
	if got := folderCounts(s); got["play"] != 1 || got["play/urgent"] != 1 {
		t.Fatalf("counts = %v", got)
	}
	if _, err := renameFolder(cfg, s, "work", "work"); err == nil {
		t.Error("same folder should fail")
	}
	n, err = renameFolder(cfg, s, "play", "")
	if err != nil || n != 2 {
		t.Fatalf("delete folder = %d %v", n, err)
	}
	if got := folderCounts(s); got["/"] != 2 || got["urgent"] != 1 {
		t.Fatalf("counts = %v", got)
	}
}
