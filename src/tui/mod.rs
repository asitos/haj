use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, text::Text, widgets::ListState};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io, time::Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt};
use tokio::sync::mpsc;

use crate::core;
use ansi_to_tui::IntoText;

pub const NEWS_PAGE_SIZE: usize = 25;

pub mod browser;

pub mod dashboard;
pub mod groups;
pub mod help;
pub mod history;
pub mod news;
pub mod stats;
pub mod transaction;

#[derive(PartialEq, Clone, Copy)]
pub enum BrowserTab {
    Overview,
    Dependencies,
    Files,
    Queue,
}

#[derive(Clone, Debug)]
pub struct BrowserCache {
    pub info: String,
    pub files: Vec<String>,
    pub dependencies: Vec<String>,
    pub loading_info: bool,
    pub loading_files: bool,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum SelectionMode {
    Explicit,
    AllVisible,
}

#[derive(PartialEq)]
pub enum CurrentScreen {
    Dashboard,
    Browser,
    News,
    Stats,
    History,
    Groups,
}

#[derive(PartialEq, Clone, Copy)]
pub enum DashboardWidget {
    Blahaj,
    News,
}

#[derive(PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(PartialEq)]
pub enum NewsFocus {
    List,
    Article,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PackageFilter {
    All,
    Installed,
    NotInstalled,
    Updates,
    Aur,
    Repositories,
    Repo(String),
    Group(String),
}

#[derive(Clone)]
pub struct GroupInfo {
    pub name: String,
    pub packages: Vec<(String, bool)>,
    pub description: String,
    pub is_favorite: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum GroupSortMode {
    Alphabetical,
    PackageCount,
    InstallCompletion,
}

impl std::fmt::Display for PackageFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Installed => write!(f, "installed"),
            Self::NotInstalled => write!(f, "not installed"),
            Self::Updates => write!(f, "updates"),
            Self::Aur => write!(f, "aur"),
            Self::Repositories => write!(f, "repositories"),
            Self::Repo(name) => write!(f, "repo:{name}"),
            Self::Group(name) => write!(f, "group:{name}"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum SortMode {
    Alphabetical,
    Installed,
    Repository,
    Relevance,
}

impl std::fmt::Display for SortMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Alphabetical => write!(f, "alphabetical"),
            Self::Installed => write!(f, "installed"),
            Self::Repository => write!(f, "repository"),
            Self::Relevance => write!(f, "relevance"),
        }
    }
}

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub repo: String,
    pub is_installed: bool,
    pub is_upgradable: bool,
    pub size_mb: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub pub_date: String,
    pub description: String,
    pub is_critical: bool,
}

