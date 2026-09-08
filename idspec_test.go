package main

import (
	"reflect"
	"testing"
)

func TestParseIDSpec(t *testing.T) {
	cases := []struct {
		in      string
		want    []int
		wantErr bool
	}{
		{"3", []int{3}, false},
		{"1-3", []int{1, 2, 3}, false},
		{"5-2", []int{2, 3, 4, 5}, false},
		{"2,5,3", []int{2, 3, 5}, false},
		{"1-4,7-9", []int{1, 2, 3, 4, 7, 8, 9}, false},
		{"1-3,2", []int{1, 2, 3}, false},
		{"x", nil, true},
		{"1-", nil, true},
		{"", nil, true},
	}
	for _, c := range cases {
		got, err := parseIDSpec(c.in)
		if c.wantErr {
			if err == nil {
				t.Errorf("parseIDSpec(%q) expected error", c.in)
			}
			continue
		}
		if err != nil {
			t.Errorf("parseIDSpec(%q) unexpected error: %v", c.in, err)
			continue
		}
		if !reflect.DeepEqual(got, c.want) {
			t.Errorf("parseIDSpec(%q) = %v, want %v", c.in, got, c.want)
		}
	}
}

func TestIsQuerySpec(t *testing.T) {
	ids := []string{"3", "1-3", "2,5,3", "1-4,7-9", "1-4, 7-9"}
	for _, s := range ids {
		if isQuerySpec(s) {
			t.Errorf("isQuerySpec(%q) = true, want false", s)
		}
	}
	queries := []string{"alpha", "my query", "1-4,x", "v2"}
	for _, s := range queries {
		if !isQuerySpec(s) {
			t.Errorf("isQuerySpec(%q) = false, want true", s)
		}
	}
}
