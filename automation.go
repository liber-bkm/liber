package main

import (
	"fmt"
	"sort"
	"strconv"
	"strings"
	"time"
)

func ruleMatches(r *AutoRule, url, title string) bool {
	match := r.Match
	target := strings.ToLower(url)
	if rest, ok := strings.CutPrefix(strings.ToLower(match), "title:"); ok {
		match, target = rest, strings.ToLower(title)
	} else if rest, ok := strings.CutPrefix(strings.ToLower(match), "host:"); ok {
		match, target = rest, hostOf(url)
	}
	return match != "" && strings.Contains(target, match)
}

func hostOf(url string) string {
	rest := url
	if i := strings.Index(rest, "://"); i != -1 {
		rest = rest[i+3:]
	}
	if i := strings.IndexAny(rest, "/?#"); i != -1 {
		rest = rest[:i]
	}
	if i := strings.LastIndex(rest, ":"); i != -1 {
		rest = rest[:i]
	}
	return strings.ToLower(rest)
}

func findAppliedRule(applied []AppliedAutoRule, ruleID int) (AppliedAutoRule, bool) {
	for _, a := range applied {
		if a.RuleID == ruleID {
			return a, true
		}
	}
	return AppliedAutoRule{}, false
}

func removeAppliedRule(applied []AppliedAutoRule, ruleID int) []AppliedAutoRule {
	for i, a := range applied {
		if a.RuleID == ruleID {
			return append(append([]AppliedAutoRule{}, applied[:i]...), applied[i+1:]...)
		}
	}
	return applied
}

func ruleIDsOf(applied []AppliedAutoRule) []int {
	ids := make([]int, len(applied))
	for i, a := range applied {
		ids[i] = a.RuleID
	}
	return ids
}

func resolveAutoRulesForNew(store *Store, url, title, folder string, tags []string) (string, []string, []AppliedAutoRule) {
	var applied []AppliedAutoRule
	for _, r := range store.AutoRules {
		if !ruleMatches(r, url, title) {
			continue
		}
		used := false
		setFolder := ""
		if r.Folder != "" {
			if folder == "" {
				folder = r.Folder
				setFolder = r.Folder
			}
			used = true
		}
		if len(r.Tags) > 0 {
			tags = dedupe(append(tags, r.Tags...))
			used = true
		}
		if used {
			applied = append(applied, AppliedAutoRule{RuleID: r.ID, Folder: setFolder})
		}
	}
	return folder, tags, applied
}

func applyRulesToExisting(cfg Config, b *Bookmark, rules []*AutoRule) bool {
	seen := map[int]bool{}
	for _, a := range b.AppliedRules {
		seen[a.RuleID] = true
	}
	changed := false
	folderChanged := false
	for _, r := range rules {
		if seen[r.ID] || !ruleMatches(r, b.URL, b.Title) {
			continue
		}
		used := false
		setFolder := ""
		if r.Folder != "" {
			if b.Folder == "" {
				b.Folder = r.Folder
				setFolder = r.Folder
				folderChanged = true
			}
			used = true
		}
		if len(r.Tags) > 0 {
			b.Tags = dedupe(append(b.Tags, r.Tags...))
			used = true
		}
		if used {
			b.AppliedRules = append(b.AppliedRules, AppliedAutoRule{RuleID: r.ID, Folder: setFolder})
			changed = true
		}
	}
	if changed {
		b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, b, folderChanged)
	}
	return changed
}

func reapplyRule(cfg Config, b *Bookmark, r *AutoRule) bool {
	prev, hadPrev := findAppliedRule(b.AppliedRules, r.ID)

	if !ruleMatches(r, b.URL, b.Title) {
		if hadPrev {
			b.AppliedRules = removeAppliedRule(b.AppliedRules, r.ID)
			return true
		}
		return false
	}

	changed := false
	folderChanged := false

	if r.Folder != "" {
		safeToSet := b.Folder == "" || (hadPrev && prev.Folder != "" && prev.Folder == b.Folder)
		if safeToSet && b.Folder != r.Folder {
			b.Folder = r.Folder
			folderChanged = true
			changed = true
		}
	}
	if len(r.Tags) > 0 {
		before := len(b.Tags)
		b.Tags = dedupe(append(b.Tags, r.Tags...))
		if len(b.Tags) != before {
			changed = true
		}
	}

	newFolder := ""
	if r.Folder != "" && b.Folder == r.Folder {
		newFolder = r.Folder
	}
	b.AppliedRules = append(removeAppliedRule(b.AppliedRules, r.ID), AppliedAutoRule{RuleID: r.ID, Folder: newFolder})

	if changed {
		b.UpdatedAt = time.Now()
		syncBookmarkFiles(cfg, b, folderChanged)
	}
	return changed
}

