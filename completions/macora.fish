complete -c macora -f
complete -c macora -n '__fish_use_subcommand' -a 'interactive repl tui topology report resolve show explain routes path dns services wifi neighbors sockets doctor diagnose ping trace watch snapshot diff subnet probe set completions'
complete -c macora -n '__fish_use_subcommand' -a '(macora list 2>/dev/null)'
complete -c macora -n 'not __fish_use_subcommand; and not __fish_prev_arg_in to' -a 'all up down list compact ipv4 ipv6 raw json redact html md prometheus apply from interface group count interval timeout format theme color speed quick strict to'
complete -c macora -n 'not __fish_use_subcommand; and not __fish_prev_arg_in to' -a '(macora list 2>/dev/null)'
complete -c macora -n '__fish_prev_arg_in format' -a 'table json ndjson md html prometheus'
complete -c macora -n '__fish_prev_arg_in theme' -a 'default mono high-contrast'
complete -c macora -n '__fish_prev_arg_in color' -a 'auto always never'
complete -c macora -n '__fish_prev_arg_in group' -a 'Network Bridges Virtual Tunnels System Loopback'
complete -c macora -n '__fish_prev_arg_in from' -F
