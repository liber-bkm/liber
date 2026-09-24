package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
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
	default:
		writeAPIError(w, http.StatusNotFound, "no such endpoint")
	}
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
