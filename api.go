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
