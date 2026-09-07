package main

import "fmt"

func runCompletion(args []string) error {
	if len(args) != 1 {
		return fmt.Errorf("usage: liber completion bash|zsh|fish")
	}
	switch args[0] {
	case "bash":
		fmt.Print(bashCompletion)
	case "zsh":
		fmt.Print(zshCompletion)
	case "fish":
		fmt.Print(fishCompletion)
	default:
		return fmt.Errorf("unknown shell %q -- expected bash, zsh, or fish", args[0])
	}
	return nil
}

const bashCompletion = `# liber bash completion -- install with:
#   liber completion bash > ~/.local/share/bash-completion/completions/liber
_liber_tags()    { liber --tags 2>/dev/null | awk '{print $1}'; }
_liber_folders() { liber --folders 2>/dev/null | awk '{print $1}'; }
_liber_ids()     { liber -l 2>/dev/null | awk -F'[][]' '/^\[/ {print $2}'; }

_liber() {
	local cur prev
	COMPREPLY=()
	cur="${COMP_WORDS[COMP_CWORD]}"
	prev="${COMP_WORDS[COMP_CWORD-1]}"

	case "$prev" in
		-t|--tag)      COMPREPLY=( $(compgen -W "$(_liber_tags)" -- "$cur") ); return 0 ;;
		-f|--folder)   COMPREPLY=( $(compgen -W "$(_liber_folders)" -- "$cur") ); return 0 ;;
		--import)      compopt -o default 2>/dev/null; return 0 ;;
		--export-site) compopt -o default 2>/dev/null; return 0 ;;
		completion)    COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") ); return 0 ;;
	esac

	case "${COMP_WORDS[1]}" in
		-e|-d|-o|--open)
			COMPREPLY=( $(compgen -W "$(_liber_ids)" -- "$cur") ); return 0 ;;
		--auto)
			COMPREPLY=( $(compgen -W "add list edit delete apply learn --match --folder --tag --reapply --min --create" -- "$cur") ); return 0 ;;
		--profile)
			COMPREPLY=( $(compgen -W "default delete" -- "$cur") ); return 0 ;;
		--tags|--folders)
			if [ "$COMP_CWORD" -eq 2 ]; then
				COMPREPLY=( $(compgen -W "rename delete" -- "$cur") ); return 0
			fi
			if [ "$prev" = "rename" ] || [ "$prev" = "delete" ]; then
				if [ "${COMP_WORDS[1]}" = "--tags" ]; then
					COMPREPLY=( $(compgen -W "$(_liber_tags)" -- "$cur") )
				else
					COMPREPLY=( $(compgen -W "$(_liber_folders)" -- "$cur") )
				fi
				return 0
			fi
			;;
	esac

	if [ "$COMP_CWORD" -eq 1 ] && [[ "$cur" == -* ]]; then
		COMPREPLY=( $(compgen -W "\
-s -sl -sn -su -st -sd -sf -l -e -d -o --open -r \
--import --tags --folders --history --auto --profile --sync --serve --export-site \
--check pick \
completion config -v -h" -- "$cur") )
	fi
	return 0
}
complete -F _liber liber
`

const zshCompletion = `#compdef liber
# liber zsh completion -- install somewhere on $fpath, e.g.:
#   liber completion zsh > ~/.zsh/completions/_liber && compinit

_liber() {
	# complete the argument of the previous word, when it demands one
	case "$words[CURRENT-1]" in
		-t|--tag)       compadd -- ${(f)"$(liber --tags 2>/dev/null | awk '{print $1}')"}; return ;;
		-f|--folder)    compadd -- ${(f)"$(liber --folders 2>/dev/null | awk '{print $1}')"}; return ;;
		--import)       _files; return ;;
		--export-site)  _files -/; return ;;
		completion)     compadd bash zsh fish; return ;;
	esac

	# complete arguments based on the leading flag
	case "$words[2]" in
		-e|-d|-o|--open)
			compadd -- ${(f)"$(liber -l 2>/dev/null | awk -F'[][]' '/^\[/ {print $2}')"}; return ;;
		--auto)    compadd add list edit delete apply learn; return ;;
		--profile) compadd default delete; return ;;
		--tags)    compadd rename delete; return ;;
		--folders) compadd rename delete; return ;;
	esac

	if (( CURRENT == 2 )); then
		_arguments -S \
			'-s[search bookmarks (fzf)]' \
			'-sl[search, plain prompt]' \
			'-l[list bookmarks]' \
			'-e[edit bookmark(s)]' \
			'-d[delete bookmark(s)]' \
			'(-o --open)'{-o,--open}'[open bookmark(s) in the browser]' \
			'-r[reindex]' \
			'--import[import browser bookmark export]' \
			'--tags[list/manage tags]' \
			'--folders[list/manage folders]' \
			'--history[most recently opened]' \
			'--check[check link health]' \
			'pick[print a matching URL]' \
			'--auto[manage automation rules]' \
			'--profile[manage profiles]' \
			'--sync[commit the collection (jj/git)]' \
			'--serve[web UI]' \
			'--export-site[write a static index]' \
			'completion[print a completion script]' \
			'config[show config]' \
			'(-v --version)'{-v,--version}'[print version]' \
			'(-h --help)'{-h,--help}'[help]' \
			'-md[also write a markdown copy]' \
			'-a[also write a page archive]' \
			'-i[interactive prompts]' \
			'-t[tags]' \
			'-f[folder]' \
			'-u[change URL (with -e)]' \
			'-at[attach a file (repeatable)]' \
			'-dt[detach an attachment]' \
			'--deep[search archive content too]' \
			'-y[no confirmation prompts]'
	fi
}
_liber "$@"
`

