package main

import (
	"bytes"
	"fmt"
	"html/template"
	"net/http"
	neturl "net/url"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"
)

var writeMu sync.Mutex

const webPageSize = 500

func parseServeFlags(args []string) (addr string, err error) {
	addr = "127.0.0.1:8080"
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--addr":
			if i+1 >= len(args) {
				return "", fmt.Errorf("--addr requires a value, e.g. 127.0.0.1:9000")
			}
			addr = args[i+1]
			i++
		default:
			return "", fmt.Errorf("unknown flag for --serve: %s", args[i])
		}
	}
	return addr, nil
}

func runServe(args []string) error {
	addr, err := parseServeFlags(args)
	if err != nil {
		return err
	}
	if !strings.HasPrefix(addr, "127.0.0.1:") && !strings.HasPrefix(addr, "localhost:") {
		fmt.Println("warning: binding to a non-loopback address exposes your whole bookmark collection -- read, add, edit AND delete access -- to anyone who can reach it, with no login. Only do this on a network you trust.")
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/", handleSearch)
	mux.HandleFunc("/add", handleAdd)
	mux.HandleFunc("/edit/", handleEdit)
	mux.HandleFunc("/delete", handleDelete)
	mux.HandleFunc("/archive/", handleArchive)
	mux.HandleFunc("/markdown/", handleMarkdown)
	mux.HandleFunc("/attachment/", handleAttachment)
	mux.HandleFunc("/settings", handleSettings)
	mux.HandleFunc("/settings/auto/", handleSettingsAuto)

	fmt.Printf("liber web UI: http://%s (Ctrl+C to stop)\n", addr)
	return http.ListenAndServe(addr, mux)
}

func scopeFromParams(vals []string) SearchFields {
	var f SearchFields
	for _, v := range vals {
		switch v {
		case "n":
			f.Title = true
		case "u":
			f.URL = true
		case "t":
			f.Tags = true
		case "d":
			f.Description = true
		case "f":
			f.Folder = true
		}
	}
	return f
}

func paginate(list []*Bookmark, page int) (pageItems []*Bookmark, totalPages, curPage int) {
	total := len(list)
	if total <= webPageSize {
		return list, 1, 1
	}
	totalPages = (total + webPageSize - 1) / webPageSize
	if page < 1 {
		page = 1
	}
	if page > totalPages {
		page = totalPages
	}
	start := (page - 1) * webPageSize
	end := start + webPageSize
	if end > total {
		end = total
	}
	return list[start:end], totalPages, page
}

func pageURL(r *http.Request, page int) string {
	v := neturl.Values{}
	if q := r.URL.Query().Get("q"); q != "" {
		v.Set("q", q)
	}
	for _, s := range r.URL.Query()["scope"] {
		v.Add("scope", s)
	}
	if r.URL.Query().Get("deep") == "1" {
		v.Set("deep", "1")
	}
	v.Set("page", strconv.Itoa(page))
	return "/?" + v.Encode()
}

type webBookmarkView struct {
	ID                       int
	Title, URL, Folder, Desc string
	FolderRaw                string // unpadded, for filter links
	Tags                     []string
	HasMarkdown, HasArchive  bool
	AttachCount              int
}

func toWebViews(list []*Bookmark) []webBookmarkView {
	out := make([]webBookmarkView, 0, len(list))
	for _, b := range list {
		out = append(out, webBookmarkView{
			ID: b.ID, Title: b.Title, URL: b.URL,
			Folder: displayFolder(b.Folder), FolderRaw: b.Folder,
			Desc: b.Description, Tags: b.Tags,
			HasMarkdown: b.MarkdownFile != "", HasArchive: b.ArchiveFile != "",
			AttachCount: len(b.Attachments),
		})
	}
	return out
}

type searchPageData struct {
	Query                                                                string
	ScopeTitle, ScopeURL, ScopeTags, ScopeDescription, ScopeFolder, Deep bool
	Flash                                                                string
	ShowAdd, PendingConfirm                                              bool
	DupWarning                                                           string
	PrefillURL, PrefillDescription, PrefillTags, PrefillFolder           string
	PrefillMarkdown, PrefillArchive                                      bool
	DeleteConfirmID                                                      int
	DeleteConfirmTitle                                                   string
	ResultCount                                                          int
	Results                                                              []webBookmarkView
	Page, TotalPages                                                     int
	PrevURL, NextURL                                                     string
}

func handleSearch(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/" {
		http.NotFound(w, r)
		return
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	q := r.URL.Query().Get("q")
	deep := r.URL.Query().Get("deep") == "1"
	fields := scopeFromParams(r.URL.Query()["scope"])
	page, _ := strconv.Atoi(r.URL.Query().Get("page"))

	var results []*Bookmark
	switch {
	case deep && strings.TrimSpace(q) != "":
		results = filterDeep(cfg, store.All(), q, fields)
	case deep:
		results = store.All()
	default:
		results = store.Search(cfg, q, fields, false)
	}

	pageItems, totalPages, curPage := paginate(results, page)
	prefillURL := r.URL.Query().Get("prefill")

	data := searchPageData{
		Query: q, Deep: deep,
		ScopeTitle: fields.Title, ScopeURL: fields.URL, ScopeTags: fields.Tags,
		ScopeDescription: fields.Description, ScopeFolder: fields.Folder,
		Flash:       r.URL.Query().Get("msg"),
		ShowAdd:     prefillURL != "",
		PrefillURL:  prefillURL,
		ResultCount: len(results),
		Results:     toWebViews(pageItems),
		Page:        curPage, TotalPages: totalPages,
	}
	if totalPages > 1 {
		if curPage > 1 {
			data.PrevURL = pageURL(r, curPage-1)
		}
		if curPage < totalPages {
			data.NextURL = pageURL(r, curPage+1)
		}
	}
	renderSearchPage(w, data)
}

type addFormState struct {
	ShowAdd, PendingConfirm                                    bool
	DupWarning                                                 string
	PrefillURL, PrefillDescription, PrefillTags, PrefillFolder string
	PrefillMarkdown, PrefillArchive                            bool
}

func renderSearchPageWithAddState(w http.ResponseWriter, store *Store, add addFormState) {
	results := store.All()
	pageItems, totalPages, curPage := paginate(results, 1)
	renderSearchPage(w, searchPageData{
		ResultCount: len(results), Results: toWebViews(pageItems), Page: curPage, TotalPages: totalPages,
		ShowAdd: add.ShowAdd, PendingConfirm: add.PendingConfirm, DupWarning: add.DupWarning,
		PrefillURL: add.PrefillURL, PrefillDescription: add.PrefillDescription,
		PrefillTags: add.PrefillTags, PrefillFolder: add.PrefillFolder,
		PrefillMarkdown: add.PrefillMarkdown, PrefillArchive: add.PrefillArchive,
	})
}

func renderDeleteConfirm(w http.ResponseWriter, store *Store, b *Bookmark) {
	results := store.All()
	pageItems, totalPages, curPage := paginate(results, 1)
	renderSearchPage(w, searchPageData{
		ResultCount: len(results), Results: toWebViews(pageItems), Page: curPage, TotalPages: totalPages,
		DeleteConfirmID: b.ID, DeleteConfirmTitle: b.Title,
	})
}

func parseWebForm(r *http.Request) error {
	if strings.HasPrefix(r.Header.Get("Content-Type"), "multipart/form-data") {
		return r.ParseMultipartForm(256 << 20)
	}
	return r.ParseForm()
}

func attachUploads(cfg Config, b *Bookmark, r *http.Request) {
	if r.MultipartForm == nil {
		return
	}
	defer r.MultipartForm.RemoveAll()
	for _, fh := range r.MultipartForm.File["attachments"] {
		f, err := fh.Open()
		if err != nil {
			continue
		}
		if err := attachReader(cfg, b, fh.Filename, f); err != nil {
			fmt.Fprintf(os.Stderr, "warning: could not attach %s: %v\n", fh.Filename, err)
		}
		f.Close()
	}
}

func handleAdd(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/", http.StatusSeeOther)
		return
	}
	if err := parseWebForm(r); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}

	rawURL := strings.TrimSpace(r.FormValue("url"))
	folderIn := r.FormValue("folder")
	tagsRaw := r.FormValue("tags")
	description := strings.TrimSpace(r.FormValue("description"))
	addMarkdown := r.FormValue("markdown") == "on"
	addArchive := r.FormValue("archive") == "on"
	confirmDup := r.FormValue("confirm_dup") == "1"

	writeMu.Lock()
	defer writeMu.Unlock()

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	if rawURL == "" {
		renderSearchPageWithAddState(w, store, addFormState{
			ShowAdd: true, DupWarning: "A URL is required.",
			PrefillDescription: description, PrefillTags: tagsRaw, PrefillFolder: folderIn,
			PrefillMarkdown: addMarkdown, PrefillArchive: addArchive,
		})
		return
	}

	normalizedURL := normalizeURL(rawURL)

	if !confirmDup {
		if dup := findDuplicate(store, normalizedURL); dup != nil {
			renderSearchPageWithAddState(w, store, addFormState{
				ShowAdd: true, PendingConfirm: true,
				DupWarning: fmt.Sprintf("This looks like it's already bookmarked: [%d] %s (folder: %s). Submit again to add it anyway.",
					dup.ID, dup.Title, displayFolder(dup.Folder)),
				PrefillURL: rawURL, PrefillDescription: description, PrefillTags: tagsRaw, PrefillFolder: folderIn,
				PrefillMarkdown: addMarkdown, PrefillArchive: addArchive,
			})
			return
		}
	}

	tags := splitWebTags(tagsRaw)
	folder := sanitizeFolder(folderIn)

	title := fetchTitle(normalizedURL)
	if strings.TrimSpace(title) == "" {
		title = normalizedURL
	}
	folder, tags, appliedRuleIDs := resolveAutoRulesForNew(store, normalizedURL, title, folder, tags)

	b, err := addBookmarkToStore(cfg, store, normalizedURL, title, description, tags, folder, addMarkdown, addArchive)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	b.AppliedRules = appliedRuleIDs
	attachUploads(cfg, b, r)
	if err := store.Save(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/?msg="+neturl.QueryEscape(fmt.Sprintf("Added [%d] %s", b.ID, b.Title)), http.StatusSeeOther)
}

