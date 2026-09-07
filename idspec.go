package main

import (
	"fmt"
	"sort"
	"strconv"
	"strings"
)

func consumeIDSpec(args []string) (spec string, rest []string) {
	var sb strings.Builder
	i := 0
	for i < len(args) && !strings.HasPrefix(args[i], "-") {
		sb.WriteString(args[i])
		i++
	}
	return sb.String(), args[i:]
}

func parseIDSpec(spec string) ([]int, error) {
	seen := map[int]bool{}
	var ids []int

	for _, part := range strings.Split(spec, ",") {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		if strings.Contains(part, "-") {
			bounds := strings.SplitN(part, "-", 2)
			lo, errLo := strconv.Atoi(strings.TrimSpace(bounds[0]))
			hi, errHi := strconv.Atoi(strings.TrimSpace(bounds[1]))
			if len(bounds) != 2 || errLo != nil || errHi != nil {
				return nil, fmt.Errorf("invalid range %q", part)
			}
			if lo > hi {
				lo, hi = hi, lo
			}
			for i := lo; i <= hi; i++ {
				if !seen[i] {
					seen[i] = true
					ids = append(ids, i)
				}
			}
			continue
		}
		n, err := strconv.Atoi(part)
		if err != nil {
			return nil, fmt.Errorf("invalid id %q", part)
		}
		if !seen[n] {
			seen[n] = true
			ids = append(ids, n)
		}
	}

	if len(ids) == 0 {
		return nil, fmt.Errorf("no ids given")
	}
	sort.Ints(ids)
	return ids, nil
}

func joinInts(ids []int) string {
	parts := make([]string, len(ids))
	for i, id := range ids {
		parts[i] = strconv.Itoa(id)
	}
	return strings.Join(parts, ", ")
}