const fishCompletion = `# liber fish completion -- install with:
#   liber completion fish > ~/.config/fish/completions/liber.fish
function __liber_tags
	liber --tags 2>/dev/null | awk '{print $1}'
end
function __liber_folders
	liber --folders 2>/dev/null | awk '{print $1}'
end
function __liber_ids
	liber -l 2>/dev/null | awk -F'[][]' '/^\[/ {print $2}'
end

complete -c liber -f

# primary flags, only as the first word
complete -c liber -n "test (count (commandline -opc)) -eq 1" -a "-s -sl -sn -su -st -sd -sf" -d "search"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -a "-l -r" -d "list / reindex"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -a "-e -d -o" -d "edit / delete / open"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -s v -l version -d "print version"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -s h -l help -d "help"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l import -r -d "import browser export"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l tags -d "list/manage tags"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l folders -d "list/manage folders"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l history -d "most recently opened"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l check -d "check link health"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -a "pick" -d "print a matching URL"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l auto -d "automation rules"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l profile -d "manage profiles"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l sync -d "commit the collection"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l serve -d "web UI"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -l "export-site" -d "write a static index"
complete -c liber -n "test (count (commandline -opc)) -eq 1" -a "completion config" -d "print completion script / show config"

# ids wherever -e/-d/-o/--open is present
complete -c liber -n "__fish_seen_argument -s e; or __fish_seen_argument -s d; or __fish_seen_argument -s o; or __fish_seen_argument -l open" -a "(__liber_ids)" -d "bookmark id"

# value completion for modifiers
complete -c liber -n "__fish_seen_argument -s t -l tag" -a "(__liber_tags)" -d "tag"
complete -c liber -n "__fish_seen_argument -s f -l folder" -a "(__liber_folders)" -d "folder"
complete -c liber -n "__fish_seen_argument rename delete -l tags" -a "(__liber_tags)" -d "tag"
complete -c liber -n "__fish_seen_argument rename delete -l folders" -a "(__liber_folders)" -d "folder"
complete -c liber -n "__fish_seen_argument -l auto" -a "add list edit delete apply learn" -d "rule operation"
complete -c liber -n "__fish_seen_argument -l auto" -l match -d "match substring (url: host: title:)"
complete -c liber -n "__fish_seen_argument -l auto" -l folder -a "(__liber_folders)" -d "folder"
complete -c liber -n "__fish_seen_argument -l auto" -l tag -a "(__liber_tags)" -d "tag"
complete -c liber -n "__fish_seen_argument -l auto" -l reapply -d "re-sync after edit"
complete -c liber -n "__fish_seen_argument -l auto" -l min -d "learn threshold"
complete -c liber -n "__fish_seen_argument -l auto" -l create -d "create learned rules"
complete -c liber -n "__fish_seen_argument -l check" -l workers -d "parallel requests"
complete -c liber -n "__fish_seen_argument -l profile" -a "default delete" -d "profile"
complete -c liber -n "__fish_seen_argument -l serve" -l addr -d "bind address"
complete -c liber -n "__fish_seen_argument completion" -a "bash zsh fish" -d "shell"

# boolean modifiers (-md/-at/-dt are single-dash multi-char = old-style options in fish)
complete -c liber -o md -d "also a markdown copy"
complete -c liber -o at -rF -d "attach file"
complete -c liber -o dt -d "detach attachment"
complete -c liber -s a -l archive -d "also a page archive"
complete -c liber -s i -l interactive -d "interactive prompts"
complete -c liber -s u -l url -d "change URL (with -e)"
complete -c liber -l deep -d "search archive content too"
complete -c liber -s y -l yes -d "no confirmation prompts"
`
