# --- HELPER FUNCTIONS FOR DYNAMIC COMPLETIONS ---

function __fish_haj_installed_packages
    pacman -Q 2>/dev/null | string replace ' ' \t
end

function __fish_haj_repo_packages
    pacman -Sl 2>/dev/null | awk '{print $2 "\t" $1 " " $3}'
end

function __fish_haj_groups
    pacman -Sg 2>/dev/null | awk '{print $1 "\tGroup"}' | sort -u
end

function __fish_haj_downgrades
    begin
        ls /var/cache/pacman/pkg/ 2>/dev/null | string match -r '\.pkg\.tar\.(zst|xz|gz)$' | string replace -r -- '-[^-]+-[^-]+-[^-]+\.pkg\.tar\..*$' '\tcached'
        ls ~/.cache/haj/aur/ 2>/dev/null | string replace -r -- '$' '\tAUR cache'
    end | sort -u
end

function __fish_haj_needs_command
    set -l cmd (commandline -opc)
    if [ (count $cmd) -eq 1 ]
        return 0
    end
    return 1
end


# --- GENERAL SETUP ---

# Disable default file completions
complete -c haj -e
complete -c haj -f


# --- GLOBAL OPTIONS ---

complete -c haj -s a -l aur -d 'restrict operations to the aur'
complete -c haj -s r -l repo -d 'restrict operations to arch repositories'
complete -c haj -s y -l noconfirm -d 'bypass all confirmation prompts'
complete -c haj -s n -l needed -d 'do not reinstall up-to-date packages'
complete -c haj -s i -l ignore -d 'ignore a package upgrade (comma-separated)'
complete -c haj -s c -l config -r -d 'specify an alternate pacman config file'
complete -c haj -l root -r -d 'specify an alternate installation root'
complete -c haj -s v -l verbose -d 'enable verbose debug logging'
complete -c haj -s d -l dry-run -d 'preview a command without modifying the system'
complete -c haj -s V -l version -d 'show version info'
complete -c haj -s h -l help -d 'display help message'


# --- SUBCOMMANDS ---

complete -c haj -n '__fish_haj_needs_command' -a 'tui' -d 'launch interactive package manager dashboard'
complete -c haj -n '__fish_haj_needs_command' -a 'update' -d 'synchronize remote repositories'
complete -c haj -n '__fish_haj_needs_command' -a 'jump' -d 'full system upgrade'
complete -c haj -n '__fish_haj_needs_command' -a 'install' -d 'install one or more packages'
complete -c haj -n '__fish_haj_needs_command' -a 'remove' -d 'remove packages & unneeded dependencies'
complete -c haj -n '__fish_haj_needs_command' -a 'search' -d 'search remote repositories'
complete -c haj -n '__fish_haj_needs_command' -a 'show' -d 'show detailed package information'
complete -c haj -n '__fish_haj_needs_command' -a 'group' -d 'browse and install package groups'
complete -c haj -n '__fish_haj_needs_command' -a 'list' -d 'list installed packages'
complete -c haj -n '__fish_haj_needs_command' -a 'stats' -d 'show system health statistics'
complete -c haj -n '__fish_haj_needs_command' -a 'load' -d 'install a local package archive'
complete -c haj -n '__fish_haj_needs_command' -a 'fetch' -d 'download a package without installing'
complete -c haj -n '__fish_haj_needs_command' -a 'downgrade' -d 'downgrade an installed package'
complete -c haj -n '__fish_haj_needs_command' -a 'owns' -d 'find which installed package owns a file'
complete -c haj -n '__fish_haj_needs_command' -a 'files' -d 'list files installed by a package'
complete -c haj -n '__fish_haj_needs_command' -a 'locate' -d 'search repositories for a file'
complete -c haj -n '__fish_haj_needs_command' -a 'history' -d 'show recent package changes'
complete -c haj -n '__fish_haj_needs_command' -a 'orphan' -d 'detect orphaned dependencies'
complete -c haj -n '__fish_haj_needs_command' -a 'clean' -d 'clean the package cache'
complete -c haj -n '__fish_haj_needs_command' -a 'mark' -d 'change a package install reason'
complete -c haj -n '__fish_haj_needs_command' -a 'diff' -d 'interactively manage and merge .pacnew files'
complete -c haj -n '__fish_haj_needs_command' -a 'pkgbuild' -d 'read and print the PKGBUILD file of a package from the AUR'


# --- DYNAMIC COMPLETIONS ---

complete -c haj -n '__fish_seen_subcommand_from install i fetch f pkgbuild pb' -f -a '(__fish_haj_repo_packages)'
complete -c haj -n '__fish_seen_subcommand_from remove rm toss mark m show info files lf' -f -a '(__fish_haj_installed_packages)'
complete -c haj -n '__fish_seen_subcommand_from downgrade sink' -f -a '(__fish_haj_downgrades)'
complete -c haj -n '__fish_seen_subcommand_from group g' -f -a '(__fish_haj_groups)'
complete -c haj -n '__fish_seen_subcommand_from load l' -f -a '(__fish_complete_suffix pkg.tar.zst; __fish_complete_suffix pkg.tar.xz; __fish_complete_suffix pkg.tar.gz; __fish_complete_suffix pkg.tar)'
complete -c haj -n '__fish_seen_subcommand_from owns ow' -F


# --- COMMAND-SPECIFIC OPTIONS ---

complete -c haj -n '__fish_seen_subcommand_from list ls' -s e -l explicit -d 'show only explicitly installed packages'
complete -c haj -n '__fish_seen_subcommand_from list ls' -s p -l deps -d 'show only dependencies'
complete -c haj -n '__fish_seen_subcommand_from list ls' -s f -l foreign -d 'show only foreign/aur packages'
complete -c haj -n '__fish_seen_subcommand_from clean c' -s k -l keep -d 'number of package versions to keep'
complete -c haj -n '__fish_seen_subcommand_from history h' -s l -l limit -d 'number of recent changes to show'
complete -c haj -n '__fish_seen_subcommand_from upgrade jump' -l no-sync -d 'do not sync remote repositories first'
