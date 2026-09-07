package main

import (
	"bytes"
	"fmt"
	"html"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"
	"time"
)

func runArchive(cfg Config, url, outPath string) error {
	switch strings.ToLower(strings.TrimSpace(cfg.ArchiveBackend)) {
	case "single-file", "singlefile":
		fmt.Println("Archiving with single-file ...")
		return runSingleFile(cfg, url, outPath)
	case "monolith":
		fmt.Println("Archiving with monolith ...")
		return runMonolith(cfg, url, outPath)
	case "native":
		fmt.Println("Archiving with native snapshot ...")
		return runNativeArchive(url, outPath)
	case "", "auto":
		if findCmd(cfg.SingleFileCmd, "single-file") != "" {
			fmt.Println("Archiving with single-file ...")
			if err := runSingleFile(cfg, url, outPath); err == nil {
				return nil
			} else {
				fmt.Printf("warning: single-file archive failed: %v -- trying monolith\n", err)
			}
		}
		if findCmd(cfg.MonolithCmd, "monolith") != "" {
			fmt.Println("Archiving with monolith ...")
			if err := runMonolith(cfg, url, outPath); err == nil {
				return nil
			} else {
				fmt.Printf("warning: monolith archive failed: %v -- trying native snapshot\n", err)
			}
		}
		fmt.Println("Archiving with native snapshot ...")
		return runNativeArchive(url, outPath)
	default:
		return fmt.Errorf("unknown archive_backend %q -- expected auto, single-file, monolith, or native", cfg.ArchiveBackend)
	}
}

func runMonolith(cfg Config, url, outPath string) error {
	cmdName := cfg.MonolithCmd
	if cmdName == "" {
		cmdName = "monolith"
	}
	if _, err := exec.LookPath(cmdName); err != nil {
		return fmt.Errorf("%q not found in PATH -- install monolith (cargo/brew/pacman/your package manager)", cmdName)
	}
	if err := os.MkdirAll(filepath.Dir(outPath), 0o755); err != nil {
		return err
	}
	if cfg.MonolithUseBrowser {
		return runMonolithPiped(cfg, url, outPath, cmdName)
	}
	cmd := exec.Command(cmdName, url, "-o", outPath)
	out, err := cmd.CombinedOutput()
	if err != nil {
		msg := strings.TrimSpace(string(out))
		if msg != "" {
			return fmt.Errorf("%s: %s", err, msg)
		}
		return err
	}
	return nil
}

func runMonolithPiped(cfg Config, url, outPath, monolithCmd string) error {
	browser := findMonolithBrowser(cfg)
	if browser == "" {
		return fmt.Errorf("monolith_use_browser is on but no chromium found -- set monolith_browser_path")
	}
	chromium := exec.Command(browser,
		"--headless", "--window-size=1920,1080",
		"--run-all-compositor-stages-before-draw",
		"--virtual-time-budget=9000", "--incognito",
		"--dump-dom", url)
	monolith := exec.Command(monolithCmd, "-", "-I", "-b", url, "-o", outPath)

	pipe, err := chromium.StdoutPipe()
	if err != nil {
		return err
	}
	monolith.Stdin = pipe
	var chromiumErr, monolithErr bytes.Buffer
	chromium.Stderr = &chromiumErr
	monolith.Stderr = &monolithErr

	if err := monolith.Start(); err != nil {
		return err
	}
	if err := chromium.Start(); err != nil {
		return fmt.Errorf("could not start browser %q: %w", browser, err)
	}
	var cerr, merr error
	cerr = chromium.Wait()
	merr = monolith.Wait()
	if cerr != nil {
		msg := strings.TrimSpace(chromiumErr.String())
		if msg != "" {
			return fmt.Errorf("chromium: %v: %s", cerr, msg)
		}
		return fmt.Errorf("chromium: %v", cerr)
	}
	if merr != nil {
		msg := strings.TrimSpace(monolithErr.String())
		if msg != "" {
			return fmt.Errorf("monolith: %v: %s", merr, msg)
		}
		return fmt.Errorf("monolith: %v", merr)
	}
	return nil
}

func findMonolithBrowser(cfg Config) string {
	if p := cfg.MonolithBrowserPath; p != "" {
		if _, err := exec.LookPath(p); err == nil {
			return p
		}
		return ""
	}
	for _, name := range []string{"chromium", "chromium-browser", "google-chrome"} {
		if path, err := exec.LookPath(name); err == nil {
			return path
		}
	}
	return ""
}

func findCmd(configured, fallback string) string {
	if configured != "" {
		if _, err := exec.LookPath(configured); err == nil {
			return configured
		}
		return ""
	}
	if _, err := exec.LookPath(fallback); err == nil {
		return fallback
	}
	return ""
}

func runSingleFile(cfg Config, url, outPath string) error {
	cmdName := cfg.SingleFileCmd
	if cmdName == "" {
		cmdName = "single-file"
	}
	if _, err := exec.LookPath(cmdName); err != nil {
		return fmt.Errorf("%q not found in PATH — install it with `npm install -g single-file-cli`", cmdName)
	}
	if err := os.MkdirAll(filepath.Dir(outPath), 0o755); err != nil {
		return err
	}
	args := []string{}
	if cfg.SingleFileBrowserPath != "" {
		args = append(args, "--browser-executable-path="+cfg.SingleFileBrowserPath)
	}
	args = append(args, url, outPath)
	cmd := exec.Command(cmdName, args...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		msg := strings.TrimSpace(string(out))
		if msg != "" {
			return fmt.Errorf("%s: %s", err, msg)
		}
		return err
	}
	return nil
}

var titleRe = regexp.MustCompile(`(?is)<title[^>]*>(.*?)</title>`)

func fetchTitle(rawURL string) string {
	client := http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequest("GET", rawURL, nil)
	if err != nil {
		return ""
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (compatible; liber-bookmark-manager/1.0)")
	resp, err := client.Do(req)
	if err != nil {
		return ""
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 400 {
		return ""
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, 300*1024))
	if err != nil {
		return ""
	}
	m := titleRe.FindSubmatch(body)
	if m == nil {
		return ""
	}
	t := strings.TrimSpace(string(m[1]))
	t = html.UnescapeString(t)
	t = strings.Join(strings.Fields(t), " ")
	return t
}

func openURL(cfg Config, url string) error {
	if cfg.BrowserCmd != "" {
		return exec.Command(cfg.BrowserCmd, url).Start()
	}
	switch runtime.GOOS {
	case "darwin":
		return exec.Command("open", url).Start()
	case "windows":
		return exec.Command("rundll32", "url.dll,FileProtocolHandler", url).Start()
	default:
		return exec.Command("xdg-open", url).Start()
	}
}

func openInEditor(cfg Config, path string) error {
	editor := cfg.EditorCmd
	if editor == "" {
		editor = os.Getenv("VISUAL")
	}
	if editor == "" {
		editor = os.Getenv("EDITOR")
	}
	if editor != "" {
		cmd := exec.Command(editor, path)
		cmd.Stdin = os.Stdin
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		return cmd.Run()
	}
	return openURL(cfg, path)
}
