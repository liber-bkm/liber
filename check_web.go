package main

import (
	"bytes"
	"fmt"
	"html/template"
	"net/http"
	"strconv"
	"strings"
	"time"
)

type checkFormData struct {
	Spec    string
	Workers string
	Stale   string
	Error   string
	Flash   string
}

type checkRowView struct {
	ID             int
	Title, URL     string
	Detail, Target string
	Status         string
}

type checkResultsData struct {
	Flash                  string
	OK, Checked, Fresh     int
	Missing                string
	Moved, Dead, Uncertain []checkRowView
	All                    []checkRowView
}

func toCheckRows(in []checkResult, status string) []checkRowView {
	out := make([]checkRowView, 0, len(in))
	for _, r := range in {
		out = append(out, checkRowView{
			ID: r.b.ID, Title: r.b.Title, URL: r.b.URL,
			Detail: r.detail, Target: r.target, Status: status,
		})
	}
	return out
}

func renderCheckForm(w http.ResponseWriter, data checkFormData) {
	var buf bytes.Buffer
	if err := checkFormTmpl.Execute(&buf, data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	layoutTmpl.Execute(w, struct {
		Title string
		Body  template.HTML
	}{"liber - check", template.HTML(buf.String())})
}

func renderCheckResults(w http.ResponseWriter, data checkResultsData) {
	var buf bytes.Buffer
	if err := checkResultsTmpl.Execute(&buf, data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	layoutTmpl.Execute(w, struct {
		Title string
		Body  template.HTML
	}{"liber - check results", template.HTML(buf.String())})
}

func handleCheck(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Redirect(w, r, "/check", http.StatusSeeOther)
		return
	}
	renderCheckForm(w, checkFormData{Workers: "12"})
}

func checkArgsFromForm(spec, workers, stale string) []string {
	var args []string
	if strings.TrimSpace(spec) != "" {
		args = append(args, strings.TrimSpace(spec))
	}
	if strings.TrimSpace(workers) != "" {
		args = append(args, "--workers", strings.TrimSpace(workers))
	}
	if strings.TrimSpace(stale) != "" {
		args = append(args, "--stale", strings.TrimSpace(stale))
	}
	return args
}

func handleCheckRun(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/check", http.StatusSeeOther)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}
	form := checkFormData{Spec: r.FormValue("spec"), Workers: r.FormValue("workers"), Stale: r.FormValue("stale")}
	if strings.TrimSpace(form.Workers) == "" {
		form.Workers = "12"
	}
	spec, workers, stale, _, err := parseCheckArgs(checkArgsFromForm(form.Spec, form.Workers, form.Stale))
	if err != nil {
		form.Error = err.Error()
		renderCheckForm(w, form)
		return
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	targets, missing, fresh, err := resolveCheckTargets(store, spec, stale)
	if err != nil {
		form.Error = err.Error()
		renderCheckForm(w, form)
		return
	}
	if len(targets) == 0 {
		form.Flash = "No bookmarks to check."
		renderCheckForm(w, form)
		return
	}

	moved, dead, uncertain := scanCheckTargets(checkClient(cfg), targets, workers, nil)
	now := time.Now()

	writeMu.Lock()
	_, freshStore, err := loadCfgAndStore()
	if err != nil {
		writeMu.Unlock()
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	stampCheckTargets(freshStore, targetIDs(targets), moved, dead, uncertain, now)
	if err := freshStore.Save(); err != nil {
		writeMu.Unlock()
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	writeMu.Unlock()

	movedRows := toCheckRows(moved, "moved")
	deadRows := toCheckRows(dead, "dead")
	uncertainRows := toCheckRows(uncertain, "uncertain")
	all := append(append(append([]checkRowView{}, movedRows...), deadRows...), uncertainRows...)
	renderCheckResults(w, checkResultsData{
		OK:      len(targets) - len(moved) - len(dead) - len(uncertain),
		Checked: len(targets), Fresh: fresh, Missing: joinInts(missing),
		Moved: movedRows, Dead: deadRows, Uncertain: uncertainRows, All: all,
	})
}

func parseCheckSnapshot(r *http.Request) []checkRowView {
	ids := r.Form["rid"]
	statuses := r.Form["rstatus"]
	details := r.Form["rdetail"]
	targets := r.Form["rtarget"]
	titles := r.Form["rtitle"]
	urls := r.Form["rurl"]
	n := len(ids)
	for _, l := range []int{len(statuses), len(details), len(targets), len(titles), len(urls)} {
		if l < n {
			n = l
		}
	}
	var out []checkRowView
	for i := 0; i < n; i++ {
		id, err := strconv.Atoi(strings.TrimSpace(ids[i]))
		if err != nil || id < 1 {
			continue
		}
		st := strings.TrimSpace(statuses[i])
		if st != "moved" && st != "dead" && st != "uncertain" {
			continue
		}
		out = append(out, checkRowView{
			ID: id, Title: titles[i], URL: urls[i],
			Detail: details[i], Target: targets[i], Status: st,
		})
	}
	return out
}

func handleCheckApply(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/check", http.StatusSeeOther)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}
	op := strings.TrimSpace(r.FormValue("op"))
	action, idStr, _ := strings.Cut(op, ":")
	id, _ := strconv.Atoi(strings.TrimSpace(idStr))
	rows := parseCheckSnapshot(r)
	ok, _ := strconv.Atoi(r.FormValue("rok"))
	checked, _ := strconv.Atoi(r.FormValue("rchecked"))
	fresh, _ := strconv.Atoi(r.FormValue("rfresh"))
	missing := r.FormValue("rmissing")

	var row *checkRowView
	for i := range rows {
		if rows[i].ID == id {
			row = &rows[i]
			break
		}
	}

	writeMu.Lock()
	defer writeMu.Unlock()

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	flash := ""
	if row == nil || id < 1 {
		flash = "Unknown item; nothing applied."
	} else if b := store.Find(id); b == nil {
		flash = fmt.Sprintf("[%d] already gone.", id)
	} else {
		b := store.Find(id)
		jent := &JournalEntry{}
		switch action {
		case "update":
			b.URL = normalizeURL(row.Target)
			b.UpdatedAt = time.Now()
			syncBookmarkFiles(cfg, b, false)
			jent = jent.merge(journalUpserts([]*Bookmark{b}))
			flash = fmt.Sprintf("Updated [%d].", id)
		case "retitle":
			b.URL = normalizeURL(row.Target)
			b.UpdatedAt = time.Now()
			syncBookmarkFiles(cfg, b, false)
			if title := fetchTitle(cfg, b.URL); title != "" && title != b.Title {
				b.Title = title
				b.UpdatedAt = time.Now()
				syncBookmarkFiles(cfg, b, false)
				flash = fmt.Sprintf("Updated [%d] and refreshed title.", id)
			} else {
				flash = fmt.Sprintf("Updated [%d] (title unchanged).", id)
			}
			jent = jent.merge(journalUpserts([]*Bookmark{b}))
		case "delete":
			jent = jent.merge(journalDeletes([]*Bookmark{b}))
			deleteBookmarkFiles(cfg, b)
			store.Delete(id)
			flash = fmt.Sprintf("Deleted [%d].", id)
		case "quarantine":
			quarantineBookmark(cfg, b)
			jent = jent.merge(journalUpserts([]*Bookmark{b}))
			flash = fmt.Sprintf("Quarantined [%d].", id)
		default:
			flash = "Unknown action; nothing applied."
			renderRemainingCheck(w, rows, id, flash, ok, checked, fresh, missing)
			return
		}
		if err := saveWithJournal(cfg, store, jent); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	}
	renderRemainingCheck(w, rows, id, flash, ok, checked, fresh, missing)
}

func renderRemainingCheck(w http.ResponseWriter, rows []checkRowView, done int, flash string, ok, checked, fresh int, missing string) {
	var remaining []checkRowView
	for _, row := range rows {
		if row.ID != done {
			remaining = append(remaining, row)
		}
	}
	var moved, dead, uncertain []checkRowView
	for _, row := range remaining {
		switch row.Status {
		case "moved":
			moved = append(moved, row)
		case "dead":
			dead = append(dead, row)
		case "uncertain":
			uncertain = append(uncertain, row)
		}
	}
	renderCheckResults(w, checkResultsData{
		Flash: flash, OK: ok, Checked: checked, Fresh: fresh, Missing: missing,
		Moved: moved, Dead: dead, Uncertain: uncertain, All: remaining,
	})
}

var checkFormTmpl = template.Must(template.New("checkForm").Parse(`
<p><a href="/">&larr; back to search</a></p>
<h2>Check link health</h2>
<p class="count">Scans bookmarks and reports moved, dead, and uncertain links. Same buckets as liber --check.</p>
{{if .Error}}<p class="flash error">{{.Error}}</p>{{end}}
{{if .Flash}}<p class="flash">{{.Flash}}</p>{{end}}
<form method="post" action="/check/run" class="addform">
  <input type="text" name="spec" placeholder="ids like 1-100,200 (empty = all)" value="{{.Spec}}">
  <label>workers<br><input type="number" name="workers" value="{{.Workers}}" min="1"></label>
  <input type="text" name="stale" placeholder="only recheck stale, e.g. 720h (empty = all)" value="{{.Stale}}">
  <button type="submit">Run check</button>
</form>
`))

var checkResultsTmpl = template.Must(template.New("checkResults").Parse(`
<p><a href="/">&larr; back to search</a> &middot; <a href="/check">new scan</a></p>
<h2>Check results</h2>
{{if .Flash}}<p class="flash">{{.Flash}}</p>{{end}}
<p class="count">{{.OK}} ok, {{len .Moved}} moved, {{len .Dead}} dead, {{len .Uncertain}} uncertain (of {{.Checked}} checked){{if .Fresh}} &middot; skipped {{.Fresh}} fresh{{end}}{{if .Missing}} &middot; missing {{.Missing}}{{end}}</p>
{{if .All}}
<form method="post" action="/check/apply">
  {{range .All}}
  <input type="hidden" name="rid" value="{{.ID}}">
  <input type="hidden" name="rtitle" value="{{.Title}}">
  <input type="hidden" name="rurl" value="{{.URL}}">
  <input type="hidden" name="rstatus" value="{{.Status}}">
  <input type="hidden" name="rdetail" value="{{.Detail}}">
  <input type="hidden" name="rtarget" value="{{.Target}}">
  {{end}}
  <input type="hidden" name="rok" value="{{.OK}}">
  <input type="hidden" name="rchecked" value="{{.Checked}}">
  <input type="hidden" name="rfresh" value="{{.Fresh}}">
  <input type="hidden" name="rmissing" value="{{.Missing}}">
  {{if .Moved}}
  <h2>Moved</h2>
  {{range .Moved}}
  <div class="ruleform">
    <div>[{{.ID}}] {{.Title}} &middot; <span class="setdetect">{{.Detail}} &rarr; {{.Target}}</span></div>
    <div class="meta">{{.URL}}</div>
    <div class="stry">
      <button type="submit" name="op" value="update:{{.ID}}">Update URL</button>
      <button type="submit" name="op" value="retitle:{{.ID}}">Update URL + title</button>
    </div>
  </div>
  {{end}}
  {{end}}
  {{if .Dead}}
  <h2>Dead</h2>
  {{range .Dead}}
  <div class="ruleform">
    <div>[{{.ID}}] {{.Title}} &middot; <span class="setdetect">{{.Detail}}</span></div>
    <div class="meta">{{.URL}}</div>
    <div class="stry">
      <button type="submit" name="op" value="delete:{{.ID}}" onclick="return confirm('Delete [{{.ID}}] {{.Title}}?');">Delete</button>
      <button type="submit" name="op" value="quarantine:{{.ID}}">Quarantine</button>
    </div>
  </div>
  {{end}}
  {{end}}
  {{if .Uncertain}}
  <h2>Uncertain</h2>
  {{range .Uncertain}}
  <div class="ruleform">
    <div>[{{.ID}}] {{.Title}} &middot; <span class="setdetect">{{.Detail}}</span></div>
    <div class="meta">{{.URL}}</div>
    <div class="stry">
      <button type="submit" name="op" value="delete:{{.ID}}" onclick="return confirm('Delete [{{.ID}}] {{.Title}}?');">Delete</button>
      <button type="submit" name="op" value="quarantine:{{.ID}}">Quarantine</button>
    </div>
  </div>
  {{end}}
  {{end}}
</form>
{{else}}
<p class="count">Nothing left to review.</p>
{{end}}
`))
