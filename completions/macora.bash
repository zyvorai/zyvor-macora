_macora() {
  local cur="${COMP_WORDS[COMP_CWORD]}" prev="${COMP_WORDS[COMP_CWORD-1]}"
  case "$prev" in
    --format) COMPREPLY=($(compgen -W "table json ndjson md html prometheus" -- "$cur")); return;;
    --theme) COMPREPLY=($(compgen -W "default mono high-contrast" -- "$cur")); return;;
    --color) COMPREPLY=($(compgen -W "auto always never" -- "$cur")); return;;
    --group) COMPREPLY=($(compgen -W "Network Bridges Virtual Tunnels System Loopback" -- "$cur")); return;;
    --from) COMPREPLY=($(compgen -f -- "$cur")); return;;
  esac
  if [[ "$cur" == -* ]]; then COMPREPLY=($(compgen -W "--all --up --interface --group --ipv4 --ipv6 --compact --json --format --redact --from --color --no-color --interval --count --timeout-ms --apply --theme --help --version" -- "$cur"))
  elif [[ $COMP_CWORD -eq 1 ]]; then COMPREPLY=($(compgen -W "interactive repl tui topology report resolve show explain routes path dns services wifi neighbors sockets doctor diagnose ping trace watch snapshot diff subnet probe set completions" -- "$cur"))
  else COMPREPLY=($(compgen -f -- "$cur")); fi
}
complete -F _macora macora
