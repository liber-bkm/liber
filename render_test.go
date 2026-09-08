package main

import (
	"strings"
	"testing"
)

func TestRenderMarkdownHTML(t *testing.T) {
	src := "---\ntitle: \"T\"\n---\n\n# Head\n\nSome **bold** and *italic* with `code`.\n\n- a\n- b\n\n[Visit original](https://example.com)\n\n```\nraw <tag>\n```\n"
	html := renderMarkdownHTML(src)
	for _, want := range []string{
		"<h1>Head</h1>",
		"<strong>bold</strong>",
		"<em>italic</em>",
		"<code>code</code>",
		"<li>a</li>",
		`<a href="https://example.com">Visit original</a>`,
		"raw &lt;tag&gt;",
	} {
		if !strings.Contains(html, want) {
			t.Errorf("rendered markdown missing %q in:\n%s", want, html)
		}
	}
	if strings.Contains(html, "title:") {
		t.Errorf("frontmatter leaked into output:\n%s", html)
	}
}

func TestRenderMarkdownXSS(t *testing.T) {
	html := renderMarkdownHTML("# T\n\n<script>alert(1)</script>\n")
	if strings.Contains(html, "<script>alert") {
		t.Errorf("unescaped script in output:\n%s", html)
	}
}
