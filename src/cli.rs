use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "haj",
    author = "asitos",
    version = "0.2.7",
    about = "fast, quiet, beautiful package manager and tui for blahArch Linux.",
    long_about = None,
    disable_help_subcommand = true,
    disable_version_flag = true,
    help_template = "\
{about}

Usage: \x1b[36;1mhaj\x1b[0m [OPTIONS] <COMMAND>

Commands (alias):
  \x1b[36;1mtui (t)\x1b[0m                         launch the interactive package manager dashboard

  \x1b[36;1mupdate (up/sync)\x1b[0m                synchronize remote repositories 
  \x1b[36;1mjump (upgrade)\x1b[0m [--no-sync]       full system upgrade
  \x1b[36;1minstall (i)\x1b[0m <pkg>               install one or more packages
  \x1b[36;1mremove (rm/toss)\x1b[0m <pkg>          remove packages & unneeded dependencies
  \x1b[36;1msearch (s)\x1b[0m <query>              search remote repositories
  \x1b[36;1mshow (info)\x1b[0m <pkg>               show detailed package information
  \x1b[36;1mgroup (g)\x1b[0m <name>                browse and install package groups
  \x1b[36;1mlist (ls)\x1b[0m [-e, -p, -f]          list installed packages
  \x1b[36;1mstats (st)\x1b[0m                      show system health and package statistics
  \x1b[36;1mload (l)\x1b[0m <path>                 install a local package archive (.pkg.tar.zst)
  \x1b[36;1mfetch (f)\x1b[0m <pkg>                 download a package without installing
  \x1b[36;1mdowngrade (sink)\x1b[0m <pkg>          downgrade an installed package

  \x1b[36;1mowns (ow)\x1b[0m <path>                find which installed package owns a file
  \x1b[36;1mfiles (lf)\x1b[0m <pkg>                list files installed by a package
  \x1b[36;1mlocate (loc)\x1b[0m <query>            search repositories for a file (pacman -F)

  \x1b[36;1mhistory (h)\x1b[0m [-l <n>]            show recent package changes
  \x1b[36;1morphan (o)\x1b[0m                      detect orphaned dependencies
  \x1b[36;1mclean (c)\x1b[0m [-k <n>]              clean the package cache
  \x1b[36;1mmark (m)\x1b[0m <pkg> [--as-explicit]  change a package's install reason
  \x1b[36;1mdiff (pn)\x1b[0m                       interactively manage and merge .pacnew files

Options:
{options}"
)]
#[command(
    name = "haj",
    version,
    about = "fast, quiet, beautiful package manager for blahArch."
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
        alias = "i",
        help_template = "\
\x1b[36;1mhaj install (alias: i)\x1b[0m

Usage: haj install [OPTIONS] <packages>...

Description:
  install one or more packages from the repositories or AUR

Arguments:
  <packages>...  list of packages to install

Options:
  -a, --aur            restrict operations to the aur
  -r, --repo           restrict operations to arch repositories
  -y, --noconfirm      bypass all confirmation prompts
      --root <PATH>    specify an alternate installation root
  -v, --verbose        enable verbose debug logging
  -d, --dry-run        preview a command without modifying the system
  -h, --help           print help"
    )]
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    #[command(
        alias = "rm",
        alias = "toss",
        help_template = "\
\x1b[36;1mhaj remove (aliases: rm, toss)\x1b[0m

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
        alias = "up",
        alias = "sync",
        help_template = "\
\x1b[36;1mhaj update (aliases: up, sync)\x1b[0m

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

Usage: haj upgrade [OPTIONS]

Description:
  perform a full system upgrade of all official repositories and AUR packages

Options:
      --no-sync        do not sync package databases from remote mirrors
  -a, --aur            restrict operations to the aur
  -r, --repo           restrict operations to arch repositories
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
    },

    #[command(
        alias = "s",
        help_template = "\
\x1b[36;1mhaj search (alias: s)\x1b[0m

Usage: haj search [OPTIONS] <query>

Description:
  search official repositories and AUR for matching packages

Arguments:
  <query>  the search term to query

Options:
  -a, --aur      restrict operations to the aur
  -r, --repo     restrict operations to arch repositories
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Search { query: String },

    #[command(
        alias = "info",
        help_template = "\
\x1b[36;1mhaj show (alias: info)\x1b[0m

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
  browse and install packages associated with a specific package group (e.g. gnome, base-devel)

Arguments:
  <name>  the package group name

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Group { name: String },

    #[command(
        alias = "ls",
        help_template = "\
\x1b[36;1mhaj list (alias: ls)\x1b[0m

Usage: haj list [OPTIONS]

Description:
  list installed packages on the system

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
  show recent transaction history (installs, upgrades, removals) with smart relative timestamps

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
        alias = "sink",
        help_template = "\
\x1b[36;1mhaj downgrade (alias: sink)\x1b[0m

Usage: haj downgrade [OPTIONS] <package>

Description:
  downgrade an installed package using the cached packages in /var/cache/pacman/pkg

Arguments:
  <package>  the package name to downgrade

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
    },

    #[command(
        alias = "c",
        help_template = "\
\x1b[36;1mhaj clean (alias: c)\x1b[0m

Usage: haj clean [OPTIONS]

Description:
  clean the pacman package cache to reclaim disk space

Options:
  -k, --keep <KEEP>  number of package versions to keep in cache [default: 3]
  -v, --verbose      enable verbose debug logging
  -h, --help         print help"
    )]
    Clean {
        /// number of package versions to keep in cache
        #[arg(short = 'k', long, default_value_t = 3)]
        keep: usize,
    },

    #[command(
        alias = "o",
        help_template = "\
\x1b[36;1mhaj orphan (alias: o)\x1b[0m

Usage: haj orphan [OPTIONS]

Description:
  detect and list orphaned package dependencies that are no longer needed

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Orphan,

    #[command(
        alias = "ow",
        help_template = "\
\x1b[36;1mhaj owns (alias: ow)\x1b[0m

Usage: haj owns [OPTIONS] <file_path>

Description:
  find which installed package owns a given absolute file path

Arguments:
  <file_path>  the file path to query (e.g. /usr/bin/bash)

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Owns { file_path: String },

    #[command(
        alias = "loc",
        help_template = "\
\x1b[36;1mhaj locate (alias: loc)\x1b[0m

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
  list all files installed by a specific package

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
        alias = "f",
        help_template = "\
\x1b[36;1mhaj fetch (alias: f)\x1b[0m

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

Usage: haj mark [OPTIONS] <package>

Description:
  toggle an installed package's install reason between explicit and dependency

Arguments:
  <package>  the package to modify

Options:
      --root <PATH>    specify an alternate installation root
  -v, --verbose        enable verbose debug logging
  -d, --dry-run        preview a command without modifying the system
  -h, --help           print help"
    )]
    Mark { package: String },

    #[command(
        alias = "pn",
        help_template = "\
\x1b[36;1mhaj diff (alias: pn)\x1b[0m

Usage: haj diff [OPTIONS]

Description:
  interactively search, manage, and merge .pacnew configuration files on the system

Options:
  -v, --verbose  enable verbose debug logging
  -h, --help     print help"
    )]
    Diff,

    #[command(hide = true)]
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    #[command(external_subcommand)]
    Interactive(Vec<String>),
}

