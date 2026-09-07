package main

import (
	"context"
	"encoding/base64"
	"fmt"
	"io"
	"net/http"
	neturl "net/url"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"time"
)

const (
	nativeMaxPageBytes   = 20 << 20
	nativeMaxAssetBytes  = 10 << 20
	nativeMaxTotalAssets = 50 << 20
	nativeAssetWorkers   = 6
	nativeOverallTimeout = 45 * time.Second
)

// Note: Go's RE2 has no backreferences, so quote matching is best-effort.
var (
	nativeAttrRe     = regexp.MustCompile(`(?is)\b(src|poster)\s*=\s*(["'])([^"']*)["']`)
	nativeLinkRe     = regexp.MustCompile(`(?is)<link\b[^>]*?\bhref\s*=\s*(["'])([^"']*)["']`)
	nativeSrcsetRe   = regexp.MustCompile(`(?is)\bsrcset\s*=\s*(["'])([^"']*)["']`)
	nativeCSSURLRe   = regexp.MustCompile(`(?i)url\(\s*['"]?([^'"\)]*)['"]?\s*\)`)
	nativeScriptRe   = regexp.MustCompile(`(?is)<script\b[^>]*>.*?</script>|<script\b[^>]*/>`)
	nativeNoscriptRe = regexp.MustCompile(`(?is)<noscript\b[^>]*>(.*?)</noscript>`)
)

func runNativeArchive(url, outPath string) error {
	ctx, cancel := context.WithTimeout(context.Background(), nativeOverallTimeout)
	defer cancel()

	page, pageURL, err := nativeFetch(ctx, url, nativeMaxPageBytes)
	if err != nil {
		return fmt.Errorf("fetching page: %w", err)
	}
	html := string(page)

	assets := nativeCollectAssets(html, pageURL)
	fetched := nativeFetchAll(ctx, assets)
	html = nativeInlineAll(html, pageURL, fetched)

	html = nativeScriptRe.ReplaceAllString(html, "")
	html = nativeNoscriptRe.ReplaceAllString(html, "$1")

	banner := fmt.Sprintf("<!-- saved by liber (native snapshot) from %s at %s -->\n%s",
		pageURL, time.Now().Format("2006-01-02 15:04:05"), html)

	if err := os.MkdirAll(filepath.Dir(outPath), 0o755); err != nil {
		return err
	}
	return os.WriteFile(outPath, []byte(banner), 0o644)
}

type nativeResult struct {
	data []byte
	mime string
	ok   bool
}

func resolveAssetURL(raw string, pageURL *neturl.URL) string {
	raw = strings.TrimSpace(raw)
	if raw == "" || strings.HasPrefix(raw, "data:") || strings.HasPrefix(raw, "javascript:") || strings.HasPrefix(raw, "#") {
		return ""
	}
	ref, err := neturl.Parse(raw)
	if err != nil {
		return ""
	}
	abs := pageURL.ResolveReference(ref)
	if abs.Scheme != "http" && abs.Scheme != "https" {
		return ""
	}
	return abs.String()
}

func nativeCollectAssets(html string, pageURL *neturl.URL) []string {
	seen := map[string]bool{}
	var urls []string
	add := func(raw string) {
		if s := resolveAssetURL(raw, pageURL); s != "" && !seen[s] {
			seen[s] = true
			urls = append(urls, s)
		}
	}

	for _, m := range nativeAttrRe.FindAllStringSubmatch(html, -1) {
		add(m[3])
	}
	for _, m := range nativeLinkRe.FindAllStringSubmatch(html, -1) {
		add(m[2])
	}
	for _, m := range nativeSrcsetRe.FindAllStringSubmatch(html, -1) {
		for _, cand := range strings.Split(m[2], ",") {
			if fields := strings.Fields(cand); len(fields) > 0 {
				add(fields[0])
			}
		}
	}
	for _, m := range nativeCSSURLRe.FindAllStringSubmatch(html, -1) {
		add(m[1])
	}
	return urls
}