func splitWebTags(raw string) []string {
	var tags []string
	for _, t := range strings.FieldsFunc(raw, func(c rune) bool { return c == ',' || c == ' ' }) {
		t = strings.TrimSpace(t)
		if t != "" {
			tags = append(tags, t)
		}
	}
	return tags
}

type webAttachmentView struct {
	Idx  int // 1-based, matches the CLI's attachment numbering
	Name string
}

type editPageData struct {
	ID                         int
	Title, Description, Folder string
	URL, TagsJoined            string
	HasMarkdown, HasArchive    bool
	Attachments                []webAttachmentView
	Error                      string
}

func renderEditPage(w http.ResponseWriter, b *Bookmark, errMsg string) {
	var buf bytes.Buffer
	var atts []webAttachmentView
	for i, at := range b.Attachments {
		atts = append(atts, webAttachmentView{Idx: i + 1, Name: at.Name})
	}
	editBodyTmpl.Execute(&buf, editPageData{
		ID: b.ID, Title: b.Title, Description: b.Description, Folder: b.Folder, URL: b.URL,
		TagsJoined:  strings.Join(b.Tags, ", "),
		HasMarkdown: b.MarkdownFile != "", HasArchive: b.ArchiveFile != "",
		Attachments: atts,
		Error:       errMsg,
	})
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	layoutTmpl.Execute(w, struct {
		Title string
		Body  template.HTML
	}{fmt.Sprintf("liber - edit [%d]", b.ID), template.HTML(buf.String())})
}

