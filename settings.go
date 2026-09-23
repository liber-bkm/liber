package main

import (
	"bytes"
	"fmt"
	"html/template"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
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
	DeviceID           string
	ActiveProfile      string
	IsAndroid          bool
	MaintenanceStatus  string
	ReindexOutput      string
	SyncOutput         string
	LibraryOutput      string
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

	backend := cfg.effectiveArchiveBackend()

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
		Rules: rules, Path: cfgPath, MaintenanceStatus: maintenanceStatus(cfg, store),
		DeviceID: cfg.DeviceID, ActiveProfile: cfg.ActiveProfile,
		IsAndroid: runtime.GOOS == "android",
	}
}

func maintenanceStatus(cfg Config, store *Store) string {
	var parts []string
	liberDir := filepath.Join(cfg.effectiveBaseDir(), ".liber")
	if n := len(findConflictCopies(liberDir)); n > 0 {
		parts = append(parts, fmt.Sprintf("%d conflict cop%s", n, entrySuffix(n)))
	}
	if all := len(findMergeCandidates(liberDir, true)); all > len(findConflictCopies(liberDir)) {
		parts = append(parts, fmt.Sprintf("%d other json candidate(s)", all-len(findConflictCopies(liberDir))))
	}
	if n := unappliedJournalCount(cfg, store); n > 0 {
		parts = append(parts, fmt.Sprintf("%d unapplied journal entr%s", n, entrySuffix(n)))
	}
	pending := 0
	for _, b := range store.Bookmarks {
		if b.HTMLFile == "" || !fileExists(filepath.Join(cfg.htmlDir(), b.HTMLFile)) {
			pending++
		}
	}
	if pending > 0 {
		parts = append(parts, fmt.Sprintf("%d pending entr%s", pending, entrySuffix(pending)))
	}
	if len(parts) == 0 {
		return "index matches disk: nothing to merge, apply, or prune."
	}
	return strings.Join(parts, ", ") + "."
}

func renderSettingsPage(w http.ResponseWriter, cfg Config, cfgPath string, store *Store, flash string) {
	renderSettingsPageWithOutput(w, cfg, cfgPath, store, flash, "")
}

func renderSettingsPageWithOutput(w http.ResponseWriter, cfg Config, cfgPath string, store *Store, flash, output string) {
	data := settingsData(cfg, cfgPath, store, flash)
	data.ReindexOutput = output
	renderSettingsData(w, data)
}

func renderSettingsData(w http.ResponseWriter, data settingsPageData) {
	var buf bytes.Buffer
	if err := settingsTmpl.Execute(&buf, data); err != nil {
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
	set := func(key string, dst *string) {
		if vals, ok := r.Form[key]; ok && len(vals) > 0 {
			*dst = strings.TrimSpace(vals[0])
		}
	}
	set("base_dir", &cfg.BaseDir)
	set("html_dir", &cfg.HTMLDir)
	set("markdown_dir", &cfg.MarkdownDir)
	set("archive_dir", &cfg.ArchiveDir)
	set("attachment_dir", &cfg.AttachmentDir)
	set("singlefile_cmd", &cfg.SingleFileCmd)
	set("singlefile_browser_path", &cfg.SingleFileBrowserPath)
	set("monolith_cmd", &cfg.MonolithCmd)
	set("monolith_browser_path", &cfg.MonolithBrowserPath)
	set("browser_cmd", &cfg.BrowserCmd)
	set("editor_cmd", &cfg.EditorCmd)
	if _, ok := r.Form["monolith_use_browser"]; ok {
		cfg.MonolithUseBrowser = r.FormValue("monolith_use_browser") == "on"
	}
	if v := strings.TrimSpace(r.FormValue("device_id")); v != "" {
		cfg.DeviceID = sanitizeDevice(v)
	}

	if _, ok := r.Form["archive_backend"]; ok {
		switch b := strings.TrimSpace(r.FormValue("archive_backend")); b {
		case "", "auto", "single-file", "monolith", "native":
			cfg.ArchiveBackend = b
		default:
			_, store, _ := loadCfgAndStore()
			renderSettingsPage(w, cfg, cfgPath, store, fmt.Sprintf("unknown archive_backend %q", b))
			return
		}
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
		rule, changed, err := createRule(cfg, store, r.FormValue("match"),
			r.FormValue("folder"), strings.Fields(r.FormValue("tags")))
		if err != nil {
			redirectSettings(w, r, "Add failed: "+err.Error())
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed).merge(journalRules([]*AutoRule{rule}))); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		redirectSettings(w, r, fmt.Sprintf("Rule added (applied to %d existing bookmark(s))", len(changed)))

	case "delete":
		id, _ := strconv.Atoi(r.FormValue("id"))
		del := store.FindAutoRule(id)
		if del == nil {
			redirectSettings(w, r, "No rule with that id")
			return
		}
		tomb := journalRuleDeletes([]*AutoRule{del})
		store.DeleteAutoRule(id)
		if err := saveWithJournal(cfg, store, tomb); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		redirectSettings(w, r, "Rule deleted (bookmarks it classified are untouched)")

	case "edit":
		id, _ := strconv.Atoi(r.FormValue("id"))
		rule, changed, err := editRule(cfg, store, id, r.FormValue("match"), r.FormValue("folder"),
			strings.Fields(r.FormValue("tags")), r.FormValue("reapply") == "on")
		if err != nil {
			redirectSettings(w, r, "Edit failed: "+err.Error())
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed).merge(journalRules([]*AutoRule{rule}))); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		msg := "Rule updated"
		if r.FormValue("reapply") == "on" {
			msg += fmt.Sprintf(" (reapplied to %d bookmark(s))", len(changed))
		}
		redirectSettings(w, r, msg)

	case "apply":
		id, _ := strconv.Atoi(r.FormValue("id"))
		changed, err := applyRules(cfg, store, id)
		if err != nil {
			redirectSettings(w, r, err.Error())
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed)); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		redirectSettings(w, r, fmt.Sprintf("Applied to %d bookmark(s)", len(changed)))

	default:
		http.NotFound(w, r)
	}
}

