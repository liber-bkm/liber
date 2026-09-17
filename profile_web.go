package main

import (
	"bytes"
	"html/template"
	"net/http"
	neturl "net/url"
)

func renderProfilesPage(w http.ResponseWriter, entries []profileEntry, flash string) {
	var buf bytes.Buffer
	if err := profilesTmpl.Execute(&buf, struct {
		Flash   string
		Entries []profileEntry
	}{flash, entries}); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	layoutTmpl.Execute(w, struct {
		Title string
		Body  template.HTML
	}{"liber profiles", template.HTML(buf.String())})
}

func redirectProfiles(w http.ResponseWriter, r *http.Request, msg string) {
	http.Redirect(w, r, "/profiles?msg="+neturl.QueryEscape(msg), http.StatusSeeOther)
}

func handleProfiles(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Redirect(w, r, "/profiles", http.StatusSeeOther)
		return
	}
	entries, err := profileListData()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	renderProfilesPage(w, entries, r.URL.Query().Get("msg"))
}

func handleProfileSwitch(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/profiles", http.StatusSeeOther)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}
	writeMu.Lock()
	defer writeMu.Unlock()
	msg, err := profileSwitchMsg(r.FormValue("name"))
	if err != nil {
		redirectProfiles(w, r, "Switch failed: "+err.Error())
		return
	}
	redirectProfiles(w, r, msg)
}

func handleProfileDelete(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/profiles", http.StatusSeeOther)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}
	writeMu.Lock()
	defer writeMu.Unlock()
	msg, err := profileDeleteMsg(r.FormValue("name"))
	if err != nil {
		redirectProfiles(w, r, "Delete failed: "+err.Error())
		return
	}
	redirectProfiles(w, r, msg)
}

var profilesTmpl = template.Must(template.New("profiles").Parse(`
<p><a href="/">&larr; back to search</a> &middot; <a href="/settings">settings</a></p>
<h2>Profiles</h2>
<p class="count">Each profile is a fully independent collection. Switching takes effect immediately.</p>
{{if .Flash}}<p class="flash">{{.Flash}}</p>{{end}}
{{range .Entries}}
<div class="ruleform">
  <div>{{if .Active}}<strong>{{.Name}} (active)</strong>{{else}}{{.Name}}{{end}} &middot; <span class="setdetect">{{.Path}}</span></div>
  {{if not .Active}}
  <form method="post" action="/profiles/switch" class="stry">
    <input type="hidden" name="name" value="{{.Name}}">
    <button type="submit">Switch</button>
  </form>
  {{if not .Default}}
  <form method="post" action="/profiles/delete" class="stry" onsubmit="return confirm('Stop tracking this profile? Its folder and bookmarks stay on disk.');">
    <input type="hidden" name="name" value="{{.Name}}">
    <button type="submit" class="linklike">delete</button>
  </form>
  {{end}}
  {{end}}
</div>
{{end}}
<details class="ruleform">
  <summary>New profile</summary>
  <form method="post" action="/profiles/switch" class="fields">
    <label>name <input type="text" name="name" required></label>
    <button type="submit">Create and switch</button>
  </form>
</details>
`))
