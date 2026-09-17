package main

import (
	"bytes"
	"fmt"
	"html/template"
	"net/http"
	neturl "net/url"
	"sort"
	"strconv"
	"strings"
)

type taxRow struct {
	Name    string
	Display string
	Count   int
}

type taxonomyPageData struct {
	Flash   string
	Tags    []taxRow
	Folders []taxRow
	Learn   []learnRow
	Min     int
}

type learnRow struct {
	Host   string
	Folder string
	Count  int
}

func sortedTaxRows(counts map[string]int) []taxRow {
	var out []taxRow
	for name, n := range counts {
		out = append(out, taxRow{Name: name, Display: name, Count: n})
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Count != out[j].Count {
			return out[i].Count > out[j].Count
		}
		return out[i].Name < out[j].Name
	})
	return out
}

func learnRows(store *Store, min int) []learnRow {
	var out []learnRow
	for _, s := range suggestRules(store, min) {
		out = append(out, learnRow{Host: s.host, Folder: s.folder, Count: s.count})
	}
	return out
}

func learnMin(r *http.Request) int {
	min := 3
	if n, err := strconv.Atoi(strings.TrimSpace(r.FormValue("min"))); err == nil && n >= 2 {
		min = n
	}
	return min
}

func renderTaxonomyPage(w http.ResponseWriter, store *Store, flash string, min int) {
	var buf bytes.Buffer
	folders := sortedTaxRows(folderCounts(store))
	for i := range folders {
		folders[i].Display = displayFolder(folders[i].Name)
	}
	if err := taxTmpl.Execute(&buf, taxonomyPageData{
		Flash:   flash,
		Tags:    sortedTaxRows(tagCounts(store)),
		Folders: folders,
		Learn:   learnRows(store, min),
		Min:     min,
	}); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	layoutTmpl.Execute(w, struct {
		Title string
		Body  template.HTML
	}{"liber tags", template.HTML(buf.String())})
}

func redirectTags(w http.ResponseWriter, r *http.Request, msg string) {
	http.Redirect(w, r, "/tags?msg="+neturl.QueryEscape(msg), http.StatusSeeOther)
}

func handleTags(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Redirect(w, r, "/tags", http.StatusSeeOther)
		return
	}
	_, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	renderTaxonomyPage(w, store, r.URL.Query().Get("msg"), learnMin(r))
}

func handleTaxonomy(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/tags", http.StatusSeeOther)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}
	action := r.URL.Path[len("/tags/"):]

	writeMu.Lock()
	defer writeMu.Unlock()

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	var msg string
	jent := &JournalEntry{}
	switch action {
	case "tag/rename":
		changed, err := renameTag(cfg, store, r.FormValue("old"), r.FormValue("new"))
		if err != nil {
			redirectTags(w, r, "Rename failed: "+err.Error())
			return
		}
		if len(changed) == 0 {
			redirectTags(w, r, "No bookmarks have that tag")
			return
		}
		jent = jent.merge(journalUpserts(changed))
		msg = fmt.Sprintf("Renamed tag on %d bookmark(s)", len(changed))
	case "tag/delete":
		changed, err := deleteTag(cfg, store, r.FormValue("name"))
		if err != nil {
			redirectTags(w, r, "Delete failed: "+err.Error())
			return
		}
		if len(changed) == 0 {
			redirectTags(w, r, "No bookmarks have that tag")
			return
		}
		jent = jent.merge(journalUpserts(changed))
		msg = fmt.Sprintf("Removed tag from %d bookmark(s)", len(changed))
	case "folder/rename":
		changed, err := renameFolder(cfg, store, r.FormValue("old"), r.FormValue("new"))
		if err != nil {
			redirectTags(w, r, "Rename failed: "+err.Error())
			return
		}
		if len(changed) == 0 {
			redirectTags(w, r, "No bookmarks are in that folder")
			return
		}
		jent = jent.merge(journalUpserts(changed))
		msg = fmt.Sprintf("Moved %d bookmark(s)", len(changed))
	case "folder/delete":
		changed, err := renameFolder(cfg, store, r.FormValue("name"), "")
		if err != nil {
			redirectTags(w, r, "Delete failed: "+err.Error())
			return
		}
		if len(changed) == 0 {
			redirectTags(w, r, "No bookmarks are in that folder")
			return
		}
		jent = jent.merge(journalUpserts(changed))
		msg = fmt.Sprintf("Moved %d bookmark(s) back to the root", len(changed))
	case "rule/learn":
		host, folder := r.FormValue("host"), r.FormValue("folder")
		rule, changed, err := createRule(cfg, store, "host:"+host, folder, nil)
		if err != nil {
			redirectTags(w, r, "Learn failed: "+err.Error())
			return
		}
		jent = jent.merge(journalUpserts(changed).merge(journalRules([]*AutoRule{rule})))
		msg = fmt.Sprintf("Added automation %s (applied to %d existing bookmark(s))", describeRule(rule), len(changed))
	case "rule/learn-all":
		min := 3
		if n, err := strconv.Atoi(strings.TrimSpace(r.FormValue("min"))); err == nil && n >= 2 {
			min = n
		}
		suggestions := suggestRules(store, min)
		if len(suggestions) == 0 {
			redirectTags(w, r, "No rule suggestions at this threshold.")
			return
		}
		made := 0
		for _, s := range suggestions {
			rule, changed, err := createRule(cfg, store, "host:"+s.host, s.folder, nil)
			if err != nil {
				continue
			}
			made++
			jent = jent.merge(journalUpserts(changed).merge(journalRules([]*AutoRule{rule})))
		}
		msg = fmt.Sprintf("Created %d automation rule(s).", made)
	default:
		http.NotFound(w, r)
		return
	}
	if err := saveWithJournal(cfg, store, jent); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	redirectTags(w, r, msg)
}