func handleSettingsReindex(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings", http.StatusSeeOther)
		return
	}
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
	_, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	f := reindexFlags{
		merge:        r.FormValue("merge") == "on",
		all:          r.FormValue("all") == "on",
		prune:        r.FormValue("prune") == "on",
		compact:      r.FormValue("compact") == "on",
		pruneJournal: r.FormValue("prune-journal") == "on",
	}
	if f.all && !f.merge {
		renderSettingsPageWithOutput(w, cfg, cfgPath, store, "Reindex failed", "--all requires --merge\n")
		return
	}

	var buf strings.Builder
	if err := runReindexWith(&buf, cfg, store, f); err != nil {
		renderSettingsPageWithOutput(w, cfg, cfgPath, store, "Reindex failed", buf.String()+"error: "+err.Error()+"\n")
		return
	}
	renderSettingsPageWithOutput(w, cfg, cfgPath, store, "Reindex done", buf.String())
}

func handleSettingsImport(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings", http.StatusSeeOther)
		return
	}
	if err := parseWebForm(r); err != nil {
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
	_, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	flash, output := importUploadedFile(cfg, store, r)
	data := settingsData(cfg, cfgPath, store, flash)
	data.LibraryOutput = output
	renderSettingsData(w, data)
}

func importUploadedFile(cfg Config, store *Store, r *http.Request) (flash, output string) {
	if r.MultipartForm == nil {
		return "Import failed", "no file uploaded\n"
	}
	fhs := r.MultipartForm.File["bookmark_file"]
	if len(fhs) == 0 {
		return "Import failed", "no file uploaded\n"
	}
	f, err := fhs[0].Open()
	if err != nil {
		return "Import failed", "error: " + err.Error() + "\n"
	}
	defer f.Close()
	defer r.MultipartForm.RemoveAll()
	data, err := io.ReadAll(io.LimitReader(f, 32<<20))
	if err != nil {
		return "Import failed", "error: " + err.Error() + "\n"
	}
	opt := importOptions{Markdown: r.FormValue("markdown") == "on", Archive: r.FormValue("archive") == "on"}
	added, skippedDup, skippedBad, warnings := importData(cfg, store, data, opt)
	if len(added) == 0 && skippedDup == 0 && skippedBad == 0 {
		return "Import done", "No bookmarks found in that file -- is it a browser bookmark export?\n"
	}
	if err := saveWithJournal(cfg, store, journalUpserts(added)); err != nil {
		return "Import failed", "error: " + err.Error() + "\n"
	}
	var buf strings.Builder
	for _, w := range warnings {
		buf.WriteString(w + "\n")
	}
	fmt.Fprintf(&buf, "Imported %d bookmark(s).\n", len(added))
	if skippedDup > 0 {
		fmt.Fprintf(&buf, "Skipped %d already in your collection.\n", skippedDup)
	}
	if skippedBad > 0 {
		fmt.Fprintf(&buf, "Skipped %d entr%s with no URL.\n", skippedBad, entrySuffix(skippedBad))
	}
	return "Import done", buf.String()
}

func handleSettingsExport(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings", http.StatusSeeOther)
		return
	}
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
	_, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	outDir := strings.TrimSpace(r.FormValue("dir"))
	if outDir == "" {
		outDir = filepath.Join(cfg.effectiveBaseDir(), "site")
	}
	outDir = expandTilde(outDir)
	out, err := doExportSite(cfg, store, outDir)
	data := settingsData(cfg, cfgPath, store, "Export done")
	if err != nil {
		data = settingsData(cfg, cfgPath, store, "Export failed")
		data.LibraryOutput = "error: " + err.Error() + "\n"
	} else {
		data.LibraryOutput = fmt.Sprintf("Exported %d bookmark(s) to %s\n", len(store.Bookmarks), out)
	}
	renderSettingsData(w, data)
}

