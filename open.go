package main

import (
	"fmt"
	"time"
)

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
		if err := openURL(cfg, b.URL); err != nil {
			fmt.Printf("[%d] %s -- could not open: %v\n", b.ID, b.Title, err)
			continue
		}
		now := time.Now()
		b.LastOpenedAt = &now
		b.OpenCount++
		opened++
		fmt.Printf("Opened [%d] %s\n", b.ID, b.Title)
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
