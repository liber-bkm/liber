package main

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"
)

const (
	fldID          = 1
	fldTitle       = 2
	fldURL         = 3
	fldTags        = 4
	fldFolder      = 5
	fldDescription = 6
	fldBadges      = 7
)

func fzfAvailable() bool {
	_, err := exec.LookPath("fzf")
	return err == nil
}

func selfPath() string {
	p, err := os.Executable()
	if err != nil {
		return "liber"
	}
	return p
}

func withNthFor(fields SearchFields) string {
	var idxs []string
	if !fields.Any() {
		idxs = []string{"2", "3", "4", "5"}
	} else {
		if fields.Title {
			idxs = append(idxs, "2")
		}
		if fields.URL {
			idxs = append(idxs, "3")
		}
		if fields.Tags {
			idxs = append(idxs, "4")
		}
		if fields.Folder {
			idxs = append(idxs, "5")
		}
		if fields.Description {
			idxs = append(idxs, "6")
		}
	}
	idxs = append(idxs, "7")
	return strings.Join(idxs, ",")
}

func sanitizeField(s string, maxLen int) string {
	s = strings.Join(strings.Fields(s), " ")
	if len(s) > maxLen {
		s = s[:maxLen] + "\u2026"
	}
	return s
}

func pickWithFzf(list []*Bookmark, fields SearchFields) (id int, ok bool, err error) {
	var buf bytes.Buffer
	for _, b := range list {
		folder := b.Folder
		if folder == "" {
			folder = "/"
		}
		tags := "(none)"
		if len(b.Tags) > 0 {
			tags = "#" + strings.Join(b.Tags, " #")
		}
		desc := sanitizeField(b.Description, 200)
		if desc == "" {
			desc = "(none)"
		}

		var badgeParts []string
		if b.MarkdownFile != "" {
			badgeParts = append(badgeParts, "md")
		}
		if b.ArchiveFile != "" {
			badgeParts = append(badgeParts, "arc")
		}
		if n := len(b.Attachments); n == 1 {
			badgeParts = append(badgeParts, "att")
		} else if n > 1 {
			badgeParts = append(badgeParts, fmt.Sprintf("att%d", n))
		}
		badges := ""
		if len(badgeParts) > 0 {
			badges = "\x1b[2m[" + strings.Join(badgeParts, ",") + "]\x1b[0m"
		}

		fmt.Fprintf(&buf, "%d\t\x1b[1m%s\x1b[0m\t\x1b[34m%s\x1b[0m\t\x1b[33m%s\x1b[0m\t\x1b[32m%s\x1b[0m\t\x1b[36m%s\x1b[0m\t%s\n",
			b.ID,
			sanitizeField(b.Title, 200),
			sanitizeField(b.URL, 200),
			sanitizeField(tags, 200),
			sanitizeField(folder, 200),
			desc,
			badges,
		)
	}

	previewCmd := fmt.Sprintf("%s __preview {%d}", shellQuote(selfPath()), fldID)

	// NOTE: --delimiter must be the regex escape "\t", not a literal tab byte -- see dev-docs.md#fzf-integration.
	cmd := exec.Command("fzf",
		"--ansi",
		"--delimiter=\\t",
		"--with-nth="+withNthFor(fields),
		"--prompt=liber> ",
		"--header="+fields.Label()+"    (preview: full details)",
		"--height=80%",
		"--layout=reverse",
		"--border",
		"--no-multi",
		"--preview", previewCmd,
		"--preview-window=right:50%:wrap",
	)
	cmd.Stdin = &buf
	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	out, runErr := cmd.Output()
	if runErr != nil {
		if exitErr, isExit := runErr.(*exec.ExitError); isExit {
			switch exitErr.ExitCode() {
			case 1, 130: // user cancelled, not an error
				return 0, false, nil
			default:
				msg := strings.TrimSpace(stderr.String())
				if msg == "" {
					msg = fmt.Sprintf("exit code %d", exitErr.ExitCode())
				}
				return 0, false, fmt.Errorf("%s", msg)
			}
		}
		return 0, false, runErr
	}

	line := strings.TrimRight(string(out), "\n")
	if line == "" {
		return 0, false, nil
	}
	parts := strings.SplitN(line, "\t", 2)
	parsedID, convErr := strconv.Atoi(strings.TrimSpace(parts[0]))
	if convErr != nil {
		return 0, false, nil
	}
	return parsedID, true, nil
}

func shellQuote(s string) string {
	return "'" + strings.ReplaceAll(s, "'", `'\''`) + "'"
}
