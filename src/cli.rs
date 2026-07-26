use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "haj",
    author = "asitos",
    version = "0.2.2",
    about = "fast, quiet, beautiful package management for blahArch Linux.",
    long_about = None,
    disable_help_subcommand = true,
    disable_help_flag = true,
    disable_version_flag = true,
    help_template = "\
{about}

Usage: haj [OPTIONS] <COMMAND>

Commands (alias):
  \x1b[1mtui (t)\x1b[0m               launch the interactive package manager dashboard

  \x1b[1mupdate (up/sync)\x1b[0m      synchronize repositories & upgrade installed packages
  \x1b[1mjump (upgrade)\x1b[0m        full system upgrade
  \x1b[1minstall (i)\x1b[0m           install one or more packages
  \x1b[1mremove (rm/toss)\x1b[0m      remove packages & unneeded dependencies
  \x1b[1msearch (s)\x1b[0m            search remote repositories
  \x1b[1mshow (info)\x1b[0m           show detailed package information
  \x1b[1mlist (ls)\x1b[0m             list explicitly installed packages

  \x1b[1mload (l)\x1b[0m              install a local package archive (.pkg.tar.zst)
  \x1b[1mfetch (f)\x1b[0m             download a package without installing
  \x1b[1mdowngrade (sink)\x1b[0m      downgrade an installed package

  \x1b[1mowns (ow)\x1b[0m             find which installed package owns a file
  \x1b[1mfiles (lf)\x1b[0m            list files installed by a package
  \x1b[1mlocate (loc)\x1b[0m          search repositories for a file (pacman -F)

  \x1b[1mhistory (h)\x1b[0m           show recent package transactions
  \x1b[1morphan (o)\x1b[0m            detect orphaned dependencies
  \x1b[1mclean (c)\x1b[0m             clean the package cache
  \x1b[1mmark (m)\x1b[0m              change a package's install reason
  \x1b[1mdiff (pn)\x1b[0m             interactively manage and merge .pacnew files

Options:
{options}"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

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

    /// display this help message
    #[arg(short = 'h', long, action = clap::ArgAction::Help)]
    pub help: Option<bool>,

    /// show version info
    #[arg(short = 'V', long, action = clap::ArgAction::Version)]
    pub version: Option<bool>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(alias = "t")]
    Tui,

    #[command(alias = "i")]
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    #[command(alias = "rm", alias = "toss")]
    Remove {
        #[arg(required = true)]
        packages: Vec<String>,
    },

    #[command(alias = "up", alias = "sync")]
    Update,

    #[command(alias = "jump")]
    Upgrade {
        #[arg(short = 'u', long)]
        sysupgrade: bool,
    },

    #[command(alias = "s")]
    Search { query: String },

    #[command(alias = "info")]
    Show { package: String },

    #[command(alias = "ls")]
    List {
        /// show only packages installed explicitly
        #[arg(short, long)]
        explicit: bool,
        /// show only packages installed as dependencies
        #[arg(short = 'p', long)]
        deps: bool,
    },

    #[command(alias = "h")]
    History {
        /// number of recent changes to show
        #[arg(short = 'l', long, default_value_t = 50)]
        limit: usize,
    },

    #[command(alias = "sink")]
    Downgrade {
        /// package to downgrade
        package: String,
    },

    #[command(alias = "c")]
    Clean {
        /// number of package versions to keep in cache
        #[arg(short = 'k', long, default_value_t = 3)]
        keep: usize,
    },

    #[command(alias = "o")]
    Orphan,

    #[command(alias = "ow")]
    Owns { file_path: String },

    #[command(alias = "loc")]
    Locate { query: String },

    #[command(alias = "lf")]
    Files { package: String },

    #[command(alias = "l")]
    Load { archive_path: String },

    #[command(alias = "f")]
    Fetch { packages: Vec<String> },

    #[command(alias = "m")]
    Mark {
        package: String,
        #[arg(long)]
        as_explicit: bool,
    },

    #[command(alias = "pn")]
    Diff,
}
