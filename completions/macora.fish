complete -c macora -f
complete -c macora -n '__fish_use_subcommand' -a 'interactive repl tui topology report resolve show explain routes path dns services wifi neighbors sockets doctor diagnose ping trace watch snapshot diff subnet probe set completions'
complete -c macora -l all
complete -c macora -l up
complete -c macora -l interface -x
complete -c macora -l group -x
complete -c macora -l ipv4
complete -c macora -l ipv6
complete -c macora -l compact
complete -c macora -l json
complete -c macora -l format -x -a 'table json ndjson md html prometheus'
complete -c macora -l redact
complete -c macora -l from -r
complete -c macora -l color -x -a 'auto always never'
complete -c macora -l no-color
complete -c macora -l interval -x
complete -c macora -l count -x
complete -c macora -l timeout-ms -x
complete -c macora -l apply
complete -c macora -l theme -x -a 'default mono high-contrast'
complete -c macora -l help
complete -c macora -l version