func handleEdit(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.Atoi(strings.TrimPrefix(r.URL.Path, "/edit/"))
	if err != nil {
		http.NotFound(w, r)
		return
	}

	if r.Method == http.MethodPost {
		handleEditSave(w, r, id)
		return
	}

	_, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	b := store.Find(id)
	if b == nil {
		http.NotFound(w, r)
		return
	}
	renderEditPage(w, b, "")
}

func handleEditSave(w http.ResponseWriter, r *http.Request, id int) {
	if err := parseWebForm(r); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}

	writeMu.Lock()
	defer writeMu.Unlock()

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	b := store.Find(id)
	if b == nil {
		http.NotFound(w, r)
		return
	}

	newTitle := strings.TrimSpace(r.FormValue("title"))
	if newTitle == "" {
		renderEditPage(w, b, "Title can't be empty.")
		return
	}

	b.Title = newTitle
	b.Description = strings.TrimSpace(r.FormValue("description"))
	b.Tags = dedupe(splitWebTags(r.FormValue("tags")))
	newFolder := sanitizeFolder(r.FormValue("folder"))
	folderChanged := newFolder != b.Folder
	b.Folder = newFolder
	b.UpdatedAt = time.Now()
	syncBookmarkFiles(cfg, b, folderChanged)

	if r.FormValue("markdown") == "on" {
		addMarkdownCopy(cfg, b)
	}
	if r.FormValue("archive") == "on" {
		addArchiveCopy(cfg, b)
	}

	// highest index first, so splice indices stay valid
	var delIdx []int
	for _, v := range r.Form["delatt"] {
		if n, err := strconv.Atoi(v); err == nil {
			delIdx = append(delIdx, n)
		}
	}
	sort.Sort(sort.Reverse(sort.IntSlice(delIdx)))
	for _, n := range delIdx {
		if n >= 1 && n <= len(b.Attachments) {
			detachAttachment(cfg, b, n-1)
		}
	}
	attachUploads(cfg, b, r)

	if err := store.Save(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/?msg="+neturl.QueryEscape(fmt.Sprintf("Updated [%d] %s", b.ID, b.Title)), http.StatusSeeOther)
}

