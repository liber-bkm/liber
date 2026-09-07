package main

import (
	"bytes"
	"fmt"
	"html/template"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
)

type settingField struct {
	Key      string
	Label    string
	Value    string // raw config value (empty = default)
	Detected string // resolved executable/effective path or "not found"
	Note     string
}

type ruleRow struct {
	ID     int
	Label  string
	Match  string
	Folder string
	Tags   string
	Count  int
}

type settingsPageData struct {
	Flash              string
	Tools              []settingField
	Dirs               []settingField
	ArchiveBackend     string
	MonolithUseBrowser bool
	Rules              []ruleRow
	Path               string // config file path, displayed to the user
}

func probe(names ...string) (string, bool) {
	for _, n := range names {
		if p, err := exec.LookPath(n); err == nil {
			return p, true
		}
	}
	return "", false
}

func probePath(names ...string) string {
	p, _ := probe(names...)
	if p == "" {
		return "not found on PATH"
	}
	return p + " (auto)"
}

func browserDefault() string { // mirrors openURL's per-OS command
	switch runtime.GOOS {
	case "darwin":
		return "open"
	case "windows":
		return "rundll32"
	default:
		return "xdg-open"
	}
}

func settingsData(cfg Config, cfgPath string, store *Store, flash string) settingsPageData {
	detect := func(current, fallback string, names ...string) string {
		if current != "" {
			if p, err := exec.LookPath(current); err == nil {
				return p + " (from config)"
			}
			return "not found: " + current
		}
		names = append([]string{fallback}, names...)
		if p, ok := probe(names...); ok {
			return p + " (default)"
		}
		return "not found on PATH"
	}

	tools := []settingField{
		{Key: "singlefile_cmd", Label: "single-file command", Value: cfg.SingleFileCmd,
			Detected: detect(cfg.SingleFileCmd, "single-file")},
		{Key: "singlefile_browser_path", Label: "single-file browser path", Value: cfg.SingleFileBrowserPath,
			Detected: "auto (single-file finds the browser itself)"},
		{Key: "monolith_cmd", Label: "monolith command", Value: cfg.MonolithCmd,
			Detected: detect(cfg.MonolithCmd, "monolith")},
		{Key: "monolith_browser_path", Label: "monolith browser path", Value: cfg.MonolithBrowserPath,
			Detected: probePath("chromium", "chromium-browser", "google-chrome")},
		{Key: "browser_cmd", Label: "open command", Value: cfg.BrowserCmd,
			Detected: detect(cfg.BrowserCmd, browserDefault())},
		{Key: "editor_cmd", Label: "editor command", Value: cfg.EditorCmd, Note: "$VISUAL / $EDITOR are used when unset",
			Detected: func() string {
				if v := os.Getenv("VISUAL"); v != "" {
					return v + " ($VISUAL)"
				}
				if v := os.Getenv("EDITOR"); v != "" {
					return v + " ($EDITOR)"
				}
				return "none set"
			}()},
	}

	dirs := []settingField{
		{Key: "base_dir", Label: "base directory", Value: cfg.BaseDir, Detected: cfg.effectiveBaseDir(),
			Note: "with an active profile this resolves to <base_dir>/<profile>; changing base_dir points liber at a different collection (nothing is moved)"},
		{Key: "html_dir", Label: "html dir", Value: cfg.HTMLDir, Detected: cfg.htmlDir()},
		{Key: "markdown_dir", Label: "markdown dir", Value: cfg.MarkdownDir, Detected: cfg.markdownDir()},
		{Key: "archive_dir", Label: "archive dir", Value: cfg.ArchiveDir, Detected: cfg.archiveDir()},
		{Key: "attachment_dir", Label: "attachment dir", Value: cfg.AttachmentDir, Detected: cfg.attachmentsDir()},
	}

	backend := cfg.ArchiveBackend
	if backend == "" {
		backend = "auto"
	}

	counts := map[int]int{}
	for _, b := range store.Bookmarks {
		for _, a := range b.AppliedRules {
			counts[a.RuleID]++
		}
	}
	var rules []ruleRow
	for _, r := range store.AutoRules {
		rules = append(rules, ruleRow{
			ID: r.ID, Label: describeRule(r), Match: r.Match,
			Folder: r.Folder, Tags: strings.Join(r.Tags, " "), Count: counts[r.ID],
		})
	}

	return settingsPageData{
		Flash: flash, Tools: tools, Dirs: dirs,
		ArchiveBackend: backend, MonolithUseBrowser: cfg.MonolithUseBrowser,
		Rules: rules, Path: cfgPath,
	}
}