func describeRule(r *AutoRule) string {
	var sb strings.Builder
	kind := "url"
	if _, ok := strings.CutPrefix(strings.ToLower(r.Match), "title:"); ok {
		kind = "title"
	} else if _, ok := strings.CutPrefix(strings.ToLower(r.Match), "host:"); ok {
		kind = "host"
	}
	fmt.Fprintf(&sb, "[%d] %s contains %q", r.ID, kind, r.Match)
	if r.Folder != "" {
		fmt.Fprintf(&sb, " -> folder %q", r.Folder)
	}
	if len(r.Tags) > 0 {
		fmt.Fprintf(&sb, " -> tag(s) %s", strings.Join(r.Tags, ", "))
	}
	return sb.String()
}

func parseAutoFlags(args []string) (match, folder string, tags []string, reapply bool, err error) {
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--match":
			if i+1 >= len(args) {
				return "", "", nil, false, fmt.Errorf("--match requires a value")
			}
			match = args[i+1]
			i++
		case "--folder":
			if i+1 >= len(args) {
				return "", "", nil, false, fmt.Errorf("--folder requires a value")
			}
			folder = args[i+1]
			i++
		case "--tag":
			i++
			for i < len(args) && !strings.HasPrefix(args[i], "-") {
				tags = append(tags, args[i])
				i++
			}
			i--
		case "--reapply":
			reapply = true
		default:
			return "", "", nil, false, fmt.Errorf("unknown flag: %s", args[i])
		}
	}
	return match, folder, tags, reapply, nil
}

func runAutoAdd(args []string) error {
	match, folder, tags, _, err := parseAutoFlags(args)
	if err != nil {
		return fmt.Errorf("usage: liber --auto add --match <substring> [--folder <folder>] [--tag <t1 t2 ...>]: %w", err)
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	rule, changed, err := createRule(cfg, store, match, folder, tags)
	if err != nil {
		return err
	}
	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}

	fmt.Println("Added automation", describeRule(rule))
	if changed > 0 {
		fmt.Printf("Applied to %d existing bookmark(s).\n", changed)
	}
	return nil
}

// createRule adds the rule and backfills it onto matching existing bookmarks.
func createRule(cfg Config, store *Store, match, folder string, tags []string) (*AutoRule, int, error) {
	if strings.TrimSpace(match) == "" {
		return nil, 0, fmt.Errorf("--match is required")
	}
	folder = sanitizeFolder(folder)
	tags = dedupe(tags)
	if folder == "" && len(tags) == 0 {
		return nil, 0, fmt.Errorf("a rule needs --folder and/or --tag")
	}

	rule := &AutoRule{Match: match, Folder: folder, Tags: tags, CreatedAt: time.Now()}
	store.AddAutoRule(rule)

	changed := 0
	for _, b := range store.Bookmarks {
		if applyRulesToExisting(cfg, b, []*AutoRule{rule}) {
			changed++
		}
	}
	return rule, changed, nil
}

// editRule updates the rule's match/folder/tags (empty values keep the old ones);
// with reapply it re-syncs the bookmarks it already classified.
func editRule(cfg Config, store *Store, id int, match, folder string, tags []string, reapply bool) (*AutoRule, int, error) {
	rule := store.FindAutoRule(id)
	if rule == nil {
		return nil, 0, fmt.Errorf("no automation with id %d", id)
	}

	if match != "" {
		rule.Match = match
	}
	if folder != "" {
		rule.Folder = sanitizeFolder(folder)
	}
	if len(tags) > 0 {
		rule.Tags = dedupe(tags)
	}

	changed := 0
	if reapply {
		for _, b := range store.Bookmarks {
			if reapplyRule(cfg, b, rule) {
				changed++
			}
		}
	}
	return rule, changed, nil
}

func runAutoList() error {
	_, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	if len(store.AutoRules) == 0 {
		fmt.Println("No automations yet. Add one with: liber --auto add --match <substring> --folder <folder>")
		return nil
	}
	counts := map[int]int{}
	for _, b := range store.Bookmarks {
		for _, id := range ruleIDsOf(b.AppliedRules) {
			counts[id]++
		}
	}
	for _, r := range store.AutoRules {
		fmt.Printf("%s  (applied to %d bookmark(s))\n", describeRule(r), counts[r.ID])
	}
	return nil
}

func runAutoEdit(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("usage: liber --auto edit <id> [--match x] [--folder y] [--tag t1 t2] [--reapply]")
	}
	id, err := strconv.Atoi(args[0])
	if err != nil {
		return fmt.Errorf("invalid automation id %q", args[0])
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	match, folder, tags, reapply, err := parseAutoFlags(args[1:])
	if err != nil {
		return err
	}
	rule, changed, err := editRule(cfg, store, id, match, folder, tags, reapply)
	if err != nil {
		return err
	}
	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}

	fmt.Println("Updated automation", describeRule(rule))
	if reapply {
		fmt.Printf("Reapplied to %d bookmark(s).\n", changed)
	}
	return nil
}