func handleDelete(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/", http.StatusSeeOther)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}
	id, err := strconv.Atoi(r.FormValue("id"))
	if err != nil {
		http.NotFound(w, r)
		return
	}
	confirmed := r.FormValue("confirm") == "1"

	writeMu.Lock()
	defer writeMu.Unlock()

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	b := store.Find(id)
	if b == nil {
		http.NotFound(w, r)
		return
	}

	if !confirmed {
		renderDeleteConfirm(w, store, b)
		return
	}

	deleteBookmarkFiles(cfg, b)
	store.Delete(b.ID)
	if err := store.Save(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/?msg="+neturl.QueryEscape(fmt.Sprintf("Deleted [%d] %s", b.ID, b.Title)), http.StatusSeeOther)
}

func handleAttachment(w http.ResponseWriter, r *http.Request) {
	parts := strings.SplitN(strings.TrimPrefix(r.URL.Path, "/attachment/"), "/", 2)
	if len(parts) != 2 {
		http.NotFound(w, r)
		return
	}
	id, err1 := strconv.Atoi(parts[0])
	n, err2 := strconv.Atoi(parts[1])
	if err1 != nil || err2 != nil {
		http.NotFound(w, r)
		return
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	b := store.Find(id)
	if b == nil || n < 1 || n > len(b.Attachments) {
		http.NotFound(w, r)
		return
	}
	http.ServeFile(w, r, filepath.Join(cfg.attachmentsDir(), b.Attachments[n-1].File))
}

func handleArchive(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.Atoi(strings.TrimPrefix(r.URL.Path, "/archive/"))
	if err != nil {
		http.NotFound(w, r)
		return
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	b := store.Find(id)
	if b == nil || b.ArchiveFile == "" {
		http.NotFound(w, r)
		return
	}
	http.ServeFile(w, r, filepath.Join(cfg.archiveDir(), b.ArchiveFile))
}

func handleMarkdown(w http.ResponseWriter, r *http.Request) {
	id, err := strconv.Atoi(strings.TrimPrefix(r.URL.Path, "/markdown/"))
	if err != nil {
		http.NotFound(w, r)
		return
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	b := store.Find(id)
	if b == nil || b.MarkdownFile == "" {
		http.NotFound(w, r)
		return
	}
	data, err := os.ReadFile(filepath.Join(cfg.markdownDir(), b.MarkdownFile))
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	fmt.Fprintf(w, `<!DOCTYPE html><html><head><meta charset="utf-8"><title>%s</title>
<script>%s</script>
<style>
:root, html[data-theme=light] { --bg: #fbf1c7; --fg: #3c3836; --link: #076678; color-scheme: light; }
html[data-theme=dark] { --bg: #282828; --fg: #ebdbb2; --link: #83a598; color-scheme: dark; }
body { font-family: ui-monospace, monospace; max-width: 800px; margin: 2rem auto; padding: 0 1rem; white-space: pre-wrap; word-wrap: break-word; color: var(--fg); background: var(--bg); }
a { color: var(--link); }
</style>
</head><body><p><a href="/">&larr; back</a></p><pre>%s</pre></body></html>`,
		template.HTMLEscapeString(b.Title), themeInitScript, template.HTMLEscapeString(string(data)))
}

func renderSearchPage(w http.ResponseWriter, data searchPageData) {
	var buf bytes.Buffer
	if err := searchBodyTmpl.Execute(&buf, data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	layoutTmpl.Execute(w, struct {
		Title string
		Body  template.HTML
	}{"liber", template.HTML(buf.String())})
}

const pageCSS = `
:root, html[data-theme=light] {
  --bg: #fbf1c7; --fg: #3c3836; --fg-soft: #504945; --muted: #7c6f64;
  --surface: #f2e5bc; --surface2: #ebdbb2;
  --border: #d5c4a1; --border-strong: #bdae93;
  --link: #076678; --tag: #af3a03; --danger: #cc241d;
  --ok-bg: #e4e3bf; --ok-border: #79740e;
  --err-bg: #f6dfe1; --err-border: #cc241d;
  color-scheme: light;
}
html[data-theme=dark] {
  --bg: #282828; --fg: #ebdbb2; --fg-soft: #d5c4a1; --muted: #a89984;
  --surface: #3c3836; --surface2: #504945;
  --border: #504945; --border-strong: #665c54;
  --link: #83a598; --tag: #fe8019; --danger: #fb4934;
  --ok-bg: #2e3324; --ok-border: #b8bb26;
  --err-bg: #382320; --err-border: #fb4934;
  color-scheme: dark;
}
* { box-sizing: border-box; }
body { font-family: -apple-system, system-ui, sans-serif; max-width: 900px; margin: 2rem auto; padding: 0 1rem; color: var(--fg); background: var(--bg); line-height: 1.5; }
h1 { font-size: 1.3rem; margin-bottom: 1rem; }
h1 a { color: inherit; text-decoration: none; }
h2 { font-size: 1.1rem; }
.searchform { display: flex; flex-wrap: wrap; gap: .6rem; align-items: center; margin-bottom: .75rem; }
.searchform input[type=text] { flex: 1; min-width: 200px; padding: .4rem .6rem; background: var(--surface2); color: var(--fg); border: 1px solid var(--border-strong); border-radius: 4px; }
.searchform label { font-size: .85rem; color: var(--fg-soft); white-space: nowrap; }
button { padding: .4rem .8rem; border: 1px solid var(--border-strong); background: var(--surface2); color: var(--fg); border-radius: 4px; cursor: pointer; }
details { margin: 1rem 0; border: 1px solid var(--border); border-radius: 4px; padding: .5rem .75rem; background: var(--surface); }
summary { cursor: pointer; font-weight: 600; }
.addform, .editform { display: flex; flex-direction: column; gap: .5rem; margin-top: .75rem; max-width: 480px; }
.addform input[type=text], .editform input[type=text] { padding: .4rem .6rem; background: var(--surface2); color: var(--fg); border: 1px solid var(--border-strong); border-radius: 4px; }
.editform label { font-size: .85rem; color: var(--fg-soft); }
.flash { background: var(--ok-bg); border: 1px solid var(--ok-border); padding: .5rem .75rem; border-radius: 4px; margin: .5rem 0; }
.flash.error { background: var(--err-bg); border-color: var(--err-border); }
.confirmbox { background: var(--err-bg); border: 1px solid var(--err-border); padding: .5rem .75rem; border-radius: 4px; margin: .5rem 0; display: flex; gap: .6rem; align-items: center; flex-wrap: wrap; }
.count { color: var(--muted); font-size: .85rem; }
ul.results { list-style: none; padding: 0; }
ul.results li { padding: .6rem 0; border-bottom: 1px solid var(--border); }
.title a.link { font-weight: 600; color: var(--link); text-decoration: none; }
.title a.link:hover { text-decoration: underline; }
.badge { font-size: .7rem; background: var(--surface2); padding: .05rem .4rem; border-radius: 999px; color: var(--fg-soft); text-decoration: none; margin-left: .3rem; }
.meta { font-size: .8rem; color: var(--muted); margin-top: .15rem; }
.tag { color: var(--tag); }
a.chip { text-decoration: none; }
a.chip:hover { text-decoration: underline; }
a.chip.folder { color: var(--fg-soft); }
.desc { font-size: .85rem; color: var(--fg-soft); margin-top: .2rem; }
.rowlinks { font-size: .8rem; margin-top: .25rem; }
.rowlinks a { color: var(--muted); text-decoration: none; margin-right: .8rem; }
.rowlinks a:hover { text-decoration: underline; }
.inlineform { display: inline; }
.attfieldset { border: 1px solid var(--border); border-radius: 4px; padding: .4rem .6rem; }
.attfieldset legend { font-size: .85rem; color: var(--fg-soft); }
.attlist { list-style: none; padding: 0; margin: 0; font-size: .85rem; display: flex; flex-direction: column; gap: .25rem; }
.attlist a { color: var(--link); text-decoration: none; }
.attlist a:hover { text-decoration: underline; }
.linklike { background: none; border: none; padding: 0; margin-right: .8rem; color: var(--danger); text-decoration: none; cursor: pointer; font: inherit; font-size: .8rem; }
.linklike:hover { text-decoration: underline; }
.pager { display: flex; justify-content: center; gap: 1rem; margin: 1rem 0; font-size: .9rem; }
.pager .disabled { color: var(--muted); }
.pager a { color: var(--link); text-decoration: none; }
.themetoggle { position: fixed; top: .8rem; right: .8rem; z-index: 10; width: 2.1rem; height: 2.1rem; padding: 0; border-radius: 999px; background: var(--surface2); color: var(--fg); border: 1px solid var(--border-strong); cursor: pointer; font-size: 1rem; line-height: 1; }
.settingslink { position: fixed; top: .8rem; right: 3.3rem; z-index: 10; width: 2.1rem; height: 2.1rem; display: flex; align-items: center; justify-content: center; border-radius: 999px; background: var(--surface2); color: var(--fg); border: 1px solid var(--border-strong); text-decoration: none; font-size: 1rem; line-height: 1; }
.settingslink:hover { text-decoration: none; }
.setform { display: grid; grid-template-columns: 210px 1fr; gap: .45rem .8rem; align-items: center; margin: .5rem 0 0; max-width: 780px; }
.setform label { font-size: .85rem; color: var(--fg-soft); }
.setform input[type=text] { padding: .35rem .55rem; background: var(--surface2); color: var(--fg); border: 1px solid var(--border-strong); border-radius: 4px; font-family: inherit; }
.setform input:placeholder-shown { }
.setdetect { font-size: .8rem; color: var(--muted); }
.ruleform { border: 1px solid var(--border); border-radius: 4px; padding: .6rem .8rem; margin: .5rem 0; background: var(--surface); }
.ruleform .fields { display: flex; flex-wrap: wrap; gap: .5rem; margin: .3rem 0; }
.ruleform input[type=text] { padding: .3rem .5rem; background: var(--surface2); color: var(--fg); border: 1px solid var(--border-strong); border-radius: 4px; }
.ruleform label { font-size: .8rem; color: var(--muted); display: flex; flex-direction: column; gap: .15rem; }
.stry { display: flex; flex-wrap: wrap; gap: .6rem; align-items: center; }
`

const themeInitScript = `(function(){
var t = null;
try { t = localStorage.getItem('liber-theme'); } catch (e) {}
if (t !== 'light' && t !== 'dark') {
  t = (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) ? 'dark' : 'light';
}
document.documentElement.setAttribute('data-theme', t);
})();`

const themeToggleScript = `(function(){
var b = document.getElementById('themetoggle');
if (!b) return;
var cur = function(){ return document.documentElement.getAttribute('data-theme') || 'light'; };
var paint = function(){ b.textContent = cur() === 'dark' ? '☀' : '☾'; };
b.addEventListener('click', function(){
  var t = cur() === 'dark' ? 'light' : 'dark';
  document.documentElement.setAttribute('data-theme', t);
  try { localStorage.setItem('liber-theme', t); } catch (e) {}
  paint();
});
paint();
})();`

var layoutTmpl = template.Must(template.New("layout").Parse(`<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>{{.Title}}</title><script>` + themeInitScript + `</script><style>` + pageCSS + `</style></head>
<body>
<a class="settingslink" href="/settings" title="Settings" aria-label="Settings">&#9881;</a>
<button id="themetoggle" class="themetoggle" type="button" title="Toggle light/dark theme" aria-label="Toggle light/dark theme"></button>
<div class="wrap">
<h1><a href="/">liber</a></h1>
{{.Body}}
</div>
<script>` + themeToggleScript + `</script>
</body></html>`))

var editBodyTmpl = template.Must(template.New("editBody").Parse(`
<p><a href="/">&larr; back to search</a></p>
<h2>Edit [{{.ID}}]</h2>
{{if .Error}}<p class="flash error">{{.Error}}</p>{{end}}
<form method="post" action="/edit/{{.ID}}" class="editform" enctype="multipart/form-data">
  <label>Title<br><input type="text" name="title" value="{{.Title}}" required></label>
  <label>Description<br><input type="text" name="description" value="{{.Description}}"></label>
  <label>Tags<br><input type="text" name="tags" value="{{.TagsJoined}}" placeholder="comma or space separated"></label>
  <label>Folder<br><input type="text" name="folder" value="{{.Folder}}"></label>
  {{if not .HasMarkdown}}<label><input type="checkbox" name="markdown"> add markdown copy</label>{{end}}
  {{if not .HasArchive}}<label><input type="checkbox" name="archive"> add archive</label>{{end}}
  {{if .Attachments}}
  <fieldset class="attfieldset">
    <legend>attachments</legend>
    <ul class="attlist">
    {{range .Attachments}}
      <li><label><input type="checkbox" name="delatt" value="{{.Idx}}"> remove</label>
        <a href="/attachment/{{$.ID}}/{{.Idx}}" target="_blank" rel="noopener">{{.Name}}</a></li>
    {{end}}
    </ul>
  </fieldset>
  {{end}}
  <label>Attach files<br><input type="file" name="attachments" multiple></label>
  <button type="submit">Save</button>
</form>
<p class="rowlinks">
  <a href="{{.URL}}" target="_blank" rel="noopener">visit original</a>
  {{if .HasMarkdown}}<a href="/markdown/{{.ID}}">view markdown</a>{{end}}
  {{if .HasArchive}}<a href="/archive/{{.ID}}">view archive</a>{{end}}
</p>
`))

var searchBodyTmpl = template.Must(template.New("searchBody").Parse(`
<form method="get" action="/" class="searchform">
  <input type="text" name="q" value="{{.Query}}" placeholder="Search...">
  <label><input type="checkbox" name="scope" value="n" {{if .ScopeTitle}}checked{{end}}> title</label>
  <label><input type="checkbox" name="scope" value="u" {{if .ScopeURL}}checked{{end}}> url</label>
  <label><input type="checkbox" name="scope" value="t" {{if .ScopeTags}}checked{{end}}> tags</label>
  <label><input type="checkbox" name="scope" value="d" {{if .ScopeDescription}}checked{{end}}> description</label>
  <label><input type="checkbox" name="scope" value="f" {{if .ScopeFolder}}checked{{end}}> folder</label>
  <label><input type="checkbox" name="deep" value="1" {{if .Deep}}checked{{end}}> deep (archive content)</label>
  <button type="submit">Search</button>
</form>

{{if .Flash}}<p class="flash">{{.Flash}}</p>{{end}}

{{if .DeleteConfirmTitle}}
<form method="post" action="/delete" class="confirmbox">
  <span>Delete [{{.DeleteConfirmID}}] {{.DeleteConfirmTitle}}?</span>
  <input type="hidden" name="id" value="{{.DeleteConfirmID}}">
  <input type="hidden" name="confirm" value="1">
  <button type="submit">Yes, delete</button>
</form>
{{end}}

<details {{if .ShowAdd}}open{{end}}>
<summary>+ Add bookmark</summary>
{{if .DupWarning}}<p class="flash error">{{.DupWarning}}</p>{{end}}
<form method="post" action="/add" class="addform" enctype="multipart/form-data">
  {{if .PendingConfirm}}<input type="hidden" name="confirm_dup" value="1">{{end}}
  <input type="text" name="url" placeholder="https://example.com" required value="{{.PrefillURL}}">
  <input type="text" name="description" placeholder="Description (optional)" value="{{.PrefillDescription}}">
  <input type="text" name="tags" placeholder="tags, comma or space separated" value="{{.PrefillTags}}">
  <input type="text" name="folder" placeholder="folder (optional)" value="{{.PrefillFolder}}">
  <label><input type="checkbox" name="markdown" {{if .PrefillMarkdown}}checked{{end}}> markdown copy</label>
  <label><input type="checkbox" name="archive" {{if .PrefillArchive}}checked{{end}}> archive</label>
  <label>attach files: <input type="file" name="attachments" multiple></label>
  <button type="submit">{{if .PendingConfirm}}Yes, add anyway{{else}}Add{{end}}</button>
</form>
</details>

<p class="count">{{.ResultCount}} bookmark(s){{if .Deep}} &middot; searched metadata + archive content{{end}}{{if gt .TotalPages 1}} &middot; page {{.Page}} of {{.TotalPages}}{{end}}</p>

<ul class="results">
{{range .Results}}
<li>
  <div class="title"><a class="link" href="{{.URL}}" target="_blank" rel="noopener">{{.Title}}</a>{{if .HasMarkdown}} <a class="badge" href="/markdown/{{.ID}}">md</a>{{end}}{{if .HasArchive}} <a class="badge" href="/archive/{{.ID}}">arc</a>{{end}}{{if .AttachCount}} <a class="badge" href="/edit/{{.ID}}" title="attachments">att{{if gt .AttachCount 1}}{{.AttachCount}}{{end}}</a>{{end}}</div>
  <div class="meta">{{.URL}} &middot; {{if .FolderRaw}}<a class="chip folder" href="/?q={{.FolderRaw | urlquery}}&amp;scope=f" title="filter by folder {{.Folder}}">{{.Folder}}</a>{{else}}{{.Folder}}{{end}}{{if .Tags}}{{range .Tags}} <a class="tag chip" href="/?q={{. | urlquery}}&amp;scope=t" title="filter by tag {{.}}">#{{.}}</a>{{end}}{{end}} &middot; id {{.ID}}</div>
  {{if .Desc}}<div class="desc">{{.Desc}}</div>{{end}}
  <div class="rowlinks">
    <a href="/edit/{{.ID}}">edit</a>
    <form method="post" action="/delete" class="inlineform">
      <input type="hidden" name="id" value="{{.ID}}">
      <button type="submit" class="linklike">delete</button>
    </form>
  </div>
</li>
{{end}}
</ul>

{{if gt .TotalPages 1}}
<p class="pager">
  {{if .PrevURL}}<a href="{{.PrevURL}}">&larr; prev</a>{{else}}<span class="disabled">&larr; prev</span>{{end}}
  <span>page {{.Page}} of {{.TotalPages}}</span>
  {{if .NextURL}}<a href="{{.NextURL}}">next &rarr;</a>{{else}}<span class="disabled">next &rarr;</span>{{end}}
</p>
{{end}}
`))