func nativeFetchAll(ctx context.Context, urls []string) map[string]nativeResult {
	results := map[string]nativeResult{}
	var mu sync.Mutex
	sem := make(chan struct{}, nativeAssetWorkers)
	var wg sync.WaitGroup
	total := int64(0)

	for _, u := range urls {
		wg.Add(1)
		go func(u string) {
			defer wg.Done()

			mu.Lock()
			budgetLeft := nativeMaxTotalAssets - total
			mu.Unlock()
			if budgetLeft <= 0 {
				return
			}

			select {
			case sem <- struct{}{}:
				defer func() { <-sem }()
			case <-ctx.Done():
				return
			}

			data, mime, err := nativeFetchAsset(ctx, u)
			if err != nil {
				return
			}
			mu.Lock()
			if nativeMaxTotalAssets-total >= int64(len(data)) {
				total += int64(len(data))
				results[u] = nativeResult{data: data, mime: mime, ok: true}
			}
			mu.Unlock()
		}(u)
	}
	wg.Wait()
	return results
}

func nativeFetchAsset(ctx context.Context, u string) ([]byte, string, error) {
	req, err := http.NewRequestWithContext(ctx, "GET", u, nil)
	if err != nil {
		return nil, "", err
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; liber-bookmark-manager/1.0)")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, "", err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 400 {
		return nil, "", fmt.Errorf("HTTP %d", resp.StatusCode)
	}
	data, err := io.ReadAll(io.LimitReader(resp.Body, nativeMaxAssetBytes))
	if err != nil {
		return nil, "", err
	}
	mime := resp.Header.Get("Content-Type")
	if mime == "" {
		mime = http.DetectContentType(data)
	}
	return data, mime, nil
}

func nativeFetch(ctx context.Context, u string, limit int64) ([]byte, *neturl.URL, error) {
	req, err := http.NewRequestWithContext(ctx, "GET", u, nil)
	if err != nil {
		return nil, nil, err
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; liber-bookmark-manager/1.0)")
	client := &http.Client{Timeout: nativeOverallTimeout}
	resp, err := client.Do(req)
	if err != nil {
		return nil, nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 400 {
		return nil, nil, fmt.Errorf("HTTP %d", resp.StatusCode)
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, limit))
	if err != nil {
		return nil, nil, err
	}
	return body, resp.Request.URL, nil
}

func nativeInlineAll(html string, pageURL *neturl.URL, results map[string]nativeResult) string {
	inline := func(raw string) string {
		if s := resolveAssetURL(raw, pageURL); s != "" {
			if r, ok := results[s]; ok && r.ok {
				return "data:" + r.mime + ";base64," + base64.StdEncoding.EncodeToString(r.data)
			}
		}
		return raw
	}

	html = nativeAttrRe.ReplaceAllStringFunc(html, func(whole string) string {
		parts := nativeAttrRe.FindStringSubmatch(whole)
		if len(parts) != 4 {
			return whole
		}
		return parts[1] + "=" + parts[2] + inline(parts[3]) + parts[2]
	})

	html = nativeLinkRe.ReplaceAllStringFunc(html, func(tag string) string {
		parts := nativeLinkRe.FindStringSubmatch(tag)
		if len(parts) != 3 {
			return tag
		}
		old := parts[1] + parts[2] + parts[1]
		return strings.Replace(tag, old, parts[1]+inline(parts[2])+parts[1], 1)
	})

	html = nativeSrcsetRe.ReplaceAllStringFunc(html, func(whole string) string {
		parts := nativeSrcsetRe.FindStringSubmatch(whole)
		if len(parts) != 3 {
			return whole
		}
		var out []string
		for _, cand := range strings.Split(parts[2], ",") {
			fields := strings.Fields(strings.TrimSpace(cand))
			if len(fields) == 0 {
				continue
			}
			fields[0] = inline(fields[0])
			out = append(out, strings.Join(fields, " "))
		}
		return "srcset=" + parts[1] + strings.Join(out, ", ") + parts[1]
	})

	html = nativeCSSURLRe.ReplaceAllStringFunc(html, func(whole string) string {
		parts := nativeCSSURLRe.FindStringSubmatch(whole)
		if len(parts) != 2 {
			return whole
		}
		return `url("` + inline(parts[1]) + `")`
	})

	return html
}
