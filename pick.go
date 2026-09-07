package main

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"
)

func runPick(args []string) error {
	if len(args) == 0 {
		return fmt.Errorf("usage: liber pick <query>")
	}
	q := strings.Join(args, " ")
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	results := store.Search(cfg, q, SearchFields{}, false)
	if len(results) == 0 {
		return fmt.Errorf("no bookmarks matching %q", q)
	}
	if len(results) == 1 {
		fmt.Println(results[0].URL)
		return nil
	}
	shown := results
	if len(shown) > 30 {
		fmt.Fprintf(os.Stderr, "%d matches -- showing first 30:\n", len(results))
		shown = results[:30]
	}
	if fzfAvailable() {
		id, ok, ferr := pickWithFzf(shown, SearchFields{})
		if ferr == nil {
			if !ok {
				return fmt.Errorf("no bookmark picked")
			}
			if b := store.Find(id); b != nil {
				fmt.Println(b.URL)
				return nil
			}
		}
		fmt.Fprintf(os.Stderr, "(fzf picker failed -- falling back to plain prompt)\n")
	}
	for _, b := range shown {
		fmt.Fprintf(os.Stderr, "[%d] %s\n    %s\n", b.ID, b.Title, b.URL)
	}
	fmt.Fprint(os.Stderr, "number to pick: ")
	line, rerr := bufio.NewReader(os.Stdin).ReadString('\n')
	if rerr != nil {
		return fmt.Errorf("no bookmark picked")
	}
	n, cerr := strconv.Atoi(strings.TrimSpace(line))
	if cerr != nil {
		return fmt.Errorf("no bookmark picked")
	}
	for _, b := range shown {
		if b.ID == n {
			fmt.Println(b.URL)
			return nil
		}
	}
	return fmt.Errorf("no bookmark picked")
}
