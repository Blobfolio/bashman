_basher___cargo_bashman() {
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
	[[ " ${COMP_LINE} " =~ " --print-targets " ]] || opts+=("--print-targets")
	if [[ ! " ${COMP_LINE} " =~ " -V " ]] && [[ ! " ${COMP_LINE} " =~ " --version " ]]; then
		opts+=("-V")
		opts+=("--version")
	fi
	if [[ ! " ${COMP_LINE} " =~ " -m " ]] && [[ ! " ${COMP_LINE} " =~ " --manifest-path " ]]; then
		opts+=("-m")
		opts+=("--manifest-path")
	fi
	if [[ ! " ${COMP_LINE} " =~ " -t " ]] && [[ ! " ${COMP_LINE} " =~ " --target " ]]; then
		opts+=("-t")
		opts+=("--target")
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
complete -F _basher___cargo_bashman -o bashdefault -o default cargo-bashman
