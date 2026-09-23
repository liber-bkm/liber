package main

import (
	"fmt"
	"html"
	"io"
	"net/http"
	"os"
	"strings"
)

func writeNetscapeExport(w io.Writer, store *Store) error {
	if _, err := io.WriteString(w, "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n"); err != nil {
		return err
	}
	if _, err := io.WriteString(w, "<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n"); err != nil {
		return err
	}
	groups := map[string][]*Bookmark{}
	var order []string
	for _, b := range store.All() {
		if _, seen := groups[b.Folder]; !seen {
			order = append(order, b.Folder)
		}
		groups[b.Folder] = append(groups[b.Folder], b)
	}
	writeEntry := func(b *Bookmark) error {
		title := b.Title
		if strings.TrimSpace(title) == "" {
			title = b.URL
		}
		if _, err := fmt.Fprintf(w, "    <DT><A HREF=\"%s\"", html.EscapeString(b.URL)); err != nil {
			return err
		}
		if len(b.Tags) > 0 {
			if _, err := fmt.Fprintf(w, " TAGS=\"%s\"", html.EscapeString(strings.Join(b.Tags, ","))); err != nil {
				return err
			}
		}
		if _, err := fmt.Fprintf(w, ">%s</A>\n", html.EscapeString(title)); err != nil {
			return err
		}
		if desc := flattenDescription(b.Description); desc != "" {
			if _, err := fmt.Fprintf(w, "    <DD>%s\n", html.EscapeString(desc)); err != nil {
				return err
			}
		}
		return nil
	}
	var open []string
	closeTo := func(n int) error {
		for len(open) > n {
			if _, err := io.WriteString(w, "</DL>\n"); err != nil {
				return err
			}
			open = open[:len(open)-1]
		}
		return nil
	}
	for _, folder := range order {
		var parts []string
		if folder != "" {
			parts = strings.Split(folder, "/")
		}
		cp := 0
		for cp < len(open) && cp < len(parts) && open[cp] == parts[cp] {
			cp++
		}
		if err := closeTo(cp); err != nil {
			return err
		}
		for _, p := range parts[cp:] {
			if _, err := fmt.Fprintf(w, "<DT><H3>%s</H3>\n<DL><p>\n", html.EscapeString(p)); err != nil {
				return err
			}
			open = append(open, p)
		}
		for _, b := range groups[folder] {
			if err := writeEntry(b); err != nil {
				return err
			}
		}
	}
	if err := closeTo(0); err != nil {
		return err
	}
	_, err := io.WriteString(w, "</DL>\n")
	return err
}

func flattenDescription(desc string) string {
	desc = strings.ReplaceAll(desc, "\r", " ")
	desc = strings.ReplaceAll(desc, "\n", " ")
	return strings.Join(strings.Fields(desc), " ")
}

func runExportBookmarks(path string) error {
	_, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	f, err := os.Create(path)
	if err != nil {
		return fmt.Errorf("writing %s: %w", path, err)
	}
	if err := writeNetscapeExport(f, store); err != nil {
		f.Close()
		return fmt.Errorf("writing %s: %w", path, err)
	}
	if err := f.Close(); err != nil {
		return fmt.Errorf("writing %s: %w", path, err)
	}
	fmt.Printf("Exported %d bookmark(s) to %s\n", len(store.Bookmarks), path)
	return nil
}

func handleExportBookmarks(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Redirect(w, r, "/settings", http.StatusSeeOther)
		return
	}
	_, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Header().Set("Content-Disposition", `attachment; filename="liber-bookmarks.html"`)
	if err := writeNetscapeExport(w, store); err != nil {
		fmt.Fprintf(os.Stderr, "warning: export failed: %v\n", err)
	}
}