func renderSettingsPage(w http.ResponseWriter, cfg Config, cfgPath string, store *Store, flash string) {
	var buf bytes.Buffer
	if err := settingsTmpl.Execute(&buf, settingsData(cfg, cfgPath, store, flash)); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	layoutTmpl.Execute(w, struct {
		Title string
		Body  template.HTML
	}{"liber settings", template.HTML(buf.String())})
}

func redirectSettings(w http.ResponseWriter, r *http.Request, msg string) {
	http.Redirect(w, r, "/settings?msg="+url.QueryEscape(msg), http.StatusSeeOther)
}

func handleSettings(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodPost {
		handleSettingsSave(w, r)
		return
	}
	cfg, cfgPath, err := LoadConfig()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	_, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	renderSettingsPage(w, cfg, cfgPath, store, r.URL.Query().Get("msg"))
}

func handleSettingsSave(w http.ResponseWriter, r *http.Request) {
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}

	writeMu.Lock()
	defer writeMu.Unlock()

	cfg, cfgPath, err := LoadConfig()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	cfg.BaseDir = strings.TrimSpace(r.FormValue("base_dir"))
	cfg.HTMLDir = strings.TrimSpace(r.FormValue("html_dir"))
	cfg.MarkdownDir = strings.TrimSpace(r.FormValue("markdown_dir"))
	cfg.ArchiveDir = strings.TrimSpace(r.FormValue("archive_dir"))
	cfg.AttachmentDir = strings.TrimSpace(r.FormValue("attachment_dir"))
	cfg.SingleFileCmd = strings.TrimSpace(r.FormValue("singlefile_cmd"))
	cfg.SingleFileBrowserPath = strings.TrimSpace(r.FormValue("singlefile_browser_path"))
	cfg.MonolithCmd = strings.TrimSpace(r.FormValue("monolith_cmd"))
	cfg.MonolithBrowserPath = strings.TrimSpace(r.FormValue("monolith_browser_path"))
	cfg.BrowserCmd = strings.TrimSpace(r.FormValue("browser_cmd"))
	cfg.EditorCmd = strings.TrimSpace(r.FormValue("editor_cmd"))
	cfg.MonolithUseBrowser = r.FormValue("monolith_use_browser") == "on"

	switch b := strings.TrimSpace(r.FormValue("archive_backend")); b {
	case "", "auto", "single-file", "monolith", "native":
		cfg.ArchiveBackend = b
	default:
		_, store, _ := loadCfgAndStore()
		renderSettingsPage(w, cfg, cfgPath, store, fmt.Sprintf("unknown archive_backend %q", b))
		return
	}

	if err := SaveConfig(cfg); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	redirectSettings(w, r, "Settings saved")
}

func handleSettingsAuto(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings", http.StatusSeeOther)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}
	action := strings.TrimPrefix(r.URL.Path, "/settings/auto/")

	writeMu.Lock()
	defer writeMu.Unlock()

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	switch action {
	case "add":
		_, changed, err := createRule(cfg, store, r.FormValue("match"),
			r.FormValue("folder"), strings.Fields(r.FormValue("tags")))
		if err != nil {
			redirectSettings(w, r, "Add failed: "+err.Error())
			return
		}
		if err := store.Save(); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		redirectSettings(w, r, fmt.Sprintf("Rule added (applied to %d existing bookmark(s))", changed))

	case "delete":
		id, _ := strconv.Atoi(r.FormValue("id"))
		if !store.DeleteAutoRule(id) {
			redirectSettings(w, r, "No rule with that id")
			return
		}
		if err := store.Save(); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		redirectSettings(w, r, "Rule deleted (bookmarks it classified are untouched)")

	case "edit":
		id, _ := strconv.Atoi(r.FormValue("id"))
		_, changed, err := editRule(cfg, store, id, r.FormValue("match"), r.FormValue("folder"),
			strings.Fields(r.FormValue("tags")), r.FormValue("reapply") == "on")
		if err != nil {
			redirectSettings(w, r, "Edit failed: "+err.Error())
			return
		}
		if err := store.Save(); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		msg := "Rule updated"
		if r.FormValue("reapply") == "on" {
			msg += fmt.Sprintf(" (reapplied to %d bookmark(s))", changed)
		}
		redirectSettings(w, r, msg)

	case "apply":
		id, _ := strconv.Atoi(r.FormValue("id"))
		changed, err := applyRules(cfg, store, id)
		if err != nil {
			redirectSettings(w, r, err.Error())
			return
		}
		if err := store.Save(); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		redirectSettings(w, r, fmt.Sprintf("Applied to %d bookmark(s)", changed))

	default:
		http.NotFound(w, r)
	}
}

