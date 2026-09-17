package main

import (
	"fmt"
	"net/http"
	"os"
	"sort"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

const checkUA = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36"

type checkStatus int

const (
	checkOK checkStatus = iota
	checkDead
	checkMoved
	checkUncertain
)

type checkResult struct {
	b      *Bookmark
	status checkStatus
	detail string
	target string
}

type checkAction int

const (
	actionSkip checkAction = iota
	actionDelete
	actionQuarantine
)

const quarantineFolder = "quarantine"

func promptCheckAction(label string) checkAction {
	fmt.Printf("%s [y/q/N]: ", label)
	line, err := stdinReader.ReadString('\n')
	exitOnEOF(err)
	switch strings.ToLower(strings.TrimSpace(line)) {
	case "y", "yes":
		return actionDelete
	case "q", "quarantine":
		return actionQuarantine
	}
	return actionSkip
}

func quarantineBookmark(cfg Config, b *Bookmark) {
	if b.Folder == quarantineFolder {
		fmt.Printf("[%d] already quarantined.\n", b.ID)
		return
	}
	b.Folder = quarantineFolder
	b.UpdatedAt = time.Now()
	syncBookmarkFiles(cfg, b, true)
	fmt.Printf("Quarantined [%d].\n", b.ID)
}

func parseCheckArgs(args []string) (spec string, workers int, stale time.Duration, rest []string, err error) {
	workers = 12
	spec, rest = consumeIDSpec(args)
	var kept []string
	for i := 0; i < len(rest); i++ {
		switch rest[i] {
		case "--workers":
			if i+1 >= len(rest) {
				return "", 0, 0, nil, fmt.Errorf("--workers requires a number")
			}
			n, convErr := strconv.Atoi(rest[i+1])
			if convErr != nil || n < 1 {
				return "", 0, 0, nil, fmt.Errorf("--workers requires a positive number")
			}
			workers = n
			i++
		case "--stale":
			if i+1 >= len(rest) {
				return "", 0, 0, nil, fmt.Errorf("--stale requires a duration, e.g. 720h")
			}
			d, convErr := time.ParseDuration(rest[i+1])
			if convErr != nil || d <= 0 {
				return "", 0, 0, nil, fmt.Errorf("--stale requires a positive duration, e.g. 720h")
			}
			stale = d
			i++
		default:
			kept = append(kept, rest[i])
		}
	}
	if len(kept) > 0 {
		return "", 0, 0, nil, fmt.Errorf("unknown flag %q (usage: liber --check [ids] [--workers N] [--stale D])", kept[0])
	}
	return spec, workers, stale, nil, nil
}

func resolveCheckTargets(store *Store, spec string, stale time.Duration) (targets []*Bookmark, missing []int, fresh int, err error) {
	if strings.TrimSpace(spec) == "" {
		targets = store.All()
	} else {
		var ids []int
		ids, err = parseIDSpec(spec)
		if err != nil {
			return nil, nil, 0, fmt.Errorf("%w (use `liber -l` to see valid ids)", err)
		}
		for _, id := range ids {
			b := store.Find(id)
			if b == nil {
				missing = append(missing, id)
				continue
			}
			targets = append(targets, b)
		}
	}
	if stale > 0 {
		cutoff := time.Now().Add(-stale)
		var kept []*Bookmark
		for _, b := range targets {
			if !b.LastCheckedAt.IsZero() && b.LastCheckedAt.After(cutoff) {
				fresh++
				continue
			}
			kept = append(kept, b)
		}
		targets = kept
	}
	return targets, missing, fresh, nil
}

func targetIDs(targets []*Bookmark) []int {
	ids := make([]int, 0, len(targets))
	for _, b := range targets {
		ids = append(ids, b.ID)
	}
	return ids
}

func scanCheckTargets(client *http.Client, targets []*Bookmark, workers int, progress func(done, total int)) (moved, dead, uncertain []checkResult) {
	results := make([]checkResult, len(targets))
	var done int64
	var wg sync.WaitGroup
	sem := make(chan struct{}, workers)
	for i, b := range targets {
		wg.Add(1)
		go func(i int, b *Bookmark) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()
			r := classifyURL(client, b.URL)
			r.b = b
			results[i] = r
			if progress != nil {
				progress(int(atomic.AddInt64(&done, 1)), len(targets))
			}
		}(i, b)
	}
	wg.Wait()

	for _, r := range results {
		switch r.status {
		case checkMoved:
			moved = append(moved, r)
		case checkDead:
			dead = append(dead, r)
		case checkUncertain:
			uncertain = append(uncertain, r)
		}
	}
	sort.Slice(moved, func(i, j int) bool { return moved[i].b.ID < moved[j].b.ID })
	sort.Slice(dead, func(i, j int) bool { return dead[i].b.ID < dead[j].b.ID })
	sort.Slice(uncertain, func(i, j int) bool { return uncertain[i].b.ID < uncertain[j].b.ID })
	return moved, dead, uncertain
}

