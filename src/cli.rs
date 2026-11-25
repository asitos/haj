use clap::{Parser, Subcommand, ValueEnum};

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceScope {
    Aur,
    Repo,
    Repos,
}

#[derive(Parser, Debug)]
#[command(
    name = "haj",
    author = "asitos",
    version = "0.3.2",
    about = "the beautiful, cross-distro package manager and tui.",
    long_about = None,
    disable_help_subcommand = true,
    disable_version_flag = true,
    help_template = "\
{about}

Usage: \x1b[36;1mhaj\x1b[0m [OPTIONS] <COMMAND>

Commands (alias):
  \x1b[36;1mtui (t)\x1b[0m                         launch the interactive package manager dashboard

  \x1b[36;1mupdate (up/sync/refresh)\x1b[0m        synchronize remote repositories
  \x1b[36;1mupgrade (jump)\x1b[0m [pkg]            upgrade system or specific packages [--no-sync]
  \x1b[36;1minstall/add (i/a)\x1b[0m <pkg|file>    install packages or local archive [--from aur|repo]
  \x1b[36;1mremove (rm/toss/uninstall/drop)\x1b[0m <pkg>  remove packages & unneeded dependencies
  \x1b[36;1msearch (s/find)\x1b[0m <query>         search repositories & AUR
  \x1b[36;1mshow (info/about/describe)\x1b[0m <pkg>  show detailed package information
  \x1b[36;1mgroup (g)\x1b[0m <name>                browse and install package groups/meta-packages
  \x1b[36;1mlist (ls)\x1b[0m [filter]              list packages (explicit|deps|aur|orphans|updates)
  \x1b[36;1mstats (st)\x1b[0m                      show system health and package statistics
  \x1b[36;1mload (l)\x1b[0m <path>                 install a local package archive (.pkg.tar.zst)
  \x1b[36;1mfetch (f/download/get)\x1b[0m <pkg>    download a package without installing
  \x1b[36;1mdowngrade (sink/rollback)\x1b[0m <pkg> [to <ver>]  downgrade an installed package

  \x1b[36;1mowns (ow/owner/which)\x1b[0m <path>    find which installed package owns a file ('who owns')
  \x1b[36;1mfiles (lf)\x1b[0m [in|of] <pkg>        list files installed by a package ('list files')
  \x1b[36;1mlocate (loc/where/find-file)\x1b[0m <query>  search repositories for a file (pacman -F)

  \x1b[36;1mhistory (h)\x1b[0m [-l <n>]            show recent package changes
  \x1b[36;1morphan (o/orphans/prune)\x1b[0m        list or remove orphaned dependencies
  \x1b[36;1mclean (c)\x1b[0m [cache|orphans]       clean package cache [-k <n>] or orphans
  \x1b[36;1mmark (m)\x1b[0m <pkg> [as explicit|dep]  set or toggle a package's install reason
  \x1b[36;1mdiff (pn/pacnew/merge)\x1b[0m          interactively manage and merge .pacnew files
  \x1b[36;1mpkgbuild (pb)\x1b[0m <pkg>             read PKGBUILD of an AUR package

Options:
{options}"
)]
#[command(
    name = "haj",
    version,
    about = "the beautiful, cross-distro package manager and tui."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// restrict operations to the aur
    #[arg(short = 'a', long, global = true)]
    pub aur: bool,

    /// restrict operations to arch repositories
    #[arg(short = 'r', long, global = true)]
    pub repo: bool,

    /// restrict operations to a specific source (aur or repo)
    #[arg(long, global = true, value_enum, value_name = "SOURCE")]
    pub from: Option<SourceScope>,

    /// bypass all confirmation prompts
    #[arg(short = 'y', long, global = true)]
    pub noconfirm: bool,

    /// do not reinstall up-to-date packages
    #[arg(short = 'n', long, global = true)]
    pub needed: bool,

    /// ignore a package upgrade (comma-separated: pkg1,pkg2)
    #[arg(short = 'i', long, global = true, value_delimiter = ',')]
    pub ignore: Vec<String>,

    /// specify an alternate pacman config file
    #[arg(short = 'c', long, global = true, value_name = "PATH")]
    pub config: Option<String>,

    /// specify an alternate installation root
    #[arg(long, global = true, value_name = "PATH")]
    pub root: Option<String>,

    /// enable verbose debug logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// preview a command without modifying the system
    #[arg(short = 'd', long, global = true)]
    pub dry_run: bool,

    /// show version info
    #[arg(short = 'V', long, action = clap::ArgAction::Version)]
    pub version: Option<bool>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    #[command(
        alias = "t",
        help_template = "\
\x1b[36;1mhaj tui (alias: t)\x1b[0m

Usage: haj tui [OPTIONS]

Description:
  launch the interactive package manager terminal dashboard
  features stats, search, orphans cleaner, news reader, and transaction history

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Tui,

    #[command(
        aliases = ["add", "i", "a"],
        help_template = "\
\x1b[36;1mhaj install/add (aliases: i, a)\x1b[0m

Usage: haj install [OPTIONS] <packages>...

Description:
  install packages from repositories/AUR or a local package archive

Examples:
  haj add neovim
  haj add yay-bin from aur
  haj add ./foo.pkg.tar.zst

Arguments:
  <packages>...  list of packages or local archive paths to install

Options:
  -a, --aur              restrict operations to the aur
  -r, --repo             restrict operations to arch repositories
      --from <SOURCE>    restrict operations to 'aur' or 'repo'
  -n, --needed           do not reinstall up-to-date packages
  -i, --ignore <IGNORE>  ignore a package upgrade (comma-separated)
  -y, --noconfirm        bypass all confirmation prompts
      --root <PATH>      specify an alternate installation root
  -v, --verbose          enable verbose debug logging
  -d, --dry-run          preview a command without modifying the system
  -h, --help             print help"
    )]
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    #[command(
        aliases = ["rm", "toss", "uninstall", "delete", "drop"],
        help_template = "\
\x1b[36;1mhaj remove (aliases: rm, toss, uninstall, delete, drop)\x1b[0m

Usage: haj remove [OPTIONS] <packages>...

Description:
  remove packages along with their unneeded dependencies (via pacman -Rs)

Arguments:
  <packages>...  list of packages to remove

Options:
  -y, --noconfirm      bypass all confirmation prompts
      --root <PATH>    specify an alternate installation root
  -v, --verbose        enable verbose debug logging
  -d, --dry-run        preview a command without modifying the system
  -h, --help           print help"
    )]
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    #[command(
        aliases = ["up", "sync", "refresh"],
        help_template = "\
\x1b[36;1mhaj update (aliases: up, sync, refresh)\x1b[0m

Usage: haj update [OPTIONS]

Description:
  synchronize package databases from remote repositories

Options:
  -a, --aur            restrict operations to the aur
  -r, --repo           restrict operations to arch repositories
      --root <PATH>    specify an alternate installation root
  -v, --verbose        enable verbose debug logging
  -d, --dry-run        preview a command without modifying the system
  -h, --help           print help"
    )]
    Update,

    #[command(
        alias = "jump",
        help_template = "\
\x1b[36;1mhaj upgrade (alias: jump)\x1b[0m

Usage: haj upgrade [OPTIONS] [packages]...

Description:
  perform a full system upgrade or upgrade specific packages

Arguments:
  [packages]...        optional specific packages to upgrade

Options:
      --no-sync        do not sync package databases from remote mirrors
  -a, --aur            restrict operations to the aur
  -r, --repo           restrict operations to arch repositories
      --from <SOURCE>  restrict operations to 'aur' or 'repo'
  -y, --noconfirm      bypass all confirmation prompts
      --root <PATH>    specify an alternate installation root
  -v, --verbose        enable verbose debug logging
  -d, --dry-run        preview a command without modifying the system
  -h, --help           print help"
    )]
    Upgrade {
        /// Do not sync package databases before upgrading
        #[arg(long)]
        no_sync: bool,
        /// Optional specific packages to upgrade
        packages: Vec<String>,
    },

    #[command(
        aliases = ["s", "find"],
        help_template = "\
\x1b[36;1mhaj search (aliases: s, find)\x1b[0m

Usage: haj search [OPTIONS] <query>

Description:
  search official repositories and AUR for matching packages

Arguments:
  <query>  the search term to query

Options:
  -a, --aur            restrict operations to the aur
  -r, --repo           restrict operations to arch repositories
      --from <SOURCE>  restrict operations to 'aur' or 'repo'
  -v, --verbose        enable verbose debug logging
  -h, --help           print help"
    )]
    Search { query: String },

    #[command(
        aliases = ["info", "about", "describe"],
        help_template = "\
\x1b[36;1mhaj show (aliases: info, about, describe)\x1b[0m

Usage: haj show [OPTIONS] <package>

Description:
  display detailed package information such as size, installation reason, and dependencies

Arguments:
  <package>  the package name to inspect

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Show { package: String },

    #[command(
        alias = "g",
        help_template = "\
\x1b[36;1mhaj group (alias: g)\x1b[0m

Usage: haj group [OPTIONS] <name>

Description:
  browse and install packages associated with a package group or meta-package

Arguments:
  <name>  the package group or meta-package name

Options:
  -y, --noconfirm  bypass all confirmation prompts
  -d, --dry-run    preview a command without modifying the system
  -v, --verbose    enable verbose debug logging
  -h, --help       print help"
    )]
    Group { name: String },

    #[command(
        alias = "ls",
        help_template = "\
\x1b[36;1mhaj list (alias: ls)\x1b[0m

Usage: haj list [OPTIONS] [filter]

Description:
  list installed packages or query subsets (explicit, deps, aur, orphans, updates)

Examples:
  haj list explicit
  haj list aur
  haj list updates

Arguments:
  [filter]        optional filter word (explicit|deps|aur|foreign|orphans|updates)

Options:
  -e, --explicit  show only packages installed explicitly
  -p, --deps      show only packages installed as dependencies
  -f, --foreign   show only foreign/AUR packages
  -v, --verbose   enable verbose debug logging
  -h, --help      print help"
    )]
    List {
        /// show only packages installed explicitly
        #[arg(short, long)]
        explicit: bool,
        /// show only packages installed as dependencies
        #[arg(short = 'p', long)]
        deps: bool,
        /// show only foreign/aur packages
        #[arg(short = 'f', long)]
        foreign: bool,
        /// natural filter keyword (explicit|deps|aur|foreign|orphans|updates)
        filter: Option<String>,
    },

    #[command(
        alias = "st",
        help_template = "\
\x1b[36;1mhaj stats (alias: st)\x1b[0m

Usage: haj stats [OPTIONS]

Description:
  display system health score, disk usage, cache statistics, and package counts

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Stats,

    #[command(
        alias = "h",
        help_template = "\
\x1b[36;1mhaj history (alias: h)\x1b[0m

Usage: haj history [OPTIONS]

Description:
  show recent transaction history (installs, upgrades, removals)

Options:
  -l, --limit <LIMIT>  number of recent changes to show [default: 50]
  -v, --verbose        enable verbose debug logging
  -h, --help           print help"
    )]
    History {
        /// number of recent changes to show
        #[arg(short = 'l', long, default_value_t = 50)]
        limit: usize,
    },

    #[command(
        aliases = ["sink", "rollback"],
        help_template = "\
\x1b[36;1mhaj downgrade (aliases: sink, rollback)\x1b[0m

Usage: haj downgrade [OPTIONS] <package> [to] [version]

Description:
  downgrade an installed package using cached archives

Examples:
  haj downgrade bash
  haj downgrade bash to 5.2.026-1

Arguments:
  <package>  the package name to downgrade
  [version]  optional target version

Options:
  -y, --noconfirm      bypass all confirmation prompts
      --root <PATH>    specify an alternate installation root
  -v, --verbose        enable verbose debug logging
  -d, --dry-run        preview a command without modifying the system
  -h, --help           print help"
    )]
    Downgrade {
        /// package to downgrade
        package: String,
        /// optional target version
        version: Option<String>,
    },

    #[command(
        alias = "c",
        help_template = "\
\x1b[36;1mhaj clean (alias: c)\x1b[0m

Usage: haj clean [OPTIONS] [target]

Description:
  clean the package cache (default) or orphaned dependencies

Examples:
  haj clean
  haj clean cache -k 2
  haj clean orphans

Arguments:
  [target]           optional target: 'cache' (default) or 'orphans'

Options:
  -k, --keep <KEEP>  number of package versions to keep in cache [default: 3]
  -y, --noconfirm    bypass all confirmation prompts
  -d, --dry-run      preview a command without modifying the system
  -v, --verbose      enable verbose debug logging
  -h, --help         print help"
    )]
    Clean {
        /// clean target ('cache' or 'orphans')
        target: Option<String>,
        /// number of package versions to keep in cache
        #[arg(short = 'k', long, default_value_t = 3)]
        keep: usize,
    },

    #[command(
        aliases = ["o", "orphans", "autoremove", "prune"],
        help_template = "\
\x1b[36;1mhaj orphan (aliases: o, orphans, autoremove, prune)\x1b[0m

Usage: haj orphan [OPTIONS]

Description:
  detect and optionally remove orphaned package dependencies

Options:
  -y, --noconfirm  bypass all confirmation prompts
  -d, --dry-run    preview a command without modifying the system
  -v, --verbose    enable verbose debug logging
  -h, --help       print help"
    )]
    Orphan,

    #[command(
        aliases = ["ow", "owner", "which"],
        help_template = "\
\x1b[36;1mhaj owns (aliases: ow, owner, which)\x1b[0m

Usage: haj owns [OPTIONS] <file_path>

Description:
  find which installed package owns a given file path ('haj who owns <path>')

Arguments:
  <file_path>  the file path to query (e.g. /usr/bin/bash)

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Owns { file_path: String },

    #[command(
        aliases = ["loc", "where", "find-file"],
        help_template = "\
\x1b[36;1mhaj locate (aliases: loc, where, find-file)\x1b[0m

Usage: haj locate [OPTIONS] <query>

Description:
  search remote package repositories for a file name matching the query (using pacman -F)

Arguments:
  <query>  the file name or pattern to search for

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Locate { query: String },

    #[command(
        alias = "lf",
        help_template = "\
\x1b[36;1mhaj files (alias: lf)\x1b[0m

Usage: haj files [OPTIONS] <package>

Description:
  list all files installed by a specific package ('haj list files <pkg>')

Arguments:
  <package>  the name of the package

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Files { package: String },

    #[command(
        alias = "l",
        help_template = "\
\x1b[36;1mhaj load (alias: l)\x1b[0m

Usage: haj load [OPTIONS] <archive_path>

Description:
  install a local package archive (.pkg.tar.zst) onto the system

Arguments:
  <archive_path>  the path to the local package archive file

Options:
  -y, --noconfirm      bypass all confirmation prompts
      --root <PATH>    specify an alternate installation root
  -v, --verbose        enable verbose debug logging
  -d, --dry-run        preview a command without modifying the system
  -h, --help           print help"
    )]
    Load { archive_path: String },

    #[command(
        aliases = ["f", "download", "get"],
        help_template = "\
\x1b[36;1mhaj fetch (aliases: f, download, get)\x1b[0m

Usage: haj fetch [OPTIONS] <packages>...

Description:
  download package archives to the cache directory without installing them

Arguments:
  <packages>...  list of packages to fetch

Options:
      --root <PATH>    specify an alternate installation root
  -v, --verbose        enable verbose debug logging
  -d, --dry-run        preview a command without modifying the system
  -h, --help           print help"
    )]
    Fetch { packages: Vec<String> },

    #[command(
        alias = "m",
        help_template = "\
\x1b[36;1mhaj mark (alias: m)\x1b[0m

Usage: haj mark [OPTIONS] <package> [reason]

Description:
  set or toggle an installed package's install reason

Examples:
  haj mark bash
  haj mark bash as explicit
  haj mark bash dep

Arguments:
  <package>      the package to modify
  [reason]       optional reason: 'explicit'/'manual' or 'dep'/'deps'/'auto'

Options:
      --as-explicit  mark package as explicitly installed
      --as-deps      mark package as installed as a dependency
      --root <PATH>  specify an alternate installation root
  -v, --verbose      enable verbose debug logging
  -d, --dry-run      preview a command without modifying the system
  -h, --help         print help"
    )]
    Mark {
        package: String,
        /// optional target reason ('explicit'|'manual' or 'dep'|'deps'|'auto')
        reason: Option<String>,
        /// mark package as explicitly installed
        #[arg(long)]
        as_explicit: bool,
        /// mark package as dependency
        #[arg(long)]
        as_deps: bool,
    },

    #[command(
        aliases = ["pn", "pacnew", "merge"],
        help_template = "\
\x1b[36;1mhaj diff (aliases: pn, pacnew, merge)\x1b[0m

Usage: haj diff [OPTIONS]

Description:
  interactively search, manage, and merge .pacnew configuration files on the system

Options:
  -d, --dry-run  preview command without running pacdiff
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Diff,

    #[command(
        alias = "pb",
        help_template = "\
\x1b[36;1mhaj pkgbuild (alias: pb)\x1b[0m

Usage: haj pkgbuild [OPTIONS] <package>

Description:
  read and print the PKGBUILD file of a package from the AUR

Arguments:
  <package>  the package to fetch the PKGBUILD for

Options:
  -d, --dry-run  preview command without fetching
  -h, --help     print help"
    )]
    Pkgbuild { package: String },

    #[command(hide = true)]
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    #[command(external_subcommand)]
    Interactive(Vec<String>),
}

const KNOWN_VERBS: &[&str] = &[
    "tui",
    "t",
    "install",
    "add",
    "i",
    "a",
    "remove",
    "rm",
    "toss",
    "uninstall",
    "delete",
    "drop",
    "update",
    "up",
    "sync",
    "refresh",
    "upgrade",
    "jump",
    "search",
    "s",
    "find",
    "show",
    "info",
    "about",
    "describe",
    "group",
    "g",
    "list",
    "ls",
    "stats",
    "st",
    "history",
    "h",
    "downgrade",
    "sink",
    "rollback",
    "clean",
    "c",
    "orphan",
    "o",
    "orphans",
    "autoremove",
    "prune",
    "owns",
    "ow",
    "owner",
    "which",
    "locate",
    "loc",
    "where",
    "find-file",
    "files",
    "lf",
    "load",
    "l",
    "fetch",
    "f",
    "download",
    "get",
    "mark",
    "m",
    "diff",
    "pn",
    "pacnew",
    "merge",
    "pkgbuild",
    "pb",
    "completions",
];

fn takes_flag_value(arg: &str) -> bool {
    matches!(
        arg,
        "-i" | "--ignore"
            | "-c"
            | "--config"
            | "--root"
            | "--from"
            | "-l"
            | "--limit"
            | "-k"
            | "--keep"
    )
}

/// Rewrites natural English command phrases into canonical CLI tokens before Clap parsing,
/// while leaving bare `haj <interactive query>` inputs untouched.
pub fn normalize_args(raw: Vec<String>) -> Vec<String> {
    if raw.len() <= 1 {
        return raw;
    }

    let mut prefix_flags = Vec::new();
    let mut rest = Vec::new();
    let mut i = 1;
    while i < raw.len() {
        let arg = &raw[i];
        if arg.starts_with('-') {
            prefix_flags.push(arg.clone());
            if takes_flag_value(arg) && i + 1 < raw.len() {
                i += 1;
                prefix_flags.push(raw[i].clone());
            }
            i += 1;
        } else {
            rest.extend_from_slice(&raw[i..]);
            break;
        }
    }

    if rest.is_empty() {
        return raw;
    }

    // Multi-word natural verb phrases -> canonical single verb.
    if rest.len() >= 2 && rest[0] == "who" && rest[1] == "owns" {
        rest.drain(0..2);
        rest.insert(0, "owns".to_string());
    } else if rest.len() >= 2 && rest[0] == "list" && rest[1] == "files" {
        rest.drain(0..2);
        rest.insert(0, "files".to_string());
    }

    let verb = rest[0].as_str();
    if !KNOWN_VERBS.contains(&verb) {
        return raw;
    }

    let mut tail = Vec::new();
    let mut pos_idx = 0usize;
    let mut j = 1;
    while j < rest.len() {
        let cur = &rest[j];

        if cur.starts_with('-') {
            tail.push(cur.clone());
            if takes_flag_value(cur) && j + 1 < rest.len() {
                j += 1;
                tail.push(rest[j].clone());
            }
            j += 1;
            continue;
        }

        // "from aur" / "in aur" / "from repo(s)" / "in repo(s)"
        if (cur == "from" || cur == "in")
            && j + 1 < rest.len()
            && matches!(
                verb,
                "install"
                    | "add"
                    | "i"
                    | "a"
                    | "search"
                    | "s"
                    | "find"
                    | "upgrade"
                    | "jump"
                    | "update"
                    | "up"
                    | "sync"
                    | "refresh"
                    | "show"
                    | "info"
                    | "about"
                    | "describe"
                    | "pkgbuild"
                    | "pb"
            )
        {
            match rest[j + 1].as_str() {
                "aur" => {
                    tail.push("--aur".to_string());
                    j += 2;
                    continue;
                }
                "repo" | "repos" => {
                    tail.push("--repo".to_string());
                    j += 2;
                    continue;
                }
                _ => {}
            }
        }

        // "downgrade <pkg> to <ver>"
        if matches!(verb, "downgrade" | "sink" | "rollback")
            && cur == "to"
            && pos_idx == 1
            && j + 1 < rest.len()
        {
            j += 1;
            continue;
        }

        // "mark <pkg> as explicit|dep"
        if matches!(verb, "mark" | "m") && cur == "as" && pos_idx == 1 && j + 1 < rest.len() {
            j += 1;
            continue;
        }

        // "files in <pkg>" / "files of <pkg>"
        if matches!(verb, "files" | "lf")
            && (cur == "in" || cur == "of")
            && pos_idx == 0
            && j + 1 < rest.len()
        {
            j += 1;
            continue;
        }

        tail.push(cur.clone());
        pos_idx += 1;
        j += 1;
    }

    let mut out = Vec::with_capacity(1 + prefix_flags.len() + 1 + tail.len());
    out.push(raw[0].clone());
    out.extend(prefix_flags);
    out.push(rest[0].clone());
    out.extend(tail);
    out
}

pub fn parse_cli() -> Result<Cli, clap::Error> {
    let raw: Vec<String> = std::env::args_os()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    Cli::try_parse_from(normalize_args(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(args: &[&str]) -> Vec<String> {
        normalize_args(args.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn test_normalize_natural_phrases() {
        assert_eq!(
            norm(&["haj", "add", "yay", "from", "aur"]),
            vec!["haj", "add", "yay", "--aur"]
        );
        assert_eq!(
            norm(&["haj", "search", "foo", "in", "repo"]),
            vec!["haj", "search", "foo", "--repo"]
        );
        assert_eq!(
            norm(&["haj", "downgrade", "bash", "to", "5.2"]),
            vec!["haj", "downgrade", "bash", "5.2"]
        );
        assert_eq!(
            norm(&["haj", "rollback", "--dry-run", "7zip", "to", "26.05"]),
            vec!["haj", "rollback", "--dry-run", "7zip", "26.05"]
        );
        assert_eq!(
            norm(&["haj", "mark", "bash", "as", "explicit"]),
            vec!["haj", "mark", "bash", "explicit"]
        );
        assert_eq!(
            norm(&["haj", "mark", "-d", "bash", "as", "dep"]),
            vec!["haj", "mark", "-d", "bash", "dep"]
        );
        assert_eq!(
            norm(&["haj", "who", "owns", "/usr/bin/bash"]),
            vec!["haj", "owns", "/usr/bin/bash"]
        );
        assert_eq!(
            norm(&["haj", "list", "files", "of", "bash"]),
            vec!["haj", "files", "bash"]
        );
        assert_eq!(
            norm(&["haj", "files", "-v", "in", "bash"]),
            vec!["haj", "files", "-v", "bash"]
        );
    }

    #[test]
    fn test_bare_interactive_query_untouched() {
        assert_eq!(
            norm(&["haj", "text", "to", "speech"]),
            vec!["haj", "text", "to", "speech"]
        );
    }
}
