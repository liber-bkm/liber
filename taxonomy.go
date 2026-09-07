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

func runTagsList() error {
	_, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	counts := map[string]int{}
	for _, b := range store.Bookmarks {
		for _, t := range b.Tags {
			counts[t]++
		}
	}
	printCounts(counts, "No tags yet.")
	return nil
}

func runTagsRename(old, newTag string) error {
	old = strings.TrimSpace(old)
	newTag = strings.TrimSpace(newTag)
	if old == "" || newTag == "" {
		return fmt.Errorf("usage: liber --tags rename <old> <new>")
	}
	if strings.EqualFold(old, newTag) {
		return fmt.Errorf("%q and %q are the same tag", old, newTag)
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
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

func runTagsDelete(tag string) error {
	tag = strings.TrimSpace(tag)
	if tag == "" {
		return fmt.Errorf("usage: liber --tags delete <tag>")
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
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
	counts := map[string]int{}
	for _, b := range store.Bookmarks {
		counts[displayFolder(b.Folder)]++
	}
	printCounts(counts, "No folders yet -- everything's at the root.")
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

func runFoldersRename(old, newFolder string) error {
	old = sanitizeFolder(old)
	newFolder = sanitizeFolder(newFolder)
	if old == "" {
		return fmt.Errorf("old folder can't be root -- did you mean a specific subfolder?")
	}
	if old == newFolder {
		return fmt.Errorf("%q and %q are the same folder", displayFolder(old), displayFolder(newFolder))
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
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
