package main

import (
	"bytes"
	"fmt"
	"html/template"
	"net/http"
	neturl "net/url"
	"sort"
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

func learnRows(store *Store) []learnRow {
	var out []learnRow
	for _, s := range suggestRules(store, 3) {
		out = append(out, learnRow{Host: s.host, Folder: s.folder, Count: s.count})
	}
	return out
}

func renderTaxonomyPage(w http.ResponseWriter, store *Store, flash string) {
	var buf bytes.Buffer
	folders := sortedTaxRows(folderCounts(store))
	for i := range folders {
		folders[i].Display = displayFolder(folders[i].Name)
	}
	if err := taxTmpl.Execute(&buf, taxonomyPageData{
		Flash:   flash,
		Tags:    sortedTaxRows(tagCounts(store)),
		Folders: folders,
		Learn:   learnRows(store),
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
	renderTaxonomyPage(w, store, r.URL.Query().Get("msg"))
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
	switch action {
	case "tag/rename":
		n, err := renameTag(cfg, store, r.FormValue("old"), r.FormValue("new"))
		if err != nil {
			redirectTags(w, r, "Rename failed: "+err.Error())
			return
		}
		if n == 0 {
			redirectTags(w, r, "No bookmarks have that tag")
			return
		}
		msg = fmt.Sprintf("Renamed tag on %d bookmark(s)", n)
	case "tag/delete":
		n, err := deleteTag(cfg, store, r.FormValue("name"))
		if err != nil {
			redirectTags(w, r, "Delete failed: "+err.Error())
			return
		}
		if n == 0 {
			redirectTags(w, r, "No bookmarks have that tag")
			return
		}
		msg = fmt.Sprintf("Removed tag from %d bookmark(s)", n)
	case "folder/rename":
		n, err := renameFolder(cfg, store, r.FormValue("old"), r.FormValue("new"))
		if err != nil {
			redirectTags(w, r, "Rename failed: "+err.Error())
			return
		}
		if n == 0 {
			redirectTags(w, r, "No bookmarks are in that folder")
			return
		}
		msg = fmt.Sprintf("Moved %d bookmark(s)", n)
	case "folder/delete":
		n, err := renameFolder(cfg, store, r.FormValue("name"), "")
		if err != nil {
			redirectTags(w, r, "Delete failed: "+err.Error())
			return
		}
		if n == 0 {
			redirectTags(w, r, "No bookmarks are in that folder")
			return
		}
		msg = fmt.Sprintf("Moved %d bookmark(s) back to the root", n)
	case "rule/learn":
		host, folder := r.FormValue("host"), r.FormValue("folder")
		rule, changed, err := createRule(cfg, store, "host:"+host, folder, nil)
		if err != nil {
			redirectTags(w, r, "Learn failed: "+err.Error())
			return
		}
		msg = fmt.Sprintf("Added automation %s (applied to %d existing bookmark(s))", describeRule(rule), changed)
	default:
		http.NotFound(w, r)
		return
	}
	if err := store.Save(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	redirectTags(w, r, msg)
}

var taxTmpl = template.Must(template.New("taxonomy").Parse(`
<p><a href="/">&larr; back to search</a></p>
<h2>Tags and folders</h2>
{{if .Flash}}<p class="flash">{{.Flash}}</p>{{end}}

{{if .Learn}}
<h2>Suggested rules</h2>
<p class="count">Hosts that keep landing in one folder. Creating a rule files future matches automatically.</p>
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
