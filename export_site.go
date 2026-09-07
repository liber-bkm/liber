package main

import (
	"fmt"
	"html/template"
	"os"
	"path/filepath"
	"strings"
)

func runExportSite(args []string) error {
	outDir := ""
	for _, a := range args {
		if strings.HasPrefix(a, "-") {
			return fmt.Errorf("unknown flag for --export-site: %s", a)
		}
		if outDir != "" {
			return fmt.Errorf("--export-site takes at most one directory")
		}
		outDir = a
	}

	cfg, store, err := loadCfgAndStore()
	if err != nil {
		return err
	}
	if outDir == "" {
		outDir = filepath.Join(cfg.effectiveBaseDir(), "site")
	}
	outDir = expandTilde(outDir)

	bookmarkPath := func(kindDir, rel string) string {
		if rel == "" {
			return ""
		}
		p, err := filepath.Rel(outDir, filepath.Join(kindDir, rel))
		if err != nil {
			return rel
		}
		return p
	}

	type exportedAtt struct {
		Name, Href string
	}
	type exportedBookmark struct {
		ID       int
		Title    string
		URL      string
		Desc     string
		Tags     []string
		HTML     string
		Markdown string
		Archive  string
		Atts     []exportedAtt
	}
	type exportedFolder struct {
		Name      string
		Bookmarks []exportedBookmark
	}

	folders := map[string][]exportedBookmark{}
	var order []string
	for _, b := range store.All() {
		eb := exportedBookmark{
			ID:       b.ID,
			Title:    b.Title,
			URL:      b.URL,
			Desc:     b.Description,
			Tags:     b.Tags,
			HTML:     bookmarkPath(cfg.htmlDir(), b.HTMLFile),
			Markdown: bookmarkPath(cfg.markdownDir(), b.MarkdownFile),
			Archive:  bookmarkPath(cfg.archiveDir(), b.ArchiveFile),
		}
		for _, at := range b.Attachments {
			eb.Atts = append(eb.Atts, exportedAtt{Name: at.Name, Href: bookmarkPath(cfg.attachmentsDir(), at.File)})
		}
		if _, seen := folders[b.Folder]; !seen {
			order = append(order, b.Folder)
		}
		folders[b.Folder] = append(folders[b.Folder], eb)
	}

	var groups []exportedFolder
	for _, name := range order {
		groups = append(groups, exportedFolder{Name: displayFolder(name), Bookmarks: folders[name]})
	}

	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return err
	}
	out := filepath.Join(outDir, "index.html")
	f, err := os.Create(out)
	if err != nil {
		return err
	}
	defer f.Close()
	if err := exportSiteTmpl.Execute(f, struct {
		Folders []exportedFolder
		Total   int
	}{groups, len(store.Bookmarks)}); err != nil {
		return err
	}

	fmt.Printf("Exported %d bookmark(s) to %s\n", len(store.Bookmarks), out)
	return nil
}

var exportSiteTmpl = template.Must(template.New("exportSite").Parse(`<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>liber export</title>
<style>
  body { font-family: system-ui, -apple-system, sans-serif; max-width: 760px;
         margin: 3rem auto; padding: 0 1rem; color: #1a1a1a; background: #fafafa; }
  h1 { font-size: 1.4rem; }
  .folder { margin-top: 1.8rem; }
  .folder h2 { font-size: 1.05rem; color: #555; border-bottom: 1px solid #e2e2e2; padding-bottom: .3rem; }
  ul { list-style: none; padding: 0; }
  li { padding: .45rem 0; }
  a { color: #0b5fff; text-decoration: none; }
  a:hover { text-decoration: underline; }
  .meta { font-size: .8rem; color: #777; word-break: break-word; }
  .badge { font-size: .7rem; background: #eee; border-radius: 999px; padding: .05rem .4rem;
           color: #555; text-decoration: none; margin-left: .3rem; }
  .tag { color: #a06a00; font-size: .8rem; }
</style>
</head>
<body>
<h1>liber export &middot; {{.Total}} bookmark(s)</h1>
{{range .Folders}}
<section class="folder">
  <h2>{{.Name}}</h2>
  <ul>
  {{range .Bookmarks}}
    <li>
      <a href="{{.HTML}}">{{.Title}}</a>
      {{if .Markdown}}<a class="badge" href="{{.Markdown}}">md</a>{{end}}
      {{if .Archive}}<a class="badge" href="{{.Archive}}">arc</a>{{end}}
      {{range .Atts}}<a class="badge" href="{{.Href}}">{{.Name}}</a>{{end}}
      <div class="meta"><a href="{{.URL}}">{{.URL}}</a>{{range .Tags}} <span class="tag">#{{.}}</span>{{end}} &middot; id {{.ID}}</div>
      {{if .Desc}}<div class="meta">{{.Desc}}</div>{{end}}
    </li>
  {{end}}
  </ul>
</section>
{{end}}
</body>
</html>
`))
