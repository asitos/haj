#compdef haj

_haj_installed_packages() {
    compadd $(pacman -Qq)
}

_haj_all_packages() {
    compadd $(pacman -Slq)
}

_haj_groups() {
    compadd $(pacman -Sg)
}

_haj_downgrades() {
    local -a pkgs
    if [[ -d /var/cache/pacman/pkg ]]; then
        pkgs+=($(ls /var/cache/pacman/pkg/ 2>/dev/null | grep -E '\.pkg\.tar\.(zst|xz|gz)$' | sed -E 's/-[^-]+-[^-]+-[^-]+\.pkg\.tar\..*$//'))
    fi
    if [[ -d ~/.cache/haj/aur ]]; then
        pkgs+=($(ls ~/.cache/haj/aur/ 2>/dev/null))
    fi
    compadd ${(u)pkgs}
}

_haj() {
    local curcontext="$curcontext" state line
    typeset -A opt_args

    _arguments -C \
        '(-a --aur)'{-a,--aur}'[restrict operations to the aur]' \
        '(-r --repo)'{-r,--repo}'[restrict operations to arch repositories]' \
        '(-y --noconfirm)'{-y,--noconfirm}'[bypass all confirmation prompts]' \
        '(-n --needed)'{-n,--needed}'[do not reinstall up-to-date packages]' \
        '(-i --ignore)'{-i,--ignore}'[ignore a package upgrade]' \
        '(-c --config)'{-c,--config}'[specify an alternate pacman config file]:config file:_files' \
        '--root[specify an alternate installation root]:root dir:_files -/' \
        '(-v --verbose)'{-v,--verbose}'[enable verbose debug logging]' \
        '(-d --dry-run)'{-d,--dry-run}'[preview a command without modifying the system]' \
        '(-V --version)'{-V,--version}'[show version info]' \
        '(-h --help)'{-h,--help}'[display help message]' \
        '1: :->cmds' \
        '*:: :->args'

    case $state in
        cmds)
            local -a subcmds
            subcmds=(
                'tui:launch interactive package manager dashboard'
                't:launch interactive package manager dashboard'
                'update:synchronize remote repositories'
                'up:synchronize remote repositories'
                'sync:synchronize remote repositories'
                'upgrade:full system upgrade'
                'jump:full system upgrade'
                'install:install one or more packages'
                'i:install one or more packages'
                'remove:remove packages & unneeded dependencies'
                'rm:remove packages & unneeded dependencies'
                'toss:remove packages & unneeded dependencies'
                'search:search remote repositories'
                's:search remote repositories'
                'show:show detailed package information'
                'info:show detailed package information'
                'group:browse and install package groups'
                'g:browse and install package groups'
                'list:list installed packages'
                'ls:list installed packages'
                'stats:show system health and package statistics'
                'st:show system health and package statistics'
                'load:install a local package archive'
                'l:install a local package archive'
                'fetch:download a package without installing'
                'f:download a package without installing'
                'downgrade:downgrade an installed package'
                'sink:downgrade an installed package'
                'owns:find which installed package owns a file'
                'ow:find which installed package owns a file'
                'files:list files installed by a package'
                'lf:list files installed by a package'
                'locate:search repositories for a file'
                'loc:search repositories for a file'
                'history:show recent package changes'
                'h:show recent package changes'
                'orphan:detect orphaned dependencies'
                'o:detect orphaned dependencies'
                'clean:clean the package cache'
                'c:clean the package cache'
                'mark:change a package install reason'
                'm:change a package install reason'
                'diff:interactively manage and merge .pacnew files'
                'pn:interactively manage and merge .pacnew files'
            )
            _describe -t commands 'haj commands' subcmds
            ;;
        args)
            case $words[1] in
                install|i|fetch|f)
                    _haj_all_packages
                    ;;
                remove|rm|toss|mark|m|show|info|files|lf)
                    _haj_installed_packages
                    ;;
                downgrade|sink)
                    _haj_downgrades
                    ;;
                group|g)
                    _haj_groups
                    ;;
                load|l)
                    _files -g "*.pkg.tar.zst"
                    ;;
                owns|ow)
                    _files
                    ;;
                list|ls)
                    _arguments \
                        '(-e --explicit)'{-e,--explicit}'[show only explicitly installed packages]' \
                        '(-p --deps)'{-p,--deps}'[show only dependencies]' \
                        '(-f --foreign)'{-f,--foreign}'[show only foreign/aur packages]'
                    ;;
                clean|c)
                    _arguments \
                        '(-k --keep)'{-k,--keep}'[number of package versions to keep]'
                    ;;
                history|h)
                    _arguments \
                        '(-l --limit)'{-l,--limit}'[number of recent changes to show]'
                    ;;
                upgrade|jump)
                    _arguments \
                        '--no-sync[do not sync remote repositories first]'
                    ;;
            esac
            ;;
    esac
}