func runAutoDelete(args []string) error {
	if len(args) < 1 {
		return fmt.Errorf("usage: liber --auto delete <id>")
	}
	id, err := strconv.Atoi(args[0])
	if err != nil {
		return fmt.Errorf("invalid automation id %q", args[0])
	}

	_, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	if !store.DeleteAutoRule(id) {
		return fmt.Errorf("no automation with id %d (see `liber --auto list`)", id)
	}
	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}
	fmt.Printf("Deleted automation [%d]. Bookmarks it already classified are left as-is.\n", id)
	return nil
}

// applyRules runs all rules (or one, when id != 0) against existing bookmarks.
func applyRules(cfg Config, store *Store, id int) (int, error) {
	rules := store.AutoRules
	if id != 0 {
		rule := store.FindAutoRule(id)
		if rule == nil {
			return 0, fmt.Errorf("no automation with id %d", id)
		}
		rules = []*AutoRule{rule}
	}
	if len(rules) == 0 {
		return 0, fmt.Errorf("no automations to apply")
	}
	changed := 0
	for _, b := range store.Bookmarks {
		if applyRulesToExisting(cfg, b, rules) {
			changed++
		}
	}
	return changed, nil
}

func runAutoApply(args []string) error {
	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	id := 0
	if len(args) > 0 {
		id, err = strconv.Atoi(args[0])
		if err != nil {
			return fmt.Errorf("invalid automation id %q", args[0])
		}
	}
	changed, err := applyRules(cfg, store, id)
	if err != nil {
		return err
	}

	if err := store.Save(); err != nil {
		return fmt.Errorf("saving index: %w", err)
	}
	fmt.Printf("Applied automations to %d bookmark(s).\n", changed)
	return nil
}

type learnSuggestion struct {
	host   string
	folder string
	count  int
}

func runAutoLearn(args []string) error {
	min := 3
	create := false
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--min":
			if i+1 >= len(args) {
				return fmt.Errorf("usage: liber --auto learn [--min N] [--create]")
			}
			n, err := strconv.Atoi(args[i+1])
			if err != nil || n < 2 {
				return fmt.Errorf("--min requires a number of 2 or more")
			}
			min = n
			i++
		case "--create":
			create = true
		default:
			return fmt.Errorf("unknown flag %q (usage: liber --auto learn [--min N] [--create])", args[i])
		}
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}

	covered := map[string]bool{}
	for _, r := range store.AutoRules {
		if rest, ok := strings.CutPrefix(strings.ToLower(r.Match), "host:"); ok {
			covered[rest] = true
		}
	}
	counts := map[string]map[string]int{}
	for _, b := range store.Bookmarks {
		h := hostOf(b.URL)
		if h == "" || b.Folder == "" || covered[h] {
			continue
		}
		if counts[h] == nil {
			counts[h] = map[string]int{}
		}
		counts[h][b.Folder]++
	}
	var suggestions []learnSuggestion
	for h, folders := range counts {
		names := make([]string, 0, len(folders))
		for f := range folders {
			names = append(names, f)
		}
		sort.Strings(names)
		best, bestN := "", 0
		for _, f := range names {
			if folders[f] > bestN {
				best, bestN = f, folders[f]
			}
		}
		if bestN >= min {
			suggestions = append(suggestions, learnSuggestion{host: h, folder: best, count: bestN})
		}
	}
	sort.Slice(suggestions, func(i, j int) bool { return suggestions[i].count > suggestions[j].count })
	if len(suggestions) == 0 {
		fmt.Println("No rule suggestions: no host appears in one folder often enough.")
		return nil
	}

	made := 0
	for _, s := range suggestions {
		fmt.Printf("%d bookmark(s) with host %s are in folder %q: liber --auto add --match host:%s --folder %s\n",
			s.count, s.host, s.folder, s.host, s.folder)
		if !create && !confirm(fmt.Sprintf("Create this rule?"), false) {
			continue
		}
		rule, changed, err := createRule(cfg, store, "host:"+s.host, s.folder, nil)
		if err != nil {
			fmt.Printf("warning: could not create rule for host %s: %v\n", s.host, err)
			continue
		}
		made++
		fmt.Printf("Added automation %s (applied to %d existing bookmark(s)).\n", describeRule(rule), changed)
	}
	if made > 0 {
		if err := store.Save(); err != nil {
			return fmt.Errorf("saving index: %w", err)
		}
	}
	return nil
}
