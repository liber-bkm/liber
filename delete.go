package main

import "fmt"

func runDelete(ids []int, rest []string) error {
	yes := false
	for _, a := range rest {
		switch a {
		case "-y", "--yes":
			yes = true
		default:
			return fmt.Errorf("unknown flag for -d: %s", a)
		}
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	var targets []*Bookmark
	var missing []int
	for _, id := range ids {
		b := store.Find(id)
		if b == nil {
			missing = append(missing, id)
			continue
		}
		targets = append(targets, b)
	}
	if len(targets) == 0 {
		return fmt.Errorf("no matching bookmarks found (see `liber -l`)")
	}

	if len(targets) == 1 {
		b := targets[0]
		if !yes && !confirm(fmt.Sprintf("Delete [%d] %s?", b.ID, b.Title), false) {
			fmt.Println("Cancelled.")
			return nil
		}
	} else {
		fmt.Printf("About to delete %d bookmarks:\n", len(targets))
		for _, b := range targets {
			fmt.Printf("  [%d] %s\n", b.ID, b.Title)
		}
		if !yes && !confirm(fmt.Sprintf("Delete all %d?", len(targets)), false) {
			fmt.Println("Cancelled.")
			return nil
		}
	}

	entry := journalDeletes(targets)
	for _, b := range targets {
		deleteBookmarkFiles(cfg, b)
		store.Delete(b.ID)
	}
	if err := saveWithJournal(cfg, store, entry); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}

	if len(targets) == 1 {
		fmt.Printf("Deleted [%d] %s\n", targets[0].ID, targets[0].Title)
	} else {
		fmt.Printf("Deleted %d bookmark(s).\n", len(targets))
	}
	if len(missing) > 0 {
		fmt.Printf("No bookmark with id(s): %s\n", joinInts(missing))
	}
	return nil
}