func stampCheckTargets(store *Store, ids []int, moved, dead, uncertain []checkResult, now time.Time) {
	status := map[int]string{}
	for _, r := range moved {
		status[r.b.ID] = "moved"
	}
	for _, r := range dead {
		status[r.b.ID] = "dead"
	}
	for _, r := range uncertain {
		status[r.b.ID] = "uncertain"
	}
	for _, id := range ids {
		b := store.Find(id)
		if b == nil {
			continue
		}
		b.LastCheckedAt = now
		if s, ok := status[id]; ok {
			b.LastCheckStatus = s
		} else {
			b.LastCheckStatus = "ok"
		}
	}
}

func checkClient() *http.Client {
	return &http.Client{
		Timeout: 15 * time.Second,
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return http.ErrUseLastResponse
		},
	}
}

func checkOnce(client *http.Client, method, url string) (*http.Response, error) {
	req, err := http.NewRequest(method, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("User-Agent", checkUA)
	req.Header.Set("Accept", "text/html,*/*")
	return client.Do(req)
}

// classifyURL follows redirects manually so permanent moves stay distinct from notice pages.
func classifyURL(client *http.Client, rawURL string) checkResult {
	res := checkResult{}
	url := rawURL
	var lastResp *http.Response
	for hops := 0; hops < 6; hops++ {
		resp, err := checkOnce(client, "HEAD", url)
		if err != nil {
			return checkResult{status: checkUncertain, detail: shortErr(err)}
		}
		lastResp = resp
		resp.Body.Close()
		code := resp.StatusCode
		if code == 404 || code == 410 {
			return checkResult{status: checkDead, detail: resp.Status}
		}
		if code == 301 || code == 308 {
			loc := resp.Header.Get("Location")
			if loc == "" {
				return checkResult{status: checkUncertain, detail: resp.Status + " (no location)"}
			}
			next := resolveRef(url, loc)
			if sameHost(url, next) || hops > 0 {
				res = checkResult{status: checkMoved, detail: resp.Status, target: next}
				return res
			}
			return checkResult{status: checkMoved, detail: resp.Status, target: next}
		}
		if code >= 300 && code < 400 {
			loc := resp.Header.Get("Location")
			if loc == "" {
				return checkResult{status: checkUncertain, detail: resp.Status + " (no location)"}
			}
			next := resolveRef(url, loc)
			if !sameHost(rawURL, next) {
				return checkResult{status: checkUncertain, detail: fmt.Sprintf("%s to different host (possible block page)", resp.Status)}
			}
			url = next
			continue
		}
		if code == 405 || code == 501 {
			break
		}
		if code >= 200 && code < 300 {
			return checkResult{}
		}
		return checkResult{status: checkUncertain, detail: resp.Status}
	}
	_ = lastResp
	resp, err := checkOnce(client, "GET", url)
	if err != nil {
		return checkResult{status: checkUncertain, detail: shortErr(err)}
	}
	defer resp.Body.Close()
	code := resp.StatusCode
	if code == 404 || code == 410 {
		return checkResult{status: checkDead, detail: resp.Status}
	}
	if code == 301 || code == 308 {
		loc := resp.Header.Get("Location")
		if loc == "" {
			return checkResult{status: checkUncertain, detail: resp.Status + " (no location)"}
		}
		return checkResult{status: checkMoved, detail: resp.Status, target: resolveRef(url, loc)}
	}
	if code >= 200 && code < 300 {
		return checkResult{}
	}
	return checkResult{status: checkUncertain, detail: resp.Status}
}

func resolveRef(base, loc string) string {
	if strings.HasPrefix(loc, "http://") || strings.HasPrefix(loc, "https://") {
		return loc
	}
	if strings.HasPrefix(loc, "/") {
		scheme := "https"
		rest := base
		if i := strings.Index(base, "://"); i != -1 {
			scheme = base[:i]
			rest = base[i+3:]
		}
		host := rest
		if i := strings.IndexAny(host, "/?#"); i != -1 {
			host = host[:i]
		}
		return scheme + "://" + host + loc
	}
	return loc
}

func sameHost(a, b string) bool {
	return hostOf(a) != "" && hostOf(a) == hostOf(b)
}

func shortErr(err error) string {
	s := err.Error()
	if i := strings.Index(s, ": "); i != -1 {
		tail := s[i+2:]
		if len(tail) < len(s) {
			s = tail
		}
	}
	s = strings.Join(strings.Fields(s), " ")
	if len(s) > 120 {
		s = s[:120]
	}
	return s
}

func runCheck(args []string) error {
	spec, workers, stale, _, err := parseCheckArgs(args)
	if err != nil {
		return err
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	targets, missing, fresh, err := resolveCheckTargets(store, spec, stale)
	if err != nil {
		return err
	}
	if len(missing) > 0 {
		fmt.Printf("No bookmark with id(s): %s\n", joinInts(missing))
	}
	if fresh > 0 {
		fmt.Printf("Skipped %d freshly checked bookmark(s).\n", fresh)
	}
	if len(targets) == 0 {
		fmt.Println("No bookmarks to check.")
		return nil
	}

	client := checkClient()
	moved, dead, uncertain := scanCheckTargets(client, targets, workers, func(done, total int) {
		fmt.Fprintf(os.Stderr, "\rChecking %d/%d...", done, total)
	})
	fmt.Fprintln(os.Stderr)
	now := time.Now()
	stampCheckTargets(store, targetIDs(targets), moved, dead, uncertain, now)

	ok := len(targets) - len(moved) - len(dead) - len(uncertain)
	fmt.Printf("%d ok, %d moved, %d dead, %d uncertain (of %d checked)\n", ok, len(moved), len(dead), len(uncertain), len(targets))
	for _, r := range moved {
		fmt.Printf("[%d] %s\n    moved -> %s (%s)\n", r.b.ID, r.b.Title, r.target, r.detail)
	}
	for _, r := range dead {
		fmt.Printf("[%d] %s\n    dead (%s)\n", r.b.ID, r.b.Title, r.detail)
	}
	for _, r := range uncertain {
		fmt.Printf("[%d] %s\n    uncertain (%s)\n", r.b.ID, r.b.Title, r.detail)
	}
	if len(moved)+len(dead)+len(uncertain) == 0 {
		if err := store.Save(); err != nil {
			return fmt.Errorf("saving index: %w", err)
		}
		return nil
	}

	updated, deleted, quarantined, skipped := 0, 0, 0, 0
	jent := &JournalEntry{}
	for _, r := range moved {
		if !confirm(fmt.Sprintf("Update [%d] URL to %s?", r.b.ID, r.target), true) {
			skipped++
			continue
		}
		r.b.URL = normalizeURL(r.target)
		r.b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, r.b, false)
		updated++
		fmt.Printf("Updated [%d].\n", r.b.ID)
		if title := fetchTitle(r.b.URL); title != "" && title != r.b.Title {
			if confirm(fmt.Sprintf("Update [%d] title to %q?", r.b.ID, title), true) {
				r.b.Title = title
				r.b.UpdatedAt = time.Now()
				syncBookmarkFiles(cfg, r.b, false)
				fmt.Printf("Retitled [%d].\n", r.b.ID)
			}
		}
		jent = jent.merge(journalUpserts([]*Bookmark{r.b}))
	}
	for _, r := range dead {
		switch promptCheckAction(fmt.Sprintf("Delete dead [%d] %s?", r.b.ID, r.b.Title)) {
		case actionDelete:
			jent = jent.merge(journalDeletes([]*Bookmark{r.b}))
			deleteBookmarkFiles(cfg, r.b)
			store.Delete(r.b.ID)
			deleted++
			fmt.Println("Deleted.")
		case actionQuarantine:
			quarantineBookmark(cfg, r.b)
			jent = jent.merge(journalUpserts([]*Bookmark{r.b}))
			quarantined++
		default:
			skipped++
		}
	}
	for _, r := range uncertain {
		switch promptCheckAction(fmt.Sprintf("Delete uncertain [%d] %s (%s)?", r.b.ID, r.b.Title, r.detail)) {
		case actionDelete:
			jent = jent.merge(journalDeletes([]*Bookmark{r.b}))
			deleteBookmarkFiles(cfg, r.b)
			store.Delete(r.b.ID)
			deleted++
			fmt.Println("Deleted.")
		case actionQuarantine:
			quarantineBookmark(cfg, r.b)
			jent = jent.merge(journalUpserts([]*Bookmark{r.b}))
			quarantined++
		default:
			skipped++
		}
	}
	if err := saveWithJournal(cfg, store, jent); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}
	fmt.Printf("Done: %d updated, %d deleted, %d quarantined, %d skipped.\n", updated, deleted, quarantined, skipped)
	return nil
}
