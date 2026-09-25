package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

const apiMaxBody = 1 << 20

type apiAttachment struct {
	Name string `json:"name"`
}

type apiBookmark struct {
	ID          int             `json:"id"`
	URL         string          `json:"url"`
	Title       string          `json:"title"`
	Description string          `json:"description,omitempty"`
	Tags        []string        `json:"tags,omitempty"`
	Folder      string          `json:"folder,omitempty"`
	CreatedAt   time.Time       `json:"created_at"`
	UpdatedAt   time.Time       `json:"updated_at"`
	HasMarkdown bool            `json:"has_markdown"`
	HasArchive  bool            `json:"has_archive"`
	Attachments []apiAttachment `json:"attachments,omitempty"`
	OpenCount   int             `json:"open_count,omitempty"`
	LastOpened  *time.Time      `json:"last_opened_at,omitempty"`
}

func toAPIBookmark(b *Bookmark) apiBookmark {
	out := apiBookmark{
		ID: b.ID, URL: b.URL, Title: b.Title, Description: b.Description,
		Tags: append([]string{}, b.Tags...), Folder: b.Folder,
		CreatedAt: b.CreatedAt, UpdatedAt: b.UpdatedAt,
		HasMarkdown: b.MarkdownFile != "", HasArchive: b.ArchiveFile != "",
		OpenCount: b.OpenCount, LastOpened: b.LastOpenedAt,
	}
	for _, at := range b.Attachments {
		out.Attachments = append(out.Attachments, apiAttachment{Name: at.Name})
	}
	return out
}

type apiTag struct {
	Name  string `json:"name"`
	Count int    `json:"count"`
}

func handleAPITags(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	_, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	rows := sortedTaxRows(tagCounts(store))
	out := make([]apiTag, 0, len(rows))
	for _, row := range rows {
		out = append(out, apiTag{Name: row.Name, Count: row.Count})
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"tags": out})
}

func handleAPITagAction(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	switch strings.TrimPrefix(r.URL.Path, "/api/v1/tags/") {
	case "rename":
		var in struct {
			Old string `json:"old"`
			New string `json:"new"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		changed, err := renameTag(cfg, store, in.Old, in.New)
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		if len(changed) == 0 {
			writeAPIError(w, http.StatusNotFound, "no bookmarks have that tag")
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed)); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"renamed": len(changed)})
	case "delete":
		var in struct {
			Tag     string `json:"tag"`
			Confirm bool   `json:"confirm"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		if strings.TrimSpace(in.Tag) == "" {
			writeAPIError(w, http.StatusBadRequest, "tag is required")
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		if !in.Confirm {
			count := 0
			for _, b := range store.Bookmarks {
				if indexOfFold(b.Tags, strings.TrimSpace(in.Tag)) != -1 {
					count++
				}
			}
			if count == 0 {
				writeAPIError(w, http.StatusNotFound, "no bookmarks have that tag")
				return
			}
			writeAPIJSON(w, http.StatusOK, map[string]any{
				"confirm_required": true,
				"count":            count,
				"hint":             "repeat with confirm true to delete",
			})
			return
		}
		changed, err := deleteTag(cfg, store, in.Tag)
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		if len(changed) == 0 {
			writeAPIError(w, http.StatusNotFound, "no bookmarks have that tag")
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed)); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"deleted": len(changed)})
	default:
		writeAPIError(w, http.StatusNotFound, "no such endpoint")
	}
}

type apiFolder struct {
	Name    string `json:"name"`
	Display string `json:"display"`
	Count   int    `json:"count"`
}

func handleAPIFolders(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	_, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	rows := sortedTaxRows(folderCounts(store))
	out := make([]apiFolder, 0, len(rows))
	for _, row := range rows {
		name := row.Name
		if name == "/" {
			name = ""
		}
		out = append(out, apiFolder{Name: name, Display: displayFolder(name), Count: row.Count})
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"folders": out})
}