pub enum TuiEvent {
    _Tick,
    Key(crossterm::event::KeyEvent),
    PacmanLog(String),
    UpdateAction(String),
    PacmanProgress(u16),
    TransactionComplete,
    TransactionFailed,
    CloseTransaction,
    DashboardArtFrame(Text<'static>),
    NewsFetched(Vec<NewsItem>, String),
    NewsFetchFailed(String),
    NewsBodyFetched(String, String),
    NewsTotalCount(usize),
    BrowserInfoLoaded(String, String, Vec<String>),
    BrowserFilesLoaded(String, Vec<String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TxAction {
    Install,
    Upgrade,
    Remove,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct PkgChange {
    pub name: String,
    pub old_version: Option<String>,
    pub new_version: String,
    pub action: TxAction,
}

#[derive(Clone, Debug)]
pub struct Transaction {
    pub timestamp: String,
    pub is_success: bool,
    pub primary_action: TxAction,
    pub packages: Vec<PkgChange>,
    pub hooks: Vec<String>,
    pub warnings: Vec<String>,
    pub raw_log: Vec<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum HistoryFilter {
    All,
    Installs,
    Upgrades,
    Removals,
    Failures,
}

pub struct App {
    pub should_quit: bool,
    pub screen: CurrentScreen,
    pub input_mode: InputMode,
    pub last_activity: Instant,

    pub orphan_count: usize,
    pub installed_count: usize,
    pub updates_count: usize,

    pub show_help: bool,
    pub active_widget: DashboardWidget,

    pub filters: Vec<PackageFilter>,
    pub filter_idx: usize,
    pub sort_mode: SortMode,
    pub pending_g: bool,
    pub package_list: Vec<PackageInfo>,
    pub filtered_packages: Vec<PackageInfo>,
    pub search_query: String,
    pub list_state: ListState,
    pub selected_packages: HashSet<String>,

    pub show_prompt: bool,
    pub prompt_type: String,
    pub prompt_targets: Vec<String>,
    pub is_installing: bool,
    pub current_action: String,
    pub progress: u16,
    pub transaction_logs: Vec<String>,
    pub dashboard_art: Text<'static>,
    pub abort_tx: Option<mpsc::Sender<()>>,

    pub news_items: Vec<NewsItem>,
    pub filtered_news: Vec<NewsItem>,
    pub read_news: HashSet<String>,
    pub news_list_state: ListState,
    pub news_scroll: u16,
    pub news_search_query: String,
    pub news_focus: NewsFocus,
    pub is_fetching_news: bool,
    pub news_last_updated: String,
    pub news_error: String,
    pub news_page: usize,
    pub news_total_count: usize,

    pub transactions: Vec<Transaction>,
    pub filtered_transactions: Vec<Transaction>,
    pub history_state: ListState,
    pub history_search_query: String,
    pub history_filter: HistoryFilter,
    pub history_expanded: bool,
    pub history_input_mode: InputMode,

    pub group_items: Vec<String>,
    pub group_state: ListState,
    pub cache_size: String,

    pub groups: Vec<GroupInfo>,
    pub filtered_groups: Vec<GroupInfo>,
    pub group_search_query: String,
    pub group_sort_mode: GroupSortMode,
    pub group_input_mode: InputMode,

    pub explicit_count: usize,
    pub kernel: String,
    pub uptime: String,
    pub free_space: String,
    pub last_sync: String,
    pub last_refreshed: String,

    pub browser_tab: BrowserTab,
    pub browser_pkg_name: String,

    pub browser_caches: std::collections::HashMap<String, BrowserCache>,
    pub browser_scroll: u16,

    pub deselected_packages: HashSet<String>,
    pub selection_mode: SelectionMode,
}

impl App {
    pub fn new() -> Self {
        let mut dynamic_filters = vec![
            PackageFilter::All,
            PackageFilter::Installed,
            PackageFilter::NotInstalled,
            PackageFilter::Updates,
            PackageFilter::Aur,
            PackageFilter::Repositories,
        ];

        if let Ok(alpm) = core::alpm_init::init_alpm() {
            for db in alpm.syncdbs() {
                dynamic_filters.push(PackageFilter::Repo(db.name().to_string()));
            }
        }

        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let cache_dir = home.join(".cache/haj");
        let _ = std::fs::create_dir_all(&cache_dir);

        let read_news: HashSet<String> = std::fs::read_to_string(cache_dir.join("read_news.json"))
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_default();

        let (saved_idx, saved_scroll) = std::fs::read_to_string(cache_dir.join("news_state.json"))
            .ok()
            .and_then(|data| serde_json::from_str::<(usize, u16)>(&data).ok())
            .unwrap_or((0, 0));

        let mut list_state = ListState::default();
        list_state.select(Some(saved_idx));

        let mut app = Self {
            should_quit: false,
            screen: CurrentScreen::Dashboard,
            input_mode: InputMode::Normal,
            last_activity: Instant::now(),

            show_help: false,
            active_widget: DashboardWidget::Blahaj,

            filters: dynamic_filters,
            filter_idx: 0,
            sort_mode: SortMode::Relevance,
            pending_g: false,
            orphan_count: 0,
            package_list: Vec::new(),
            filtered_packages: Vec::new(),
            search_query: String::new(),
            list_state: ListState::default(),
            selected_packages: HashSet::new(),
            show_prompt: false,
            prompt_type: String::new(),
            prompt_targets: Vec::new(),
            is_installing: false,
            current_action: String::from("idle"),
            progress: 0,
            transaction_logs: Vec::new(),
            dashboard_art: Text::raw(" loading art... "),
            abort_tx: None,

            news_items: Vec::new(),
            filtered_news: Vec::new(),
            read_news,
            news_list_state: list_state,
            news_scroll: saved_scroll,
            news_search_query: String::new(),
            news_focus: NewsFocus::List,
            is_fetching_news: true,
            news_last_updated: "checking...".to_string(),
            news_error: String::new(),
            news_page: 1,
            news_total_count: 50,

            transactions: Vec::new(),
            filtered_transactions: Vec::new(),
            history_state: ListState::default(),
            history_search_query: String::new(),
            history_filter: HistoryFilter::All,
            history_expanded: false,
            history_input_mode: InputMode::Normal,

            group_items: Vec::new(),
            cache_size: "unknown".into(),
            explicit_count: 0,
            kernel: "unknown".into(),
            uptime: "unknown".into(),
            free_space: "unknown".into(),
            last_sync: "unknown".into(),
            last_refreshed: "never".into(),

            groups: Vec::new(),
            filtered_groups: Vec::new(),
            group_state: ListState::default(),
            group_search_query: String::new(),
            group_sort_mode: GroupSortMode::Alphabetical,
            group_input_mode: InputMode::Normal,

            installed_count: 0,
            updates_count: 0,

            browser_tab: BrowserTab::Overview,
            browser_pkg_name: String::new(),

            browser_caches: std::collections::HashMap::new(),
            browser_scroll: 0,

            deselected_packages: HashSet::new(),
            selection_mode: SelectionMode::Explicit,
        };

        app.refresh_state();
        app
    }

    pub fn get_selected_count(&self) -> usize {
        match self.selection_mode {
            SelectionMode::Explicit => self.selected_packages.len(),
            SelectionMode::AllVisible => self
                .filtered_packages
                .len()
                .saturating_sub(self.deselected_packages.len()),
        }
    }

    pub fn is_package_selected(&self, name: &str) -> bool {
        match self.selection_mode {
            SelectionMode::Explicit => self.selected_packages.contains(name),
            SelectionMode::AllVisible => !self.deselected_packages.contains(name),
        }
    }

    pub fn update_history_filter(&mut self) {
        let query = self.history_search_query.to_lowercase();
        self.filtered_transactions = self
            .transactions
            .iter()
            .filter(|tx| {
                let matches_query = query.is_empty()
                    || tx.timestamp.to_lowercase().contains(&query)
                    || tx
                        .packages
                        .iter()
                        .any(|p| p.name.to_lowercase().contains(&query));

                let matches_filter = match self.history_filter {
                    HistoryFilter::All => true,
                    HistoryFilter::Installs => tx.primary_action == TxAction::Install,
                    HistoryFilter::Upgrades => tx.primary_action == TxAction::Upgrade,
                    HistoryFilter::Removals => tx.primary_action == TxAction::Remove,
                    HistoryFilter::Failures => !tx.is_success,
                };

                matches_query && matches_filter
            })
            .cloned()
            .collect();

        if self.filtered_transactions.is_empty() {
            self.history_state.select(None);
        } else {
            let idx = self
                .history_state
                .selected()
                .unwrap_or(0)
                .min(self.filtered_transactions.len() - 1);
            self.history_state.select(Some(idx));
        }
    }

    pub fn update_group_filter(&mut self) {
        let query = self.group_search_query.to_lowercase();
        self.filtered_groups = self
            .groups
            .iter()
            .filter(|g| query.is_empty() || g.name.to_lowercase().contains(&query))
            .cloned()
            .collect();

        self.filtered_groups.sort_by(|a, b| {
            b.is_favorite
                .cmp(&a.is_favorite)
                .then_with(|| match self.group_sort_mode {
                    GroupSortMode::Alphabetical => a.name.cmp(&b.name),
                    GroupSortMode::PackageCount => b.packages.len().cmp(&a.packages.len()),
                    GroupSortMode::InstallCompletion => {
                        let a_inst = a.packages.iter().filter(|p| p.1).count() as f64
                            / a.packages.len().max(1) as f64;
                        let b_inst = b.packages.iter().filter(|p| p.1).count() as f64
                            / b.packages.len().max(1) as f64;
                        b_inst
                            .partial_cmp(&a_inst)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                })
        });

        if self.filtered_groups.is_empty() {
            self.group_state.select(None);
        } else {
            let idx = self
                .group_state
                .selected()
                .unwrap_or(0)
                .min(self.filtered_groups.len() - 1);
            self.group_state.select(Some(idx));
        }
    }

    pub fn mark_news_read(&mut self, link: String) {
        if self.read_news.insert(link) {
            let home = std::env::var("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_default();
            let data = serde_json::to_string(&self.read_news).unwrap_or_default();
            tokio::spawn(async move {
                let _ = tokio::fs::create_dir_all(home.join(".cache/haj")).await;
                let _ = tokio::fs::write(home.join(".cache/haj/read_news.json"), data).await;
            });
        }
    }

    pub fn update_news_search(&mut self, reset_page: bool) {
        let query = self.news_search_query.to_lowercase();
        self.filtered_news = self
            .news_items
            .iter()
            .filter(|n| {
                query.is_empty()
                    || n.title.to_lowercase().contains(&query)
                    || n.description.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        if reset_page {
            self.news_page = 1;
            self.news_list_state
                .select(if self.filtered_news.is_empty() {
                    None
                } else {
                    Some(0)
                });
            self.news_scroll = 0;
        } else if let Some(idx) = self.news_list_state.selected() {
            let page_size = NEWS_PAGE_SIZE;
            let start_idx = (self.news_page.saturating_sub(1)) * page_size;
            let end_idx = (start_idx + page_size).min(self.filtered_news.len());
            let displayed_count = end_idx.saturating_sub(start_idx);
            if idx >= displayed_count {
                self.news_list_state.select(if displayed_count > 0 {
                    Some(displayed_count - 1)
                } else {
                    None
                });
            }
        } else if !self.filtered_news.is_empty() {
            self.news_list_state.select(Some(0));
        }
    }

    pub fn refresh_state(&mut self) {
        if let Ok(output) = std::process::Command::new("sudo")
            .args(["pacman", "-Qdtq"])
            .output()
        {
            self.orphan_count = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .count();
        } else {
            self.orphan_count = 0;
        }

        let mut package_list = Vec::new();
        let mut installed_count = 0;
        let mut updates_count = 0;

        if let Ok(alpm) = core::alpm_init::init_alpm() {
            let local_db = alpm.localdb();
            let mut seen_packages = HashSet::new();

            for db in alpm.syncdbs() {
                for pkg in db.pkgs() {
                    let name = pkg.name().to_string();
                    let local_pkg = local_db.pkg(name.as_str());
                    let is_installed = local_pkg.is_ok();
                    let mut is_upgradable = false;

                    if let Ok(l_pkg) = local_pkg {
                        installed_count += 1;
                        if alpm::vercmp(pkg.version().to_string(), l_pkg.version().to_string())
                            == std::cmp::Ordering::Greater
                        {
                            is_upgradable = true;
                            updates_count += 1;
                        }
                    }

                    seen_packages.insert(name.clone());
                    package_list.push(PackageInfo {
                        name,
                        version: pkg.version().to_string(),
                        repo: db.name().to_string(),
                        is_installed,
                        is_upgradable,
                        size_mb: pkg.isize() as f64 / 1_048_576.0,
                    });
                }
            }

            for pkg in local_db.pkgs() {
                let name = pkg.name().to_string();
                if !seen_packages.contains(&name) {
                    installed_count += 1;
                    package_list.push(PackageInfo {
                        name,
                        version: pkg.version().to_string(),
                        repo: "local/aur".to_string(),
                        is_installed: true,
                        is_upgradable: false,
                        size_mb: pkg.isize() as f64 / 1_048_576.0,
                    });
                }
            }

            package_list.sort_by(|a, b| a.name.cmp(&b.name));
            self.package_list = package_list;
            self.installed_count = installed_count;
            self.updates_count = updates_count;
        }

        self.transactions.clear();
        if let Ok(output) = std::process::Command::new("sh")
            .arg("-c")
            .arg("sudo tail -n 5000 /var/log/pacman.log")
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_tx: Option<Transaction> = None;

            for line in stdout.lines() {
                let ts = if line.starts_with('[') && line.len() > 18 {
                    line[1..17].to_string()
                } else {
                    "unknown".to_string()
                };

                if line.contains("transaction started") {
                    current_tx = Some(Transaction {
                        timestamp: ts,
                        is_success: false,
                        primary_action: TxAction::Unknown,
                        packages: Vec::new(),
                        hooks: Vec::new(),
                        warnings: Vec::new(),
                        raw_log: vec![line.to_string()],
                    });
                } else if line.contains("transaction completed")
                    || line.contains("transaction failed")
                {
                    if let Some(mut tx) = current_tx.take() {
                        tx.raw_log.push(line.to_string());
                        tx.is_success = line.contains("transaction completed");

                        let installs = tx
                            .packages
                            .iter()
                            .filter(|p| p.action == TxAction::Install)
                            .count();
                        let upgrades = tx
                            .packages
                            .iter()
                            .filter(|p| p.action == TxAction::Upgrade)
                            .count();
                        let removals = tx
                            .packages
                            .iter()
                            .filter(|p| p.action == TxAction::Remove)
                            .count();

                        tx.primary_action = if upgrades > installs && upgrades > removals {
                            TxAction::Upgrade
                        } else if removals > installs && removals > upgrades {
                            TxAction::Remove
                        } else if installs > 0 {
                            TxAction::Install
                        } else {
                            TxAction::Unknown
                        };

                        self.transactions.push(tx);
                    }
                } else if let Some(ref mut tx) = current_tx {
                    tx.raw_log.push(line.to_string());

                    if line.contains("[ALPM] installed") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 5 {
                            tx.packages.push(PkgChange {
                                name: parts[3].to_string(),
                                old_version: None,
                                new_version: parts[4].replace(['(', ')'], ""),
                                action: TxAction::Install,
                            });
                        }
                    } else if line.contains("[ALPM] upgraded") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 7 {
                            tx.packages.push(PkgChange {
                                name: parts[3].to_string(),
                                old_version: Some(parts[4].replace('(', "")),
                                new_version: parts[6].replace(')', ""),
                                action: TxAction::Upgrade,
                            });
                        }
                    } else if line.contains("[ALPM] removed") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 5 {
                            tx.packages.push(PkgChange {
                                name: parts[3].to_string(),
                                old_version: None,
                                new_version: parts[4].replace(['(', ')'], ""),
                                action: TxAction::Remove,
                            });
                        }
                    } else if line.contains("warning:") {
                        tx.warnings.push(
                            line.split("warning:")
                                .last()
                                .unwrap_or("")
                                .trim()
                                .to_string(),
                        );
                    } else if line.contains("Running") && line.contains("hook") {
                        tx.hooks.push(
                            line.split("Running")
                                .last()
                                .unwrap_or("")
                                .replace("hook...", "")
                                .trim()
                                .to_string(),
                        );
                    }
                }
            }
            self.transactions.reverse();
        }
        self.update_history_filter();

        let mut groups_map: std::collections::BTreeMap<String, Vec<(String, bool)>> =
            std::collections::BTreeMap::new();
        if let Ok(output) = std::process::Command::new("sudo")
            .args(["pacman", "-Sg"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let grp = parts[0].to_string();
                    let pkg = parts[1].to_string();
                    let is_installed = self
                        .package_list
                        .iter()
                        .any(|p| p.name == pkg && p.is_installed);
                    groups_map.entry(grp).or_default().push((pkg, is_installed));
                }
            }
        }

        if groups_map.is_empty()
            && let Ok(output) = std::process::Command::new("sudo")
                .args(["pacman", "-Qg"])
                .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let grp = parts[0].to_string();
                    let pkg = parts[1].to_string();
                    let is_installed = self
                        .package_list
                        .iter()
                        .any(|p| p.name == pkg && p.is_installed);
                    groups_map.entry(grp).or_default().push((pkg, is_installed));
                }
            }
        }

        self.groups = groups_map
            .into_iter()
            .map(|(name, pkgs)| GroupInfo {
                name,
                packages: pkgs,
                description: "package group".to_string(),
                is_favorite: false,
            })
            .collect();

        self.group_items = self.groups.iter().map(|g| g.name.clone()).collect();

        self.cache_size = if let Ok(output) = std::process::Command::new("sh")
            .arg("-c")
            .arg("sudo du -sh /var/cache/pacman/pkg | cut -f1")
            .output()
        {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            "unknown".to_string()
        };

        if !self.transactions.is_empty() {
            self.history_state.select(Some(0));
        }
        if !self.groups.is_empty() {
            self.group_state.select(Some(0));
        }

        if let Ok(output) = std::process::Command::new("sh")
            .arg("-c")
            .arg("sudo pacman -Qeq | wc -l")
            .output()
        {
            self.explicit_count = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse()
                .unwrap_or(0);
        }
        if let Ok(output) = std::process::Command::new("uname").arg("-r").output() {
            self.kernel = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        if let Ok(output) = std::process::Command::new("uptime").arg("-p").output() {
            self.uptime = String::from_utf8_lossy(&output.stdout)
                .trim()
                .replace("up ", "");
        }
        if let Ok(output) = std::process::Command::new("sh")
            .arg("-c")
            .arg("df -h / | awk 'NR==2 {print $4}'")
            .output()
        {
            self.free_space = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        if let Ok(output) = std::process::Command::new("sh")
            .arg("-c")
            .arg("sudo stat -c %Y /var/lib/pacman/sync/core.db 2>/dev/null")
            .output()
        {
            let ts: i64 = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse()
                .unwrap_or(0);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let diff = now - ts;
            if diff < 3600 {
                self.last_sync = format!("{} min ago", diff / 60);
            } else if diff < 86400 {
                self.last_sync = format!("{} hrs ago", diff / 3600);
            } else {
                self.last_sync = format!("{} days ago", diff / 86400);
            }
        }
        self.last_refreshed = chrono::Local::now().format("%H:%M:%S").to_string();

        self.update_search();

        for group in &mut self.groups {
            for pkg in &mut group.packages {
                pkg.1 = self
                    .package_list
                    .iter()
                    .any(|p| p.name == pkg.0 && p.is_installed);
            }
        }
        self.update_group_filter();
    }

    pub fn update_search(&mut self) {
        let query = self.search_query.to_lowercase();
        let current_filter = &self.filters[self.filter_idx];

        self.filtered_packages = self
            .package_list
            .iter()
            .filter(|p| {
                let matches_query = query.is_empty() || p.name.to_lowercase().contains(&query);

                let matches_filter = match current_filter {
                    PackageFilter::All => true,
                    PackageFilter::Installed => p.is_installed,
                    PackageFilter::NotInstalled => !p.is_installed,
                    PackageFilter::Updates => p.is_upgradable,
                    PackageFilter::Aur => p.repo == "local/aur",
                    PackageFilter::Repositories => p.repo != "local/aur",
                    PackageFilter::Repo(name) => &p.repo == name,

                    PackageFilter::Group(gname) => {
                        if let Some(g) = self.groups.iter().find(|x| &x.name == gname) {
                            g.packages.iter().any(|pkg| pkg.0 == p.name)
                        } else {
                            false
                        }
                    }
                };
                matches_query && matches_filter
            })
            .cloned()
            .collect();

        match self.sort_mode {
            SortMode::Alphabetical => {
                self.filtered_packages.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortMode::Repository => {
                self.filtered_packages
                    .sort_by(|a, b| a.repo.cmp(&b.repo).then(a.name.cmp(&b.name)));
            }
            SortMode::Installed => {
                self.filtered_packages.sort_by(|a, b| {
                    b.is_installed
                        .cmp(&a.is_installed)
                        .then(a.name.cmp(&b.name))
                });
            }
            SortMode::Relevance => {
                if query.is_empty() {
                    self.filtered_packages.sort_by(|a, b| a.name.cmp(&b.name));
                } else {
                    self.filtered_packages.sort_by(|a, b| {
                        let a_name = a.name.to_lowercase();
                        let b_name = b.name.to_lowercase();
                        let a_exact = a_name == query;
                        let b_exact = b_name == query;
                        let a_starts = a_name.starts_with(&query);
                        let b_starts = b_name.starts_with(&query);

                        b_exact
                            .cmp(&a_exact)
                            .then(b_starts.cmp(&a_starts))
                            .then(a_name.len().cmp(&b_name.len()))
                            .then(a.name.cmp(&b_name))
                    });
                }
            }
        }

        if self.filtered_packages.is_empty() {
            self.list_state.select(None);
        } else {
            let current_idx = self.list_state.selected().unwrap_or(0);
            if current_idx >= self.filtered_packages.len() {
                self.list_state
                    .select(Some(self.filtered_packages.len() - 1));
            } else {
                self.list_state.select(Some(current_idx));
            }
        }
    }

    pub fn next_item(&mut self) {
        if self.filtered_packages.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.filtered_packages.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
        if self.filtered_packages.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_packages.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn go_to_top(&mut self) {
        if !self.filtered_packages.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn go_to_bottom(&mut self) {
        if !self.filtered_packages.is_empty() {
            self.list_state
                .select(Some(self.filtered_packages.len() - 1));
        }
    }
}

pub mod events;
pub mod news_fetch;
use events::run_app;
use news_fetch::{fetch_arch_news, fetch_article_body};
pub async fn run() -> Result<()> {
    let display3d = std::process::Command::new("display3d")
        .arg("--help")
        .output()
        .map_err(|_| {
            anyhow::anyhow!(
                "display3d is required for `haj tui` but was not found in PATH. \\\n+                 Install it with: cargo install display3d"
            )
        })?;

    if !display3d.status.success() {
        return Err(anyhow::anyhow!(
            "display3d is required for `haj tui` but could not be started successfully."
        ));
    }

    println!("🦈 haj requires root privileges for package management.");
    let status = std::process::Command::new("sudo").arg("-v").status()?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "sudo authentication failed or was cancelled."
        ));
    }
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();
    let res = run_app(&mut terminal, &mut app).await;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    if let Err(err) = res {
        println!("{err:?}");
    }
    std::process::exit(0);
}