func handleSettingsSync(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings", http.StatusSeeOther)
		return
	}
	if err := r.ParseForm(); err != nil {
		http.Error(w, "bad form", http.StatusBadRequest)
		return
	}

	writeMu.Lock()
	defer writeMu.Unlock()

	_, cfgPath, err := LoadConfig()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	var buf strings.Builder
	push := r.FormValue("push") == "on"
	if err := runSyncTo(&buf, push); err != nil {
		data := settingsData(cfg, cfgPath, store, "Sync failed")
		data.SyncOutput = buf.String() + "error: " + err.Error() + "\n"
		renderSettingsData(w, data)
		return
	}
	data := settingsData(cfg, cfgPath, store, "Sync done")
	data.SyncOutput = buf.String()
	renderSettingsData(w, data)
}

var settingsTmpl = template.Must(template.New("settings").Parse(`
<p><a href="/">&larr; back to search</a></p>
<h2>Settings</h2>
<p class="count">Config file: {{.Path}}{{if .IsAndroid}} &middot; app-private storage, not directly editable: manage it here{{end}}</p>
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
  {{if .IsAndroid}}<p class="count" style="grid-column: 1 / -1">On Android only the native snapshot is available: single-file and monolith need binaries that do not exist on-device. Leave the backend empty for the native default.</p>{{end}}
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

  <h2 style="grid-column: 1 / -1">Sync</h2>
  <label for="device_id">device id</label>
  <div>
    <input type="text" name="device_id" id="device_id" value="{{.DeviceID}}">
    <div class="setdetect">effective: {{if .DeviceID}}{{.DeviceID}}{{else}}(generated on first write){{end}} &middot; active profile: {{if .ActiveProfile}}{{.ActiveProfile}}{{else}}default{{end}}</div>
  </div>
  <label>profiles</label>
  <div><a href="/profiles">manage profiles</a> <span class="setdetect">switch, create, or delete collections</span></div>

  <div></div>
  <div><button type="submit" class="primary">Save settings</button></div>
</form>

<h2>Sync</h2>
<p class="count">Commit the collection with jj or git, the same thing liber --sync does. Needs a repo at or above the base dir.</p>
<form method="post" action="/settings/sync" class="stry">
  <label class="stry"><input type="checkbox" name="push"> push</label>
  <button type="submit">Run sync</button>
</form>
{{if .SyncOutput}}<pre class="reindexout">{{.SyncOutput}}</pre>{{end}}

<h2>Library</h2>
<p class="count">Import a browser bookmark export, write a static site of the whole collection, download a portable bookmark export, or <a href="/check">check link health</a>.</p>
<form method="post" action="/settings/import" class="stry" enctype="multipart/form-data">
  <input type="file" name="bookmark_file" required>
  <label class="stry"><input type="checkbox" name="markdown"> markdown</label>
  <label class="stry"><input type="checkbox" name="archive"> archive</label>
  <button type="submit">Import</button>
</form>
<form method="post" action="/settings/export" class="stry">
  <input type="text" name="dir" placeholder="output dir (default <base_dir>/site)" size="40">
  <button type="submit">Export site</button>
</form>
<p class="count"><a href="/export-bookmarks">Download browser export</a> <span class="setdetect">Netscape HTML, re-importable anywhere including a fresh liber</span></p>
{{if .LibraryOutput}}<pre class="reindexout">{{.LibraryOutput}}</pre>{{end}}

<h2>Maintenance</h2>
<p class="count">{{.MaintenanceStatus}} Only prune or compact on a fully synced collection.</p>
<form method="post" action="/settings/reindex" class="stry" onsubmit="return confirmReindex();">
  <label class="stry"><input type="checkbox" name="merge" id="rx-merge" checked> merge</label>
  <label class="stry"><input type="checkbox" name="all" id="rx-all"> all</label>
  <label class="stry"><input type="checkbox" name="prune" id="rx-prune"> prune</label>
  <label class="stry"><input type="checkbox" name="compact" id="rx-compact"> compact</label>
  <label class="stry"><input type="checkbox" name="prune-journal" id="rx-prune-journal"> prune journal</label>
  <button type="submit">Run reindex</button>
</form>
<script>
function confirmReindex() {
  if (document.getElementById("rx-all").checked && !document.getElementById("rx-merge").checked) {
    alert("--all requires --merge.");
    return false;
  }
  if (document.getElementById("rx-prune").checked &&
      !confirm("Prune drops pending entries. Only do this on a fully synced collection. Continue?")) {
    return false;
  }
  if (document.getElementById("rx-compact").checked &&
      !confirm("Compact renames bookmark files to close id gaps. Continue?")) {
    return false;
  }
  return true;
}
</script>
{{if .ReindexOutput}}<pre class="reindexout">{{.ReindexOutput}}</pre>{{end}}

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
  <form method="post" action="/settings/auto/apply" class="stry">
    <input type="hidden" name="id" value="{{.ID}}">
    <button type="submit" class="linklike neutral">apply</button>
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