var taxTmpl = template.Must(template.New("taxonomy").Parse(`
<p><a href="/">&larr; back to search</a></p>
<h2>Tags and folders</h2>
{{if .Flash}}<p class="flash">{{.Flash}}</p>{{end}}

<h2>Suggested rules</h2>
<p class="count">Hosts that keep landing in one folder. Creating a rule files future matches automatically.</p>
<form method="get" action="/tags" class="stry">
  <label class="stry">threshold <input type="number" name="min" value="{{.Min}}" min="2" style="width:4rem"></label>
  <button type="submit">Apply</button>
</form>
{{if .Learn}}
<form method="post" action="/tags/rule/learn-all" class="stry">
  <input type="hidden" name="min" value="{{.Min}}">
  <button type="submit">Create all</button>
</form>
{{range .Learn}}
<div class="ruleform">
  <div>{{.Count}} bookmark(s) with host {{.Host}} are in folder {{.Folder}}</div>
  <form method="post" action="/tags/rule/learn" class="fields">
    <input type="hidden" name="host" value="{{.Host}}">
    <input type="hidden" name="folder" value="{{.Folder}}">
    <button type="submit">Create rule</button>
  </form>
</div>
{{end}}
{{end}}

<h2>Tags</h2>
<p class="count">Renaming onto an existing tag merges into it. Deleting removes the tag everywhere.</p>
{{if .Tags}}
{{range .Tags}}
<div class="ruleform">
  <div><span class="tag">#{{.Name}}</span> &middot; <span class="setdetect">{{.Count}} bookmark(s)</span></div>
  <form method="post" action="/tags/tag/rename" class="fields">
    <input type="hidden" name="old" value="{{.Name}}">
    <label>rename to <input type="text" name="new" required></label>
    <button type="submit">Save</button>
  </form>
  <form method="post" action="/tags/tag/delete" class="stry" onsubmit="return confirm('Delete this tag everywhere?');">
    <input type="hidden" name="name" value="{{.Name}}">
    <button type="submit" class="linklike">delete</button>
  </form>
</div>
{{end}}
{{else}}
<p class="count">No tags yet.</p>
{{end}}

<h2>Folders</h2>
<p class="count">Rename moves the folder and its subfolders. Deleting a folder moves its bookmarks back to the root.</p>
{{if .Folders}}
{{range .Folders}}
<div class="ruleform">
  <div>{{.Display}} &middot; <span class="setdetect">{{.Count}} bookmark(s)</span></div>
  <form method="post" action="/tags/folder/rename" class="fields">
    <input type="hidden" name="old" value="{{.Name}}">
    <label>rename to <input type="text" name="new" required></label>
    <button type="submit">Save</button>
  </form>
  <form method="post" action="/tags/folder/delete" class="stry" onsubmit="return confirm('Move this folder back to the root?');">
    <input type="hidden" name="name" value="{{.Name}}">
    <button type="submit" class="linklike">delete</button>
  </form>
</div>
{{end}}
{{else}}
<p class="count">No folders yet.</p>
{{end}}
`))
