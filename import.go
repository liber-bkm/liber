package main

import (
	"fmt"
	"html"
	"os"
	"regexp"
	"strings"
)

var netscapeTagRe = regexp.MustCompile(`(?is)` +
	`<DT>\s*<H3(?P<h3attrs>[^>]*)>(?P<foldername>.*?)</H3>` +
	`|(?P<dlopen><DL>\s*<p>)` +
	`|(?P<dlclose></DL>)` +
	`|<DT>\s*<A\s+(?P<aattrs>[^>]*)>(?P<title>.*?)</A>` +
	`|<DD>(?P<desc>[^<\r\n]*)`)

var (
	hrefAttrRe = regexp.MustCompile(`(?i)HREF\s*=\s*"([^"]*)"`)
	tagsAttrRe = regexp.MustCompile(`(?i)TAGS\s*=\s*"([^"]*)"`)
	stripTagRe = regexp.MustCompile(`<[^>]*>`)
)

type importedEntry struct {
	title   string
	href    string
	folder  string
	tagsRaw string
	desc    string
}

func parseNetscapeBookmarks(content string) []importedEntry {
	matches := netscapeTagRe.FindAllStringSubmatchIndex(content, -1)
	names := netscapeTagRe.SubexpNames()

	group := func(m []int, name string) (string, bool) {
		for i, n := range names {
			if n == name && m[2*i] != -1 {
				return content[m[2*i]:m[2*i+1]], true
			}
		}
		return "", false
	}

	var entries []importedEntry
	var folderStack []string
	pendingFolder := ""

	for _, m := range matches {
		if name, ok := group(m, "foldername"); ok {
			pendingFolder = html.UnescapeString(strings.TrimSpace(stripTagRe.ReplaceAllString(name, "")))
			continue
		}
		if _, ok := group(m, "dlopen"); ok {
			folderStack = append(folderStack, pendingFolder)
			pendingFolder = ""
			continue
		}
		if _, ok := group(m, "dlclose"); ok {
			if len(folderStack) > 0 {
				folderStack = folderStack[:len(folderStack)-1]
			}
			continue
		}
		if attrs, ok := group(m, "aattrs"); ok {
			title, _ := group(m, "title")
			href := ""
			if hm := hrefAttrRe.FindStringSubmatch(attrs); hm != nil {
				href = html.UnescapeString(hm[1])
			}
			tagsRaw := ""
			if tm := tagsAttrRe.FindStringSubmatch(attrs); tm != nil {
				tagsRaw = html.UnescapeString(tm[1])
			}
			var parts []string
			for _, f := range folderStack {
				if f != "" {
					parts = append(parts, f)
				}
			}
			entries = append(entries, importedEntry{
				title:   html.UnescapeString(strings.TrimSpace(stripTagRe.ReplaceAllString(title, ""))),
				href:    href,
				folder:  strings.Join(parts, "/"),
				tagsRaw: tagsRaw,
			})
			continue
		}
		if desc, ok := group(m, "desc"); ok {
			if len(entries) > 0 {
				entries[len(entries)-1].desc = html.UnescapeString(strings.TrimSpace(desc))
			}
		}
	}
	return entries
}

type importOptions struct {
	Markdown bool
	Archive  bool
}

func parseImportFlags(args []string) (importOptions, error) {
	var opt importOptions
	for _, a := range args {
		switch a {
		case "-md", "--markdown":
			opt.Markdown = true
		case "-a", "--archive":
			opt.Archive = true
		default:
			return opt, fmt.Errorf("unknown flag for --import: %s", a)
		}
	}
	return opt, nil
}

func runImport(path string, opt importOptions) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("reading %s: %w", path, err)
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	entries := parseNetscapeBookmarks(string(data))
	if len(entries) == 0 {
		fmt.Println("No bookmarks found in that file -- is it a browser bookmark export (Netscape Bookmark File Format)?")
		return nil
	}

	imported, skippedDup, skippedBad := 0, 0, 0
	for _, e := range entries {
		if strings.TrimSpace(e.href) == "" {
			skippedBad++
			continue
		}
		url := normalizeURL(e.href)
		if findDuplicate(store, url) != nil {
			skippedDup++
			continue
		}

		title := e.title
		if strings.TrimSpace(title) == "" {
			title = url
		}
		var tags []string
		if e.tagsRaw != "" {
			for _, t := range strings.Split(e.tagsRaw, ",") {
				t = strings.TrimSpace(t)
				if t != "" {
					tags = append(tags, t)
				}
			}
		}
		folder := sanitizeFolder(e.folder)
		folder, tags, appliedRuleIDs := resolveAutoRulesForNew(store, url, title, folder, tags)

		b, err := addBookmarkToStore(cfg, store, url, title, e.desc, tags, folder, opt.Markdown, opt.Archive)
		if err != nil {
			fmt.Printf("warning: could not import %s: %v\n", url, err)
			continue
		}
		b.AppliedRules = appliedRuleIDs
		imported++
	}

	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}

	fmt.Printf("Imported %d bookmark(s).\n", imported)
	if skippedDup > 0 {
		fmt.Printf("Skipped %d already in your collection.\n", skippedDup)
	}
	if skippedBad > 0 {
		fmt.Printf("Skipped %d entr%s with no URL.\n", skippedBad, entrySuffix(skippedBad))
	}
	return nil
}
