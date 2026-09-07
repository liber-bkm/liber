package main

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

func attachFile(cfg Config, b *Bookmark, src string) error {
	src = expandTilde(strings.TrimSpace(src))
	info, err := os.Stat(src)
	if err != nil {
		return fmt.Errorf("cannot read %s: %w", src, err)
	}
	if info.IsDir() {
		return fmt.Errorf("%s is a directory -- attach files, not folders", src)
	}
	f, err := os.Open(src)
	if err != nil {
		return fmt.Errorf("cannot read %s: %w", src, err)
	}
	defer f.Close()
	return attachReader(cfg, b, filepath.Base(src), f)
}

func attachReader(cfg Config, b *Bookmark, name string, r io.Reader) error {
	name = filepath.Base(name)
	if name == "" || name == "." || name == "/" {
		return fmt.Errorf("attachment needs a filename")
	}
	rel := uniqueAttachmentRel(cfg, b, name)
	dst := filepath.Join(cfg.attachmentsDir(), rel)
	if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
		return err
	}
	f, err := os.Create(dst)
	if err != nil {
		return err
	}
	_, copyErr := io.Copy(f, r)
	closeErr := f.Close()
	if copyErr != nil || closeErr != nil {
		os.Remove(dst)
		if copyErr != nil {
			return copyErr
		}
		return closeErr
	}
	b.Attachments = append(b.Attachments, Attachment{Name: name, File: rel})
	return nil
}

func uniqueAttachmentRel(cfg Config, b *Bookmark, name string) string {
	ext := filepath.Ext(name)
	stem := slugify(strings.TrimSuffix(filepath.Base(name), ext))
	if stem == "" {
		stem = "attachment"
	}
	base := fmt.Sprintf("%04d-%s", b.ID, stem)
	rel := base + ext
	for n := 2; attachmentNameTaken(cfg, b, rel); n++ {
		rel = fmt.Sprintf("%s-%d%s", base, n, ext)
	}
	return rel
}

func attachmentNameTaken(cfg Config, b *Bookmark, rel string) bool {
	for _, at := range b.Attachments {
		if at.File == rel {
			return true
		}
	}
	return fileExists(filepath.Join(cfg.attachmentsDir(), rel))
}

func findAttachment(b *Bookmark, match string) (int, error) {
	if len(b.Attachments) == 0 {
		return -1, fmt.Errorf("this bookmark has no attachments")
	}
	if n, err := strconv.Atoi(match); err == nil {
		if n < 1 || n > len(b.Attachments) {
			return -1, fmt.Errorf("no attachment #%d (has %d)", n, len(b.Attachments))
		}
		return n - 1, nil
	}
	var hits []int
	for i, at := range b.Attachments {
		if strings.EqualFold(at.Name, match) {
			hits = append(hits, i)
		}
	}
	switch len(hits) {
	case 1:
		return hits[0], nil
	case 0:
		return -1, fmt.Errorf("no attachment named %q", match)
	default:
		return -1, fmt.Errorf("%q matches %d attachments -- use its number", match, len(hits))
	}
}

func detachAttachment(cfg Config, b *Bookmark, i int) {
	at := b.Attachments[i]
	if at.File != "" {
		os.Remove(filepath.Join(cfg.attachmentsDir(), at.File))
	}
	b.Attachments = append(b.Attachments[:i], b.Attachments[i+1:]...)
}

func attachOrWarn(cfg Config, b *Bookmark, path string) {
	if err := attachFile(cfg, b, path); err != nil {
		fmt.Println("Could not attach:", err)
		return
	}
	fmt.Printf("Attached %s\n", b.Attachments[len(b.Attachments)-1].Name)
}

func attachmentsMenu(cfg Config, b *Bookmark) {
	for {
		fmt.Println("  attachments:")
		if len(b.Attachments) == 0 {
			fmt.Println("    (none)")
		}
		for i, at := range b.Attachments {
			fmt.Printf("    %d) %s\n", i+1, at.Name)
		}
		line := promptLine("  open <#> | add <path> | rm <#|name> | enter = done")
		if line == "" {
			return
		}
		if rest, ok := strings.CutPrefix(line, "add "); ok {
			attachOrWarn(cfg, b, rest)
			continue
		}
		if rest, ok := strings.CutPrefix(line, "rm "); ok {
			i, err := findAttachment(b, strings.TrimSpace(rest))
			if err != nil {
				fmt.Println(err)
				continue
			}
			detachAttachment(cfg, b, i)
			fmt.Println("Removed.")
			continue
		}
		if fileExists(expandTilde(line)) {
			attachOrWarn(cfg, b, line)
			continue
		}
		i, err := findAttachment(b, line)
		if err != nil {
			fmt.Println(err)
			continue
		}
		at := b.Attachments[i]
		if err := openURL(cfg, filepath.Join(cfg.attachmentsDir(), at.File)); err != nil {
			fmt.Println("Could not open attachment:", err)
		}
	}
}
