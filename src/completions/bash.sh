_haj_completions() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    # Complete options for commands
    case "$prev" in
        install|i|fetch|f)
            COMPREPLY=( $(compgen -W "$(pacman -Slq)" -- "$cur") )
            return 0
            ;;
        remove|rm|toss|mark|m|show|info|files|lf)
            COMPREPLY=( $(compgen -W "$(pacman -Qq)" -- "$cur") )
            return 0
            ;;
        downgrade|sink)
            local pkgs
            pkgs=$( (ls /var/cache/pacman/pkg/ 2>/dev/null | grep -E '\.pkg\.tar\.(zst|xz|gz)$' | sed -E 's/-[^-]+-[^-]+-[^-]+\.pkg\.tar\..*$//'; ls ~/.cache/haj/aur/ 2>/dev/null) | sort -u )
            COMPREPLY=( $(compgen -W "$pkgs" -- "$cur") )
            return 0
            ;;
        group|g)
            COMPREPLY=( $(compgen -W "$(pacman -Sg)" -- "$cur") )
            return 0
            ;;
        load|l)
            COMPREPLY=( $(compgen -f -X '!*.pkg.tar.zst' -- "$cur") )
            return 0
            ;;
        owns|ow)
            COMPREPLY=( $(compgen -f -- "$cur") )
            return 0
            ;;
        list|ls)
            COMPREPLY=( $(compgen -W "-e --explicit -p --deps -f --foreign" -- "$cur") )
            return 0
            ;;
        clean|c)
            COMPREPLY=( $(compgen -W "-k --keep" -- "$cur") )
            return 0
            ;;
        history|h)
            COMPREPLY=( $(compgen -W "-l --limit" -- "$cur") )
            return 0
            ;;
        upgrade|jump)
            COMPREPLY=( $(compgen -W "--no-sync" -- "$cur") )
            return 0
            ;;
    esac

    # Complete global options if they start with -
    if [[ "$cur" == -* ]]; then
        opts="-a --aur -r --repo -y --noconfirm -n --needed -i --ignore -c --config --root -v --verbose -d --dry-run -V --version -h --help"
        COMPREPLY=( $(compgen -W "$opts" -- "$cur") )
        return 0
    fi

    # Complete commands
    if [ $COMP_CWORD -eq 1 ]; then
        opts="tui t update up sync upgrade jump install i remove rm toss search s show info group g list ls stats st load l fetch f downgrade sink owns ow files lf locate loc history h orphan o clean c mark m diff pn"
        COMPREPLY=( $(compgen -W "$opts" -- "$cur") )
        return 0
    fi
}
complete -F _haj_completions haj
