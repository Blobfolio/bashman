_basher__bashman_action() {
	local cur prev opts
	COMPREPLY=()
	cur="${COMP_WORDS[COMP_CWORD]}"
	prev="${COMP_WORDS[COMP_CWORD-1]}"
	opts=()
	if [[ ! " ${COMP_LINE} " =~ " -h " ]] && [[ ! " ${COMP_LINE} " =~ " --help " ]]; then
		opts+=("-h")
		opts+=("--help")
	fi
	if [[ ! " ${COMP_LINE} " =~ " -V " ]] && [[ ! " ${COMP_LINE} " =~ " --version " ]]; then
		opts+=("-V")
		opts+=("--version")
	fi
	if [[ ! " ${COMP_LINE} " =~ " -m " ]] && [[ ! " ${COMP_LINE} " =~ " --manifest-path " ]]; then
		opts+=("-m")
		opts+=("--manifest-path")
	fi
	opts=" ${opts[@]} "
	if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
		COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
		return 0
	fi
	case "${prev}" in
		--manifest-path|-m)
			if [ -z "$( declare -f _filedir )" ]; then
				COMPREPLY=( $( compgen -f "${cur}" ) )
			else
				COMPREPLY=( $( _filedir ) )
			fi
			return 0
			;;
		*)
			COMPREPLY=()
			;;
	esac
	COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
	return 0
}
_basher__bashman_make() {
	local cur prev opts
	COMPREPLY=()
	cur="${COMP_WORDS[COMP_CWORD]}"
	prev="${COMP_WORDS[COMP_CWORD-1]}"
	opts=()
	if [[ ! " ${COMP_LINE} " =~ " -h " ]] && [[ ! " ${COMP_LINE} " =~ " --help " ]]; then
		opts+=("-h")
		opts+=("--help")
	fi
	[[ " ${COMP_LINE} " =~ " --no-bash " ]] || opts+=("--no-bash")
	[[ " ${COMP_LINE} " =~ " --no-credits " ]] || opts+=("--no-credits")
	[[ " ${COMP_LINE} " =~ " --no-man " ]] || opts+=("--no-man")
	if [[ ! " ${COMP_LINE} " =~ " -m " ]] && [[ ! " ${COMP_LINE} " =~ " --manifest-path " ]]; then
		opts+=("-m")
		opts+=("--manifest-path")
	fi
	opts=" ${opts[@]} "
	if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
		COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
		return 0
	fi
	case "${prev}" in
		--manifest-path|-m)
			if [ -z "$( declare -f _filedir )" ]; then
				COMPREPLY=( $( compgen -f "${cur}" ) )
			else
				COMPREPLY=( $( _filedir ) )
			fi
			return 0
			;;
		*)
			COMPREPLY=()
			;;
	esac
	COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
	return 0
}
_basher___bashman() {
	local cur prev opts
	COMPREPLY=()
	cur="${COMP_WORDS[COMP_CWORD]}"
	prev="${COMP_WORDS[COMP_CWORD-1]}"
	opts=()
	if [[ ! " ${COMP_LINE} " =~ " -h " ]] && [[ ! " ${COMP_LINE} " =~ " --help " ]]; then
		opts+=("-h")
		opts+=("--help")
	fi
	if [[ ! " ${COMP_LINE} " =~ " -f " ]] && [[ ! " ${COMP_LINE} " =~ " --features " ]]; then
		opts+=("-f")
		opts+=("--features")
	fi
	opts+=("action")
	opts+=("make")
	opts=" ${opts[@]} "
	if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
		COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
		return 0
	fi
	COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
	return 0
}
subcmd__basher___bashman() {
	local i cmd
	COMPREPLY=()
	cmd=""
	for i in ${COMP_WORDS[@]}; do
		case "${i}" in
			bashman)
				cmd="bashman"
				;;
			action)
				cmd="action"
				;;
			make)
				cmd="make"
				;;
			*)
				;;
		esac
	done
	echo "$cmd"
}
chooser__basher___bashman() {
	local i cmd
	COMPREPLY=()
	cmd="$( subcmd__basher___bashman )"
	case "${cmd}" in
		bashman)
			_basher___bashman
			;;
		action)
			_basher__bashman_action
			;;
		make)
			_basher__bashman_make
			;;
		*)
			;;
	esac
}
complete -F chooser__basher___bashman -o bashdefault -o default bashman
