package main

import "testing"

func TestNormalizeForDedupe(t *testing.T) {
	cases := []struct{ in, want string }{
		{"https://example.com/", "https://example.com"},
		{"https://example.com", "https://example.com"},
		{"HTTPS://EXAMPLE.COM/x", "https://example.com/x"},
		{"http://example.com:80/x", "http://example.com/x"},
		{"https://example.com:443/x", "https://example.com/x"},
		{"https://example.com:8443/x", "https://example.com:8443/x"},
		{"https://example.com/x?utm_source=a&y=1", "https://example.com/x?y=1"},
		{"https://example.com/x?fbclid=1&y=1", "https://example.com/x?y=1"},
		{"https://example.com/x#frag", "https://example.com/x"},
		{"https://example.com/X", "https://example.com/X"},
	}
	for _, c := range cases {
		if got := normalizeForDedupe(c.in); got != c.want {
			t.Errorf("normalizeForDedupe(%q) = %q, want %q", c.in, got, c.want)
		}
	}
}
