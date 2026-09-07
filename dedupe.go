package main

import (
	"net/url"
	"strings"
)

var trackingParams = []string{
	"utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
	"fbclid", "gclid", "mc_cid", "mc_eid", "igshid", "ref",
}

func normalizeForDedupe(raw string) string {
	u, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || u.Host == "" {
		return strings.TrimSpace(raw)
	}
	scheme := strings.ToLower(u.Scheme)
	host := strings.ToLower(u.Hostname())
	if p := u.Port(); p != "" && !((scheme == "http" && p == "80") || (scheme == "https" && p == "443")) {
		host += ":" + p
	}
	path := strings.TrimSuffix(u.Path, "/")

	query := ""
	if u.RawQuery != "" {
		q := u.Query()
		for _, k := range trackingParams {
			q.Del(k)
		}
		if enc := q.Encode(); enc != "" {
			query = "?" + enc
		}
	}
	return scheme + "://" + host + path + query
}

func findDuplicate(store *Store, url string) *Bookmark {
	target := normalizeForDedupe(url)
	for _, b := range store.Bookmarks {
		if normalizeForDedupe(b.URL) == target {
			return b
		}
	}
	return nil
}