func handleAPIFolderAction(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	switch strings.TrimPrefix(r.URL.Path, "/api/v1/folders/") {
	case "rename":
		var in struct {
			Old string `json:"old"`
			New string `json:"new"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		changed, err := renameFolder(cfg, store, in.Old, in.New)
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		if len(changed) == 0 {
			writeAPIError(w, http.StatusNotFound, "no bookmarks in that folder")
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed)); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"moved": len(changed)})
	case "delete":
		var in struct {
			Folder  string `json:"folder"`
			Confirm bool   `json:"confirm"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		if sanitizeFolder(in.Folder) == "" {
			writeAPIError(w, http.StatusBadRequest, "folder is required")
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		if !in.Confirm {
			count := 0
			for _, b := range store.Bookmarks {
				if folderMatchesOrIsChild(b.Folder, sanitizeFolder(in.Folder)) {
					count++
				}
			}
			if count == 0 {
				writeAPIError(w, http.StatusNotFound, "no bookmarks in that folder")
				return
			}
			writeAPIJSON(w, http.StatusOK, map[string]any{
				"confirm_required": true,
				"count":            count,
				"hint":             "repeat with confirm true to move to root",
			})
			return
		}
		changed, err := renameFolder(cfg, store, in.Folder, "")
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		if len(changed) == 0 {
			writeAPIError(w, http.StatusNotFound, "no bookmarks in that folder")
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed)); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"moved": len(changed)})
	default:
		writeAPIError(w, http.StatusNotFound, "no such endpoint")
	}
}

type apiRule struct {
	ID           int      `json:"id"`
	Match        string   `json:"match"`
	Folder       string   `json:"folder,omitempty"`
	Tags         []string `json:"tags,omitempty"`
	AppliedCount int      `json:"applied_count"`
}

func toAPIRule(r *AutoRule, applied int) apiRule {
	return apiRule{
		ID: r.ID, Match: r.Match, Folder: r.Folder,
		Tags: append([]string{}, r.Tags...), AppliedCount: applied,
	}
}

func ruleAppliedCounts(store *Store) map[int]int {
	counts := map[int]int{}
	for _, b := range store.Bookmarks {
		for _, id := range ruleIDsOf(b.AppliedRules) {
			counts[id]++
		}
	}
	return counts
}

func handleAPIRules(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		_, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		counts := ruleAppliedCounts(store)
		out := make([]apiRule, 0, len(store.AutoRules))
		for _, rule := range store.AutoRules {
			out = append(out, toAPIRule(rule, counts[rule.ID]))
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"rules": out})
	case http.MethodPost:
		var in struct {
			Match  string   `json:"match"`
			Folder string   `json:"folder"`
			Tags   []string `json:"tags"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		rule, changed, err := createRule(cfg, store, in.Match, in.Folder, in.Tags)
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed).merge(journalRules([]*AutoRule{rule}))); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		out := toAPIRule(rule, len(changed))
		writeAPIJSON(w, http.StatusCreated, out)
	default:
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
	}
}

type apiSuggestion struct {
	Host   string `json:"host"`
	Folder string `json:"folder"`
	Count  int    `json:"count"`
}

func apiLearnMin(r *http.Request, bodyMin int) (int, bool) {
	min := bodyMin
	if min == 0 {
		min = 3
		if n, err := strconv.Atoi(strings.TrimSpace(r.URL.Query().Get("min"))); err == nil {
			min = n
		}
	}
	if min < 2 {
		return 0, false
	}
	return min, true
}

func handleAPIRulesSub(w http.ResponseWriter, r *http.Request) {
	rest := strings.TrimPrefix(r.URL.Path, "/api/v1/rules/")
	name, sub, _ := strings.Cut(rest, "/")
	switch {
	case name == "suggestions" && sub == "" && r.Method == http.MethodGet:
		_, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		min, ok := apiLearnMin(r, 0)
		if !ok {
			writeAPIError(w, http.StatusBadRequest, "min must be 2 or more")
			return
		}
		raw := suggestRules(store, min)
		out := make([]apiSuggestion, 0, len(raw))
		for _, s := range raw {
			out = append(out, apiSuggestion{Host: s.host, Folder: s.folder, Count: s.count})
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"min": min, "suggestions": out})
	case name == "learn" && sub == "" && r.Method == http.MethodPost:
		var in struct {
			Min int `json:"min"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		min, ok := apiLearnMin(r, in.Min)
		if !ok {
			writeAPIError(w, http.StatusBadRequest, "min must be 2 or more")
			return
		}
		learned := &JournalEntry{}
		created := []apiRule{}
		applied := 0
		for _, s := range suggestRules(store, min) {
			rule, changed, err := createRule(cfg, store, "host:"+s.host, s.folder, nil)
			if err != nil {
				continue
			}
			learned = learned.merge(journalUpserts(changed).merge(journalRules([]*AutoRule{rule})))
			created = append(created, toAPIRule(rule, len(changed)))
			applied += len(changed)
		}
		if len(created) > 0 {
			if err := saveWithJournal(cfg, store, learned); err != nil {
				writeAPIError(w, http.StatusInternalServerError, err.Error())
				return
			}
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"created": created, "applied": applied})
	case name == "apply" && sub == "" && r.Method == http.MethodPost:
		var in struct {
			ID int `json:"id"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		changed, err := applyRules(cfg, store, in.ID)
		if err != nil {
			if in.ID != 0 {
				writeAPIError(w, http.StatusNotFound, err.Error())
			} else {
				writeAPIError(w, http.StatusBadRequest, err.Error())
			}
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed)); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"applied": len(changed)})
	case sub == "" && r.Method == http.MethodDelete:
		id, err := strconv.Atoi(name)
		if err != nil || id < 1 {
			writeAPIError(w, http.StatusNotFound, "no such rule")
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		del := store.FindAutoRule(id)
		if del == nil {
			writeAPIError(w, http.StatusNotFound, "no such rule")
			return
		}
		if r.URL.Query().Get("confirm") != "true" {
			writeAPIJSON(w, http.StatusOK, map[string]any{
				"confirm_required": true,
				"rule":             toAPIRule(del, ruleAppliedCounts(store)[id]),
				"hint":             "repeat with ?confirm=true to delete",
			})
			return
		}
		tomb := journalRuleDeletes([]*AutoRule{del})
		store.DeleteAutoRule(id)
		if err := saveWithJournal(cfg, store, tomb); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"deleted": id})
	case sub == "" && r.Method == http.MethodPut:
		id, err := strconv.Atoi(name)
		if err != nil || id < 1 {
			writeAPIError(w, http.StatusNotFound, "no such rule")
			return
		}
		var in struct {
			Match   *string   `json:"match"`
			Folder  *string   `json:"folder"`
			Tags    *[]string `json:"tags"`
			Reapply bool      `json:"reapply"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		match, folder, tags := "", "", []string{}
		if in.Match != nil {
			match = *in.Match
		}
		if in.Folder != nil {
			folder = *in.Folder
		}
		if in.Tags != nil {
			tags = *in.Tags
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		if store.FindAutoRule(id) == nil {
			writeAPIError(w, http.StatusNotFound, "no such rule")
			return
		}
		rule, changed, err := editRule(cfg, store, id, match, folder, tags, in.Reapply)
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(changed).merge(journalRules([]*AutoRule{rule}))); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		out := toAPIRule(rule, ruleAppliedCounts(store)[rule.ID])
		writeAPIJSON(w, http.StatusOK, map[string]any{
			"id": out.ID, "match": out.Match, "folder": out.Folder,
			"tags": out.Tags, "applied_count": out.AppliedCount, "reapplied": len(changed),
		})
	default:
		writeAPIError(w, http.StatusNotFound, "no such endpoint")
	}
}

