package main

import (
	"fmt"
	"sort"
	"strings"
	"time"
)

func indexOfFold(list []string, target string) int {
	for i, s := range list {
		if strings.EqualFold(s, target) {
			return i
		}
	}
	return -1
}

func printCounts(counts map[string]int, emptyMsg string) {
	if len(counts) == 0 {
		fmt.Println(emptyMsg)
		return
	}
	type entry struct {
		label string
		n     int
	}
	var list []entry
	for label, n := range counts {
		list = append(list, entry{label, n})
	}
	sort.Slice(list, func(i, j int) bool {
		if list[i].n != list[j].n {
			return list[i].n > list[j].n
		}
		return list[i].label < list[j].label
	})
	for _, e := range list {
		fmt.Printf("%-30s %d\n", e.label, e.n)
	}
}

func tagCounts(store *Store) map[string]int {
	counts := map[string]int{}
	for _, b := range store.Bookmarks {
		for _, t := range b.Tags {
			counts[t]++
		}
	}
	return counts
}

func folderCounts(store *Store) map[string]int {
	counts := map[string]int{}
	for _, b := range store.Bookmarks {
		counts[displayFolder(b.Folder)]++
	}
	return counts
}

func runTagsList() error {
	_, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	printCounts(tagCounts(store), "No tags yet.")
	return nil
}

func renameTag(cfg Config, store *Store, old, newTag string) (int, error) {
	old = strings.TrimSpace(old)
	newTag = strings.TrimSpace(newTag)
	if old == "" || newTag == "" {
		return 0, fmt.Errorf("usage: liber --tags rename <old> <new>")
	}
	if strings.EqualFold(old, newTag) {
		return 0, fmt.Errorf("%q and %q are the same tag", old, newTag)
	}
	changed := 0
	for _, b := range store.Bookmarks {
		idx := indexOfFold(b.Tags, old)
		if idx == -1 {
			continue
		}
		b.Tags = append(append([]string{}, b.Tags[:idx]...), b.Tags[idx+1:]...)
		b.Tags = dedupe(append(b.Tags, newTag))
		b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, b, false)
		changed++
	}
	return changed, nil
}

func runTagsRename(old, newTag string) error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	changed, err := renameTag(cfg, store, old, newTag)
	if err != nil {
		return err
	}
	if changed == 0 {
		fmt.Printf("No bookmarks have the tag %q.\n", old)
		return nil
	}
	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}
	fmt.Printf("Renamed tag %q to %q on %d bookmark(s).\n", old, newTag, changed)
	return nil
}

func deleteTag(cfg Config, store *Store, tag string) (int, error) {
	tag = strings.TrimSpace(tag)
	if tag == "" {
		return 0, fmt.Errorf("usage: liber --tags delete <tag>")
	}
	changed := 0
	for _, b := range store.Bookmarks {
		idx := indexOfFold(b.Tags, tag)
		if idx == -1 {
			continue
		}
		b.Tags = append(append([]string{}, b.Tags[:idx]...), b.Tags[idx+1:]...)
		b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, b, false)
		changed++
	}
	return changed, nil
}

func runTagsDelete(tag string) error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	changed, err := deleteTag(cfg, store, tag)
	if err != nil {
		return err
	}
	if changed == 0 {
		fmt.Printf("No bookmarks have the tag %q.\n", tag)
		return nil
	}
	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}
	fmt.Printf("Removed tag %q from %d bookmark(s).\n", tag, changed)
	return nil
}

func runFoldersList() error {
	_, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	printCounts(folderCounts(store), "No folders yet -- everything's at the root.")
	return nil
}

func folderMatchesOrIsChild(folder, target string) bool {
	return folder == target || strings.HasPrefix(folder, target+"/")
}

func renameFolderPrefix(folder, oldPrefix, newPrefix string) string {
	if folder == oldPrefix {
		return newPrefix
	}
	return newPrefix + folder[len(oldPrefix):]
}

func renameFolder(cfg Config, store *Store, old, newFolder string) (int, error) {
	old = sanitizeFolder(old)
	newFolder = sanitizeFolder(newFolder)
	if old == "" {
		return 0, fmt.Errorf("old folder can't be root -- did you mean a specific subfolder?")
	}
	if old == newFolder {
		return 0, fmt.Errorf("%q and %q are the same folder", displayFolder(old), displayFolder(newFolder))
	}
	changed := 0
	for _, b := range store.Bookmarks {
		if !folderMatchesOrIsChild(b.Folder, old) {
			continue
		}
		b.Folder = sanitizeFolder(renameFolderPrefix(b.Folder, old, newFolder))
		b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, b, true)
		changed++
	}
	return changed, nil
}

func runFoldersRename(old, newFolder string) error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	changed, err := renameFolder(cfg, store, old, newFolder)
	if err != nil {
		return err
	}
	if changed == 0 {
		fmt.Printf("No bookmarks are in folder %q.\n", old)
		return nil
	}
	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}
	fmt.Printf("Moved %d bookmark(s) from %q to %q.\n", changed, old, displayFolder(newFolder))
	return nil
}

func runFoldersDelete(folder string) error {
	return runFoldersRename(folder, "")
}