var settingsTmpl = template.Must(template.New("settings").Parse(`
<p><a href="/">&larr; back to search</a></p>
<h2>Settings</h2>
<p class="count">Config file: {{.Path}}</p>
{{if .Flash}}<p class="flash">{{.Flash}}</p>{{end}}

<h2>Tools</h2>
<p class="count">Empty box = use the default. Detection shows what liber would resolve to right now.</p>
<form method="post" action="/settings" class="setform">
  {{range .Tools}}
  <label for="{{.Key}}">{{.Label}}</label>
  <div>
    <input type="text" name="{{.Key}}" id="{{.Key}}" value="{{.Value}}">
    <div class="setdetect">detected: {{.Detected}}{{if .Note}} &middot; {{.Note}}{{end}}</div>
  </div>
  {{end}}

  <h2 style="grid-column: 1 / -1">Directories</h2>
  {{range .Dirs}}
  <label for="{{.Key}}">{{.Label}}</label>
  <div>
    <input type="text" name="{{.Key}}" id="{{.Key}}" value="{{.Value}}">
    <div class="setdetect">effective: {{.Detected}}{{if .Note}} &middot; {{.Note}}{{end}}</div>
  </div>
  {{end}}

  <h2 style="grid-column: 1 / -1">Archiving</h2>
  <label for="archive_backend">archive backend</label>
  <div>
    <select name="archive_backend" id="archive_backend">
      {{with .ArchiveBackend}}
      <option value="auto" {{if eq . "auto"}}selected{{end}}>auto (single-file, then monolith, then native)</option>
      <option value="single-file" {{if eq . "single-file"}}selected{{end}}>single-file</option>
      <option value="monolith" {{if eq . "monolith"}}selected{{end}}>monolith</option>
      <option value="native" {{if eq . "native"}}selected{{end}}>native (built-in static snapshot)</option>
      {{end}}
    </select>
  </div>
  <label for="monolith_use_browser">use browser pipe for monolith</label>
  <div><input type="checkbox" name="monolith_use_browser" id="monolith_use_browser" {{if .MonolithUseBrowser}}checked{{end}}></div>

  <div></div>
  <div><button type="submit">Save settings</button></div>
</form>

<h2>Automation rules</h2>
<details class="ruleform">
  <summary>Add a rule</summary>
  <form method="post" action="/settings/auto/add" class="fields">
    <label>match <input type="text" name="match" placeholder="github.com / host:x / title:y" required></label>
    <label>folder <input type="text" name="folder"></label>
    <label>tags <input type="text" name="tags" placeholder="space separated"></label>
    <button type="submit">Add</button>
  </form>
</details>

{{range .Rules}}
<div class="ruleform">
  <div>{{.Label}} &middot; <span class="setdetect">applied to {{.Count}} bookmark(s)</span></div>
  <form method="post" action="/settings/auto/{{"edit"}}" class="fields">
    <input type="hidden" name="id" value="{{.ID}}">
    <label>match <input type="text" name="match" value="{{.Match}}"></label>
    <label>folder <input type="text" name="folder" value="{{.Folder}}"></label>
    <label>tags <input type="text" name="tags" value="{{.Tags}}"></label>
    <label class="stry"><input type="checkbox" name="reapply"> reapply</label>
    <button type="submit">Save</button>
  </form>
  <form method="post" action="/settings/auto/delete" class="stry" onsubmit="return confirm('Delete this rule?');">
    <input type="hidden" name="id" value="{{.ID}}">
    <button type="submit" class="linklike">delete</button>
  </form>
</div>
{{end}}
{{if .Rules}}
<form method="post" action="/settings/auto/apply" class="stry">
  <button type="submit">Re-run all rules against existing bookmarks</button>
</form>
{{else}}
<p class="count">No automation rules yet.</p>
{{end}}
`))
