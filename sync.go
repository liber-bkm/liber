package main

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

func findRepoRoot(startDir string) (root string, isJJ bool, isGit bool) {
	dir := startDir
	for i := 0; i < 40; i++ {
		if fileExists(filepath.Join(dir, ".jj")) {
			return dir, true, false
		}
		if fileExists(filepath.Join(dir, ".git")) {
			return dir, false, true
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			break // reached filesystem root
		}
		dir = parent
	}
	return "", false, false
}

func parseSyncFlags(args []string) (push bool, err error) {
	for _, a := range args {
		switch a {
		case "-p", "--push":
			push = true
		default:
			return false, fmt.Errorf("unknown flag for --sync: %s", a)
		}
	}
	return push, nil
}

func runSync(push bool) error {
	cfg, _, err := LoadConfig()
	if err != nil {
		return fmt.Errorf("loading config: %w", err)
	}
	baseDir := cfg.effectiveBaseDir()

	root, isJJ, isGit := findRepoRoot(baseDir)
	if root == "" {
		fmt.Printf("%s doesn't look like it's inside a jj or git repo.\n", baseDir)
		fmt.Println("Run `jj git init` or `git init` there first if you want to sync it.")
		return nil
	}

	msg := fmt.Sprintf("liber sync: %s", time.Now().Format("2006-01-02 15:04:05"))

	if isJJ {
		fmt.Println("Committing with jj ...")
		if err := runInDir(root, "jj", "commit", "-m", msg); err != nil {
			return err
		}
		if push {
			fmt.Println("Pushing with jj ...")
			return runInDir(root, "jj", "git", "push")
		}
		fmt.Println("Done. Push with `jj git push` if you have a remote set up (or run `liber --sync -p`).")
		return nil
	}

	if !isGit {
		return fmt.Errorf("internal error: repo at %s is neither jj nor git", root)
	}
	fmt.Println("Committing with git ...")
	if err := runInDir(root, "git", "add", "-A"); err != nil {
		return err
	}
	committed, err := runGitCommit(root, msg)
	if err != nil {
		return err
	}
	if push {
		fmt.Println("Pushing with git ...")
		return runInDir(root, "git", "push")
	}
	if committed {
		fmt.Println("Done. Push with `git push` if you have a remote set up (or run `liber --sync -p`).")
	}
	return nil
}

func runInDir(dir, name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	if len(out) > 0 {
		fmt.Print(string(out))
	}
	if err != nil {
		return fmt.Errorf("%s %s: %w", name, strings.Join(args, " "), err)
	}
	return nil
}

func runGitCommit(dir, msg string) (committed bool, err error) {
	cmd := exec.Command("git", "commit", "-m", msg)
	cmd.Dir = dir
	out, runErr := cmd.CombinedOutput()
	if runErr != nil {
		if strings.Contains(string(out), "nothing to commit") {
			fmt.Println("Nothing new to sync.")
			return false, nil
		}
		fmt.Print(string(out))
		return false, fmt.Errorf("git commit: %w", runErr)
	}
	fmt.Print(string(out))
	return true, nil
}
