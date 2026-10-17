_macora() {
  local cur="${COMP_WORDS[COMP_CWORD]}" prev="${COMP_WORDS[COMP_CWORD-1]}"
  case "$prev" in
    format) COMPREPLY=($(compgen -W "table json ndjson md html prometheus" -- "$cur")); return;;
    theme) COMPREPLY=($(compgen -W "default mono high-contrast" -- "$cur")); return;;
    color) COMPREPLY=($(compgen -W "auto always never" -- "$cur")); return;;
    group) COMPREPLY=($(compgen -W "Network Bridges Virtual Tunnels System Loopback" -- "$cur")); return;;
    from) COMPREPLY=($(compgen -f -- "$cur")); return;;
    interface|explain|set) COMPREPLY=($(compgen -W "$(macora list 2>/dev/null)" -- "$cur")); return;;
  esac
  if [[ $COMP_CWORD -eq 1 ]]; then
    COMPREPLY=($(compgen -W "interactive repl tui topology report resolve show explain routes path dns services wifi neighbors sockets doctor diagnose ping trace watch snapshot diff subnet probe set completions all up down list compact ipv4 ipv6 raw json redact html md prometheus apply from interface group count interval timeout format theme color $(macora list 2>/dev/null)" -- "$cur"))
  else
    COMPREPLY=($(compgen -W "all up down list compact ipv4 ipv6 raw json redact html md prometheus apply from interface group count interval timeout format theme color $(macora list 2>/dev/null)" -- "$cur"))
  fi
}
complete -F _macora macora