type apiCheckRow struct {
	ID     int    `json:"id"`
	Title  string `json:"title"`
	URL    string `json:"url"`
	Detail string `json:"detail,omitempty"`
	Target string `json:"target,omitempty"`
	Status string `json:"status"`
}

func toAPICheckRows(in []checkResult, status string) []apiCheckRow {
	out := make([]apiCheckRow, 0, len(in))
	for _, r := range in {
		out = append(out, apiCheckRow{
			ID: r.b.ID, Title: r.b.Title, URL: r.b.URL,
			Detail: r.detail, Target: r.target, Status: status,
		})
	}
	return out
}

func handleAPICheckRun(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	var in struct {
		Spec    string `json:"spec"`
		Workers int    `json:"workers"`
		Stale   string `json:"stale"`
	}
	if !decodeAPIBody(w, r, &in) {
		return
	}
	workers := ""
	if in.Workers != 0 {
		workers = strconv.Itoa(in.Workers)
	}
	spec, workersN, stale, _, err := parseCheckArgs(checkArgsFromForm(in.Spec, workers, in.Stale))
	if err != nil {
		writeAPIError(w, http.StatusBadRequest, err.Error())
		return
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	targets, missing, fresh, err := resolveCheckTargets(store, spec, stale)
	if err != nil {
		writeAPIError(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(targets) == 0 {
		writeAPIJSON(w, http.StatusOK, map[string]any{
			"ok": 0, "checked": 0, "fresh": fresh, "missing": missing,
			"moved": []apiCheckRow{}, "dead": []apiCheckRow{}, "uncertain": []apiCheckRow{},
		})
		return
	}
	moved, dead, uncertain := scanCheckTargets(checkClient(cfg), targets, workersN, nil)
	now := time.Now()
	writeMu.Lock()
	_, freshStore, err := loadCfgAndStore()
	if err != nil {
		writeMu.Unlock()
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	stampCheckTargets(freshStore, targetIDs(targets), moved, dead, uncertain, now)
	if err := freshStore.Save(); err != nil {
		writeMu.Unlock()
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeMu.Unlock()
	writeAPIJSON(w, http.StatusOK, map[string]any{
		"ok":      len(targets) - len(moved) - len(dead) - len(uncertain),
		"checked": len(targets), "fresh": fresh, "missing": missing,
		"moved":     toAPICheckRows(moved, "moved"),
		"dead":      toAPICheckRows(dead, "dead"),
		"uncertain": toAPICheckRows(uncertain, "uncertain"),
	})
}

func handleAPICheckApply(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	var in struct {
		ID      int    `json:"id"`
		Action  string `json:"action"`
		Target  string `json:"target"`
		Confirm bool   `json:"confirm"`
	}
	if !decodeAPIBody(w, r, &in) {
		return
	}
	action := strings.TrimSpace(in.Action)
	if action != "update" && action != "retitle" && action != "delete" && action != "quarantine" {
		writeAPIError(w, http.StatusBadRequest, "unknown action")
		return
	}
	if (action == "update" || action == "retitle") && strings.TrimSpace(in.Target) == "" {
		writeAPIError(w, http.StatusBadRequest, "target is required")
		return
	}
	writeMu.Lock()
	defer writeMu.Unlock()
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	b := store.Find(in.ID)
	if b == nil {
		writeAPIError(w, http.StatusNotFound, "bookmark already gone")
		return
	}
	if action == "delete" && !in.Confirm {
		writeAPIJSON(w, http.StatusOK, map[string]any{
			"confirm_required": true,
			"count":            1,
			"hint":             "repeat with confirm true to delete",
		})
		return
	}
	jent := &JournalEntry{}
	msg := ""
	switch action {
	case "update":
		b.URL = normalizeURL(in.Target)
		b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, b, false)
		jent = jent.merge(journalUpserts([]*Bookmark{b}))
		msg = fmt.Sprintf("Updated [%d].", b.ID)
	case "retitle":
		b.URL = normalizeURL(in.Target)
		b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, b, false)
		if title := fetchTitle(cfg, b.URL); title != "" && title != b.Title {
			b.Title = title
			b.UpdatedAt = time.Now()
			syncBookmarkFiles(cfg, b, false)
			msg = fmt.Sprintf("Updated [%d] and refreshed title.", b.ID)
		} else {
			msg = fmt.Sprintf("Updated [%d] (title unchanged).", b.ID)
		}
		jent = jent.merge(journalUpserts([]*Bookmark{b}))
	case "delete":
		jent = jent.merge(journalDeletes([]*Bookmark{b}))
		deleteBookmarkFiles(cfg, b)
		store.Delete(b.ID)
		msg = fmt.Sprintf("Deleted [%d].", b.ID)
	case "quarantine":
		quarantineBookmark(cfg, b)
		jent = jent.merge(journalUpserts([]*Bookmark{b}))
		msg = fmt.Sprintf("Quarantined [%d].", b.ID)
	}
	if err := saveWithJournal(cfg, store, jent); err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"result": msg})
}

func handleAPISettings(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		tags := map[string]bool{}
		folders := map[string]bool{}
		for _, b := range store.Bookmarks {
			for _, t := range b.Tags {
				tags[t] = true
			}
			folders[b.Folder] = true
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{
			"base_dir":           cfg.effectiveBaseDir(),
			"active_profile":     cfg.ActiveProfile,
			"archive_backend":    cfg.effectiveArchiveBackend(),
			"bookmarks":          len(store.Bookmarks),
			"tags":               len(tags),
			"folders":            len(folders),
			"rules":              len(store.AutoRules),
			"maintenance_status": maintenanceStatus(cfg, store),
		})
	case http.MethodPost:
		var in struct {
			ArchiveBackend string `json:"archive_backend"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, _, err := LoadConfig()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		switch b := strings.TrimSpace(in.ArchiveBackend); b {
		case "", "auto", "single-file", "monolith", "native":
			cfg.ArchiveBackend = b
		default:
			writeAPIError(w, http.StatusBadRequest, fmt.Sprintf("unknown archive_backend %q", b))
			return
		}
		if err := SaveConfig(cfg); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"archive_backend": cfg.effectiveArchiveBackend()})
	default:
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
	}
}

type apiProfile struct {
	Name    string `json:"name"`
	Path    string `json:"path"`
	Active  bool   `json:"active"`
	Default bool   `json:"default"`
}

func handleAPIProfiles(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	entries, err := profileListData()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	out := make([]apiProfile, 0, len(entries))
	active := "default"
	for _, e := range entries {
		out = append(out, apiProfile{Name: e.Name, Path: e.Path, Active: e.Active, Default: e.Default})
		if e.Active {
			active = e.Name
		}
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"active": active, "profiles": out})
}

func handleAPIProfilesSub(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	switch strings.TrimPrefix(r.URL.Path, "/api/v1/profiles/") {
	case "switch":
		var in struct {
			Name string `json:"name"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		msg, err := profileSwitchMsg(in.Name)
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		active := strings.TrimSpace(in.Name)
		if active == "" {
			active = "default"
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"result": msg, "active": active})
	case "delete":
		var in struct {
			Name string `json:"name"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		msg, err := profileDeleteMsg(in.Name)
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"result": msg})
	default:
		writeAPIError(w, http.StatusNotFound, "no such endpoint")
	}
}

type apiHistoryRow struct {
	ID         int        `json:"id"`
	Title      string     `json:"title"`
	URL        string     `json:"url"`
	OpenCount  int        `json:"open_count"`
	LastOpened *time.Time `json:"last_opened_at,omitempty"`
}

func handleAPIHistory(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	_, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	rows := historyRows(store)
	out := make([]apiHistoryRow, 0, len(rows))
	for _, b := range rows {
		out = append(out, apiHistoryRow{
			ID: b.ID, Title: b.Title, URL: b.URL,
			OpenCount: b.OpenCount, LastOpened: b.LastOpenedAt,
		})
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"history": out})
}

func handleAPIBulk(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	var in struct {
		IDs     []int    `json:"ids"`
		Action  string   `json:"action"`
		Tags    []string `json:"tags"`
		Folder  *string  `json:"folder"`
		Confirm bool     `json:"confirm"`
	}
	if !decodeAPIBody(w, r, &in) {
		return
	}
	action := strings.TrimSpace(in.Action)
	if action != "delete" && action != "tags" && action != "folder" {
		writeAPIError(w, http.StatusBadRequest, "unknown action")
		return
	}
	writeMu.Lock()
	defer writeMu.Unlock()
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	seen := map[int]bool{}
	var targets []*Bookmark
	for _, id := range in.IDs {
		if id < 1 || seen[id] {
			continue
		}
		seen[id] = true
		if b := store.Find(id); b != nil {
			targets = append(targets, b)
		}
	}
	if len(targets) == 0 {
		writeAPIError(w, http.StatusNotFound, "no matching bookmarks")
		return
	}
	if action == "delete" && !in.Confirm {
		titles := make([]string, 0, len(targets))
		for _, b := range targets {
			titles = append(titles, b.Title)
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{
			"confirm_required": true,
			"count":            len(targets),
			"titles":           titles,
			"hint":             "repeat with confirm true to delete",
		})
		return
	}
	msg := ""
	switch action {
	case "delete":
		tomb := journalDeletes(targets)
		for _, b := range targets {
			deleteBookmarkFiles(cfg, b)
			store.Delete(b.ID)
		}
		if err := saveWithJournal(cfg, store, tomb); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		msg = fmt.Sprintf("Deleted %d bookmark(s).", len(targets))
	case "tags":
		tags := dedupe(in.Tags)
		for _, b := range targets {
			b.Tags = tags
			b.UpdatedAt = time.Now()
			syncBookmarkFiles(cfg, b, false)
		}
		if err := saveWithJournal(cfg, store, journalUpserts(targets)); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		msg = fmt.Sprintf("Updated tags on %d bookmark(s).", len(targets))
	case "folder":
		newFolder := ""
		if in.Folder != nil {
			newFolder = sanitizeFolder(*in.Folder)
		}
		for _, b := range targets {
			folderChanged := newFolder != b.Folder
			b.Folder = newFolder
			b.UpdatedAt = time.Now()
			syncBookmarkFiles(cfg, b, folderChanged)
		}
		if err := saveWithJournal(cfg, store, journalUpserts(targets)); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		msg = fmt.Sprintf("Moved %d bookmark(s).", len(targets))
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"result": msg, "count": len(targets)})
}

func handleAPILibrary(w http.ResponseWriter, r *http.Request) {
	rest := strings.TrimPrefix(r.URL.Path, "/api/v1/library/")
	switch {
	case rest == "export" && r.Method == http.MethodGet:
		_, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		var buf bytes.Buffer
		if err := writeNetscapeExport(&buf, store); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		w.Header().Set("Content-Disposition", `attachment; filename="liber-bookmarks.html"`)
		w.Write(buf.Bytes())
	case rest == "import" && r.Method == http.MethodPost:
		var in struct {
			Content  string `json:"content"`
			Markdown bool   `json:"markdown"`
			Archive  bool   `json:"archive"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		if strings.TrimSpace(in.Content) == "" {
			writeAPIError(w, http.StatusBadRequest, "content is required")
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		added, skippedDup, skippedBad, warnings := importData(cfg, store, []byte(in.Content), importOptions{Markdown: in.Markdown, Archive: in.Archive})
		if len(added) == 0 && skippedDup == 0 && skippedBad == 0 {
			writeAPIError(w, http.StatusBadRequest, "no bookmarks found -- is it a browser bookmark export?")
			return
		}
		if err := saveWithJournal(cfg, store, journalUpserts(added)); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{
			"imported": len(added), "skipped_dup": skippedDup,
			"skipped_bad": skippedBad, "warnings": warnings,
		})
	case rest == "site" && r.Method == http.MethodPost:
		var in struct {
			Dir string `json:"dir"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		outDir := strings.TrimSpace(in.Dir)
		if outDir == "" {
			outDir = filepath.Join(cfg.effectiveBaseDir(), "site")
		}
		out, err := doExportSite(cfg, store, expandTilde(outDir))
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"path": out, "count": len(store.Bookmarks)})
	default:
		writeAPIError(w, http.StatusNotFound, "no such endpoint")
	}
}

func handleAPISync(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	var in struct {
		Push bool `json:"push"`
	}
	if !decodeAPIBody(w, r, &in) {
		return
	}
	writeMu.Lock()
	defer writeMu.Unlock()
	var buf strings.Builder
	if err := runSyncTo(&buf, in.Push); err != nil {
		writeAPIJSON(w, http.StatusOK, map[string]any{"output": buf.String(), "error": err.Error()})
		return
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"output": buf.String()})
}

func handleAPIReindex(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	var in struct {
		Merge        bool `json:"merge"`
		All          bool `json:"all"`
		Prune        bool `json:"prune"`
		Compact      bool `json:"compact"`
		PruneJournal bool `json:"prune_journal"`
	}
	if !decodeAPIBody(w, r, &in) {
		return
	}
	if in.All && !in.Merge {
		writeAPIError(w, http.StatusBadRequest, "--all requires --merge")
		return
	}
	writeMu.Lock()
	defer writeMu.Unlock()
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	var buf strings.Builder
	if err := runReindexWith(&buf, cfg, store, reindexFlags{
		merge: in.Merge, all: in.All, prune: in.Prune,
		compact: in.Compact, pruneJournal: in.PruneJournal,
	}); err != nil {
		writeAPIJSON(w, http.StatusOK, map[string]any{"output": buf.String(), "error": err.Error()})
		return
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"output": buf.String()})
}

func writeAPIError(w http.ResponseWriter, code int, msg string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	fmt.Fprintf(w, `{"error":%q}`+"\n", msg)
}

func writeAPIJSON(w http.ResponseWriter, code int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(code)
	_ = json.NewEncoder(w).Encode(v)
}

func decodeAPIBody(w http.ResponseWriter, r *http.Request, v any) bool {
	r.Body = http.MaxBytesReader(w, r.Body, apiMaxBody)
	if err := json.NewDecoder(r.Body).Decode(v); err != nil && err != io.EOF {
		writeAPIError(w, http.StatusBadRequest, "invalid JSON body")
		return false
	}
	return true
}

func apiOpenID(r *http.Request) (int, bool) {
	rest, ok := strings.CutPrefix(r.URL.Path, "/api/v1/bookmarks/")
	if !ok {
		return 0, false
	}
	idStr, sub, _ := strings.Cut(rest, "/")
	if sub != "open" {
		return 0, false
	}
	id, err := strconv.Atoi(idStr)
	if err != nil || id < 1 {
		return 0, false
	}
	return id, true
}

func handleAPIOpen(w http.ResponseWriter, r *http.Request, id int) {
	if r.Method != http.MethodPost {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	writeMu.Lock()
	defer writeMu.Unlock()
	_, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	b := store.Find(id)
	if b == nil {
		writeAPIError(w, http.StatusNotFound, "no such bookmark")
		return
	}
	recordOpen(b)
	if err := store.Save(); err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeAPIJSON(w, http.StatusOK, map[string]any{"url": b.URL})
}

func apiContentID(r *http.Request) (id int, kind string, n int, ok bool) {
	rest, found := strings.CutPrefix(r.URL.Path, "/api/v1/bookmarks/")
	if !found {
		return 0, "", 0, false
	}
	parts := strings.Split(rest, "/")
	if len(parts) == 2 && (parts[1] == "card" || parts[1] == "archive" || parts[1] == "markdown") {
		id, err := strconv.Atoi(parts[0])
		if err != nil || id < 1 {
			return 0, "", 0, false
		}
		return id, parts[1], 0, true
	}
	if len(parts) == 3 && parts[1] == "attachments" {
		id, err := strconv.Atoi(parts[0])
		n, nerr := strconv.Atoi(parts[2])
		if err != nil || id < 1 || nerr != nil || n < 1 {
			return 0, "", 0, false
		}
		return id, "attachment", n, true
	}
	return 0, "", 0, false
}

func handleAPIContent(w http.ResponseWriter, r *http.Request, id int, kind string, n int) {
	if r.Method != http.MethodGet {
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
		return
	}
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		writeAPIError(w, http.StatusInternalServerError, err.Error())
		return
	}
	b := store.Find(id)
	if b == nil {
		writeAPIError(w, http.StatusNotFound, "no such bookmark")
		return
	}
	switch kind {
	case "card", "archive":
		rel := b.HTMLFile
		dir := cfg.htmlDir()
		if kind == "archive" {
			rel = b.ArchiveFile
			dir = cfg.archiveDir()
		}
		if rel == "" {
			writeAPIError(w, http.StatusNotFound, "no saved "+kind)
			return
		}
		http.ServeFile(w, r, filepath.Join(dir, rel))
	case "markdown":
		if b.MarkdownFile == "" {
			writeAPIError(w, http.StatusNotFound, "no saved markdown")
			return
		}
		doc, err := markdownPageHTML(cfg, b)
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		fmt.Fprint(w, doc)
	case "attachment":
		if n > len(b.Attachments) {
			writeAPIError(w, http.StatusNotFound, "no such attachment")
			return
		}
		at := b.Attachments[n-1]
		w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%q", at.Name))
		http.ServeFile(w, r, filepath.Join(cfg.attachmentsDir(), at.File))
	}
}

func apiBookmarkID(r *http.Request) (int, bool) {
	id, err := strconv.Atoi(strings.TrimPrefix(r.URL.Path, "/api/v1/bookmarks/"))
	if err != nil || id < 1 {
		return 0, false
	}
	return id, true
}

func handleAPIBookmarks(w http.ResponseWriter, r *http.Request) {
	switch r.Method {
	case http.MethodGet:
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		q := r.URL.Query().Get("q")
		deep := r.URL.Query().Get("deep") == "1"
		fields := scopeFromParams(r.URL.Query()["scope"])
		page, _ := strconv.Atoi(r.URL.Query().Get("page"))
		sortMode, err := ParseSortMode(r.URL.Query().Get("sort"))
		if err != nil {
			writeAPIError(w, http.StatusBadRequest, err.Error())
			return
		}
		var results []*Bookmark
		switch {
		case deep && strings.TrimSpace(q) != "":
			results = filterDeep(cfg, store.All(), q, fields, sortMode)
		case deep:
			results = orderResults(store.All(), "", fields, sortMode)
		default:
			results = store.Search(cfg, q, fields, false, sortMode)
		}
		pageItems, totalPages, curPage := paginate(results, page)
		out := make([]apiBookmark, 0, len(pageItems))
		for _, b := range pageItems {
			out = append(out, toAPIBookmark(b))
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{
			"total": len(results), "page": curPage, "total_pages": totalPages,
			"bookmarks": out,
		})
	case http.MethodPost:
		var in struct {
			URL         string   `json:"url"`
			Title       string   `json:"title"`
			Description string   `json:"description"`
			Tags        []string `json:"tags"`
			Folder      string   `json:"folder"`
			Markdown    bool     `json:"markdown"`
			Archive     bool     `json:"archive"`
			ConfirmDup  bool     `json:"confirm_dup"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		if strings.TrimSpace(in.URL) == "" {
			writeAPIError(w, http.StatusBadRequest, "url is required")
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		normalizedURL := normalizeURL(in.URL)
		if !in.ConfirmDup {
			if dup := findDuplicate(store, normalizedURL); dup != nil {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusConflict)
				_ = json.NewEncoder(w).Encode(map[string]any{
					"error":     "possible duplicate",
					"duplicate": toAPIBookmark(dup),
					"hint":      "resubmit with confirm_dup true to add anyway",
				})
				return
			}
		}
		title := strings.TrimSpace(in.Title)
		if title == "" {
			title = fetchTitle(cfg, normalizedURL)
			if strings.TrimSpace(title) == "" {
				title = normalizedURL
			}
		}
		folder := sanitizeFolder(in.Folder)
		tags := dedupe(in.Tags)
		folder, tags, applied := resolveAutoRulesForNew(store, normalizedURL, title, folder, tags)
		b, err := addBookmarkToStore(cfg, store, normalizedURL, title, strings.TrimSpace(in.Description), tags, folder, in.Markdown, in.Archive)
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		b.AppliedRules = applied
		if err := saveWithJournal(cfg, store, journalUpserts([]*Bookmark{b})); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusCreated, toAPIBookmark(b))
	default:
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
	}
}

func handleAPIBookmark(w http.ResponseWriter, r *http.Request) {
	if id, ok := apiOpenID(r); ok {
		handleAPIOpen(w, r, id)
		return
	}
	if id, kind, n, ok := apiContentID(r); ok {
		handleAPIContent(w, r, id, kind, n)
		return
	}
	id, ok := apiBookmarkID(r)
	if !ok {
		writeAPIError(w, http.StatusNotFound, "no such bookmark")
		return
	}
	switch r.Method {
	case http.MethodGet:
		_, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		b := store.Find(id)
		if b == nil {
			writeAPIError(w, http.StatusNotFound, "no such bookmark")
			return
		}
		writeAPIJSON(w, http.StatusOK, toAPIBookmark(b))
	case http.MethodPut:
		var in struct {
			Title       *string   `json:"title"`
			URL         *string   `json:"url"`
			Description *string   `json:"description"`
			Tags        *[]string `json:"tags"`
			Folder      *string   `json:"folder"`
			Markdown    bool      `json:"markdown"`
			Archive     bool      `json:"archive"`
		}
		if !decodeAPIBody(w, r, &in) {
			return
		}
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		b := store.Find(id)
		if b == nil {
			writeAPIError(w, http.StatusNotFound, "no such bookmark")
			return
		}
		if in.Title != nil {
			if strings.TrimSpace(*in.Title) == "" {
				writeAPIError(w, http.StatusBadRequest, "title can't be empty")
				return
			}
			b.Title = strings.TrimSpace(*in.Title)
		}
		if in.URL != nil {
			if strings.TrimSpace(*in.URL) == "" {
				writeAPIError(w, http.StatusBadRequest, "url can't be empty")
				return
			}
			b.URL = normalizeURL(*in.URL)
		}
		if in.Description != nil {
			b.Description = strings.TrimSpace(*in.Description)
		}
		if in.Tags != nil {
			b.Tags = dedupe(*in.Tags)
		}
		folderChanged := false
		if in.Folder != nil {
			newFolder := sanitizeFolder(*in.Folder)
			folderChanged = newFolder != b.Folder
			b.Folder = newFolder
		}
		b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, b, folderChanged)
		if in.Markdown {
			addMarkdownCopy(cfg, b)
		}
		if in.Archive {
			addArchiveCopy(cfg, b)
		}
		if err := saveWithJournal(cfg, store, journalUpserts([]*Bookmark{b})); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, toAPIBookmark(b))
	case http.MethodDelete:
		writeMu.Lock()
		defer writeMu.Unlock()
		cfg, store, err := loadCfgAndStore()
		if err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		b := store.Find(id)
		if b == nil {
			writeAPIError(w, http.StatusNotFound, "no such bookmark")
			return
		}
		if r.URL.Query().Get("confirm") != "true" {
			writeAPIJSON(w, http.StatusOK, map[string]any{
				"confirm_required": true,
				"bookmark":         toAPIBookmark(b),
				"hint":             "repeat with ?confirm=true to delete",
			})
			return
		}
		tomb := journalDeletes([]*Bookmark{b})
		deleteBookmarkFiles(cfg, b)
		store.Delete(id)
		if err := saveWithJournal(cfg, store, tomb); err != nil {
			writeAPIError(w, http.StatusInternalServerError, err.Error())
			return
		}
		writeAPIJSON(w, http.StatusOK, map[string]any{"deleted": id})
	default:
		writeAPIError(w, http.StatusMethodNotAllowed, "method not allowed")
	}
}
