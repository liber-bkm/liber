package main

import (
	"fmt"
	"sort"
)

func historyRows(store *Store) []*Bookmark {
	var opened []*Bookmark
	for _, b := range store.Bookmarks {
		if b.LastOpenedAt != nil {
			opened = append(opened, b)
		}
	}
	sort.Slice(opened, func(i, j int) bool {
		return opened[i].LastOpenedAt.After(*opened[j].LastOpenedAt)
	})
	return opened
}

func runHistory() error {
	_, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	opened := historyRows(store)
	if len(opened) == 0 {
		fmt.Println("No open history yet -- using the (o) action in `liber -s`/`liber -sl` records it here.")
		return nil
	}

	for _, b := range opened {
		times := "time"
		if b.OpenCount != 1 {
			times = "times"
		}
		fmt.Printf("[%d] %s%s\n     opened %s \u00b7 %d %s\n", b.ID, b.Title, badgeSuffix(b), b.LastOpenedAt.Format("Jan 2, 2006 15:04"), b.OpenCount, times)
	}
	return nil
}
