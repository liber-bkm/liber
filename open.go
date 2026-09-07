package main

import (
	"fmt"
	"strconv"
	"strings"
	"time"
)

func markOpened(cfg Config, b *Bookmark) bool {
	if err := openURL(cfg, b.URL); err != nil {
		fmt.Printf("[%d] %s -- could not open: %v\n", b.ID, b.Title, err)
		return false
	}
	now := time.Now()
	b.LastOpenedAt = &now
	b.OpenCount++
	fmt.Printf("Opened [%d] %s\n", b.ID, b.Title)
	return true
}

func runOpen(ids []int) error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	var missing []int
	opened := 0
	for _, id := range ids {
		b := store.Find(id)
		if b == nil {
			missing = append(missing, id)
			continue
		}
		if markOpened(cfg, b) {
			opened++
		}
	}

	if opened > 0 {
		if err := store.Save(); err != nil {
			fmt.Println("Could not save index:", err)
		}
	}
	if len(missing) > 0 {
		fmt.Printf("No bookmark with id(s): %s\n", joinInts(missing))
	}
	if opened == 0 {
		return fmt.Errorf("no matching bookmarks found (see `liber -l`)")
	}
	return nil
}

func isQuerySpec(spec string) bool {
	for _, c := range spec {
		if !(c >= '0' && c <= '9' || c == ',' || c == '-' || c == ' ') {
			return true
		}
	}
	return false
}

func runOpenQuery(q string) error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	results := store.Search(cfg, q, SearchFields{}, false)
	if len(results) == 0 {
		return fmt.Errorf("no bookmarks matching %q", q)
	}
	var b *Bookmark
	switch {
	case len(results) == 1:
		b = results[0]
	case fzfAvailable():
		shown := capResults(results)
		id, ok, ferr := pickWithFzf(shown, SearchFields{})
		if ferr != nil {
			fmt.Printf("(fzf picker failed: %v -- falling back to plain prompt)\n", ferr)
			b, ok = promptOpenPick(shown)
			if !ok {
				return nil
			}
		} else if !ok {
			return nil
		} else {
			b = store.Find(id)
		}
	default:
		picked, ok := promptOpenPick(capResults(results))
		if !ok {
			return nil
		}
		b = picked
	}
	if b == nil {
		return fmt.Errorf("no bookmarks matching %q", q)
	}
	if markOpened(cfg, b) {
		if err := store.Save(); err != nil {
			fmt.Println("Could not save index:", err)
		}
		return nil
	}
	return fmt.Errorf("could not open [%d] %s", b.ID, b.Title)
}

func capResults(results []*Bookmark) []*Bookmark {
	if len(results) > 30 {
		fmt.Printf("%d matches -- showing first 30:\n", len(results))
		return results[:30]
	}
	return results
}

func promptOpenPick(shown []*Bookmark) (*Bookmark, bool) {
	printResults(shown)
	sel := promptLine("number to open ('q' to quit)")
	if sel == "q" || sel == "" {
		return nil, false
	}
	n, err := strconv.Atoi(strings.TrimSpace(sel))
	if err != nil {
		fmt.Println("Not a valid number.")
		return nil, false
	}
	for _, b := range shown {
		if b.ID == n {
			return b, true
		}
	}
	fmt.Println("No match with that number.")
	return nil, false
}
