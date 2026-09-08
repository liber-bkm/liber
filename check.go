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
	var targets []*Bookmark
	if strings.TrimSpace(spec) == "" {
		targets = store.All()
	} else {
		ids, perr := parseIDSpec(spec)
		if perr != nil {
			return fmt.Errorf("%w (use `liber -l` to see valid ids)", perr)
		}
		var missing []int
		for _, id := range ids {
			b := store.Find(id)
			if b == nil {
				missing = append(missing, id)
				continue
			}
			targets = append(targets, b)
		}
		if len(missing) > 0 {
			fmt.Printf("No bookmark with id(s): %s\n", joinInts(missing))
		}
	}
	if stale > 0 {
		cutoff := time.Now().Add(-stale)
		fresh := 0
		kept := targets[:0]
		for _, b := range targets {
			if !b.LastCheckedAt.IsZero() && b.LastCheckedAt.After(cutoff) {
				fresh++
				continue
			}
			kept = append(kept, b)
		}
		targets = kept
		if fresh > 0 {
			fmt.Printf("Skipped %d freshly checked bookmark(s).\n", fresh)
		}
	}
	if len(targets) == 0 {
		fmt.Println("No bookmarks to check.")
		return nil
	}

	client := checkClient()
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
			n := atomic.AddInt64(&done, 1)
			fmt.Fprintf(os.Stderr, "\rChecking %d/%d...", n, len(targets))
		}(i, b)
	}
	wg.Wait()
	fmt.Fprintln(os.Stderr)

	var moved, dead, uncertain []checkResult
	now := time.Now()
	for _, r := range results {
		r.b.LastCheckedAt = now
		switch r.status {
		case checkMoved:
			r.b.LastCheckStatus = "moved"
			moved = append(moved, r)
		case checkDead:
			r.b.LastCheckStatus = "dead"
			dead = append(dead, r)
		case checkUncertain:
			r.b.LastCheckStatus = "uncertain"
			uncertain = append(uncertain, r)
		default:
			r.b.LastCheckStatus = "ok"
		}
	}
	sort.Slice(moved, func(i, j int) bool { return moved[i].b.ID < moved[j].b.ID })
	sort.Slice(dead, func(i, j int) bool { return dead[i].b.ID < dead[j].b.ID })
	sort.Slice(uncertain, func(i, j int) bool { return uncertain[i].b.ID < uncertain[j].b.ID })

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

	updated, deleted, skipped := 0, 0, 0
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
	}
	for _, r := range dead {
		if !confirm(fmt.Sprintf("Delete dead [%d] %s?", r.b.ID, r.b.Title), false) {
			skipped++
			continue
		}
		deleteBookmarkFiles(cfg, r.b)
		store.Delete(r.b.ID)
		deleted++
		fmt.Println("Deleted.")
	}
	for _, r := range uncertain {
		if !confirm(fmt.Sprintf("Delete uncertain [%d] %s (%s)?", r.b.ID, r.b.Title, r.detail), false) {
			skipped++
			continue
		}
		deleteBookmarkFiles(cfg, r.b)
		store.Delete(r.b.ID)
		deleted++
		fmt.Println("Deleted.")
	}
	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}
	fmt.Printf("Done: %d updated, %d deleted, %d skipped.\n", updated, deleted, skipped)
	return nil
}
