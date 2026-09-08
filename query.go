package main

import (
	"fmt"
	"io"
)

func searchTargets(store *Store, cfg Config, q string) []*Bookmark {
	return store.Search(cfg, q, SearchFields{}, false, SortRelevance)
}

func capShown(results []*Bookmark, max int, w io.Writer) []*Bookmark {
	if len(results) > max {
		fmt.Fprintf(w, "%d matches -- showing first %d:\n", len(results), max)
		return results[:max]
	}
	return results
}

func pickFzfTarget(store *Store, shown []*Bookmark) (*Bookmark, bool, error) {
	id, ok, err := pickWithFzf(shown, SearchFields{})
	if err != nil || !ok {
		return nil, ok, err
	}
	return store.Find(id), true, nil
}
