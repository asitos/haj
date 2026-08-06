use ratatui::widgets::ListState;
use std::collections::HashSet;
use crate::core;

pub enum Transition {
    None,
    Quit,
    SwitchScreen(ActiveScreen),
    OpenHelp,
    CloseHelp,
    OpenCommandPalette,
    CloseCommandPalette,
}

#[derive(Clone)]
pub struct GroupInfo {
    pub name: String,
    pub packages: Vec<(String, bool)>, 
    pub description: String,
    pub repo: String,
    pub is_favorite: bool,
}

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveScreen {
    Dashboard, Browser, Updates, Stats, History, Groups, Orphans, Clean,
}

#[derive(PartialEq)]
pub enum InputMode { Normal, Editing }

#[derive(Clone, PartialEq, Debug)]
pub enum PackageFilter {
    All, Installed, NotInstalled, Updates, Aur, Repositories, Repo(String),
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
            Self::Repo(name) => write!(f, "repo:{}", name),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum SortMode { Alphabetical, Installed, Repository, Relevance }

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

#[derive(Clone, PartialEq, Debug)]
pub enum GroupSortMode { Alphabetical, PackageCount, InstallCompletion }

#[derive(Clone)]
pub struct PackageInfo {
    pub name: String, pub version: String, pub desc: String, pub repo: String,
    pub is_installed: bool, pub is_upgradable: bool, pub size_mb: f64,
}

pub struct SharedState {
    pub search_query: String,
    pub transaction_logs: Vec<String>,
    pub orphan_count: usize,
    pub updates_count: usize,
    pub installed_count: usize,
    pub selected_packages: HashSet<String>,
    
    pub input_mode: InputMode,
    pub filters: Vec<PackageFilter>,
    pub filter_idx: usize,
    pub sort_mode: SortMode,
    pub pending_g: bool,
    pub package_list: Vec<PackageInfo>,
    pub filtered_packages: Vec<PackageInfo>,
    pub list_state: ListState,

    pub history_items: Vec<String>,
    pub history_state: ListState,
    pub group_items: Vec<String>,
    pub cache_size: String,

    pub explicit_count: usize,
    pub kernel: String,
    pub uptime: String,
    pub free_space: String,
    pub last_sync: String,
    pub last_refreshed: String,

    pub groups: Vec<GroupInfo>,
    pub filtered_groups: Vec<GroupInfo>,
    pub group_state: ListState,
    pub group_search_query: String,
    pub group_sort_mode: GroupSortMode,
    pub group_input_mode: InputMode,
}

impl SharedState {
    pub fn new() -> Self {
        let mut state = Self {
            search_query: String::new(), transaction_logs: Vec::new(),
            orphan_count: 0, updates_count: 0, installed_count: 0,
            selected_packages: HashSet::new(),
            input_mode: InputMode::Normal, 
            filters: vec![
                PackageFilter::All, PackageFilter::Installed, PackageFilter::NotInstalled,
                PackageFilter::Updates, PackageFilter::Aur, PackageFilter::Repositories,
            ], 
            filter_idx: 0,
            sort_mode: SortMode::Relevance, pending_g: false, 
            package_list: Vec::new(), filtered_packages: Vec::new(), list_state: ListState::default(),
            history_items: Vec::new(), history_state: ListState::default(),
            group_items: Vec::new(), cache_size: "unknown".into(),
            explicit_count: 0, kernel: "unknown".into(), uptime: "unknown".into(),
            free_space: "unknown".into(), last_sync: "unknown".into(), last_refreshed: "never".into(),
            groups: Vec::new(), filtered_groups: Vec::new(), group_state: ListState::default(),
            group_search_query: String::new(), group_sort_mode: GroupSortMode::Alphabetical,
            group_input_mode: InputMode::Normal,
        };
        
        if let Ok(alpm) = core::alpm_init::init_alpm() {
            for db in alpm.syncdbs() {
                state.filters.push(PackageFilter::Repo(db.name().to_string()));
            }
        }

        state.refresh_state();
        state
    }

    pub fn refresh_state(&mut self) {
        if let Ok(output) = std::process::Command::new("pacman").arg("-Qdtq").output() {
            self.orphan_count = String::from_utf8_lossy(&output.stdout).split_whitespace().count();
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
                        if alpm::vercmp(pkg.version().to_string(), l_pkg.version().to_string()) == std::cmp::Ordering::Greater {
                            is_upgradable = true;
                            updates_count += 1;
                        }
                    }

                    seen_packages.insert(name.clone());
                    package_list.push(PackageInfo {
                        name, version: pkg.version().to_string(), desc: pkg.desc().unwrap_or("none").to_string(),
                        repo: db.name().to_string(), is_installed, is_upgradable, size_mb: pkg.isize() as f64 / 1_048_576.0,
                    });
                }
            }

            for pkg in local_db.pkgs() {
                let name = pkg.name().to_string();
                if !seen_packages.contains(&name) {
                    installed_count += 1;
                    package_list.push(PackageInfo {
                        name, version: pkg.version().to_string(), desc: pkg.desc().unwrap_or("none").to_string(),
                        repo: "local/aur".to_string(), is_installed: true, is_upgradable: false, size_mb: pkg.isize() as f64 / 1_048_576.0,
                    });
                }
            }
        }

        package_list.sort_by(|a, b| a.name.cmp(&b.name));
        self.package_list = package_list;
        self.installed_count = installed_count;
        self.updates_count = updates_count;

        self.history_items.clear();
        if let Ok(output) = std::process::Command::new("sh").arg("-c").arg("grep '\\[ALPM\\]' /var/log/pacman.log | tail -n 100").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().rev() { self.history_items.push(line.to_string()); }
        }

        let mut groups_map: std::collections::BTreeMap<String, Vec<(String, bool)>> = std::collections::BTreeMap::new();
        if let Ok(output) = std::process::Command::new("pacman").arg("-Sg").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let grp = parts[0].to_string();
                    let pkg = parts[1].to_string();
                    let is_installed = self.package_list.iter().any(|p| p.name == pkg && p.is_installed);
                    groups_map.entry(grp).or_default().push((pkg, is_installed));
                }
            }
        }
        self.groups = groups_map.into_iter().map(|(name, pkgs)| GroupInfo {
            name,
            packages: pkgs,
            description: "package group".to_string(),
            repo: "extra".to_string(),
            is_favorite: false,
        }).collect();

        self.group_items = self.groups.iter().map(|g| g.name.clone()).collect();

        self.cache_size = if let Ok(output) = std::process::Command::new("sh").arg("-c").arg("du -sh /var/cache/pacman/pkg | cut -f1").output() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else { "unknown".to_string() };

        if !self.history_items.is_empty() { self.history_state.select(Some(0)); }
        if !self.groups.is_empty() { self.group_state.select(Some(0)); }

        if let Ok(output) = std::process::Command::new("sh").arg("-c").arg("pacman -Qeq | wc -l").output() {
            self.explicit_count = String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0);
        }
        
        if let Ok(output) = std::process::Command::new("uname").arg("-r").output() {
            self.kernel = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        
        if let Ok(output) = std::process::Command::new("uptime").arg("-p").output() {
            self.uptime = String::from_utf8_lossy(&output.stdout).trim().replace("up ", "");
        }
        
        if let Ok(output) = std::process::Command::new("sh").arg("-c").arg("df -h / | awk 'NR==2 {print $4}'").output() {
            self.free_space = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        
        if let Ok(output) = std::process::Command::new("sh").arg("-c").arg("stat -c %Y /var/lib/pacman/sync/core.db 2>/dev/null").output() {
            let ts: i64 = String::from_utf8_lossy(&output.stdout).trim().parse().unwrap_or(0);
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
            let diff = now - ts;
            if diff < 3600 { self.last_sync = format!("{} min ago", diff / 60); }
            else if diff < 86400 { self.last_sync = format!("{} hrs ago", diff / 3600); }
            else { self.last_sync = format!("{} days ago", diff / 86400); }
        }
        
        self.last_refreshed = chrono::Local::now().format("%H:%M:%S").to_string();

        self.update_search();
        self.update_group_filter();
    }

    pub fn update_search(&mut self) {
        let query = self.search_query.to_lowercase();
        let current_filter = &self.filters[self.filter_idx];

        self.filtered_packages = self.package_list.iter().filter(|p| {
            let matches_query = query.is_empty() || p.name.to_lowercase().contains(&query);
            let matches_filter = match current_filter {
                PackageFilter::All => true, PackageFilter::Installed => p.is_installed,
                PackageFilter::NotInstalled => !p.is_installed, PackageFilter::Updates => p.is_upgradable,
                PackageFilter::Aur => p.repo == "local/aur", PackageFilter::Repositories => p.repo != "local/aur",
                PackageFilter::Repo(name) => &p.repo == name,
            };
            matches_query && matches_filter
        }).cloned().collect();

        match self.sort_mode {
            SortMode::Alphabetical => self.filtered_packages.sort_by(|a, b| a.name.cmp(&b.name)),
            SortMode::Repository => self.filtered_packages.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.name.cmp(&b.name))),
            SortMode::Installed => self.filtered_packages.sort_by(|a, b| b.is_installed.cmp(&a.is_installed).then(a.name.cmp(&b.name))),
            SortMode::Relevance => {
                if !query.is_empty() {
                    self.filtered_packages.sort_by(|a, b| {
                        let a_name = a.name.to_lowercase(); let b_name = b.name.to_lowercase();
                        (b_name == query).cmp(&(a_name == query))
                            .then(b_name.starts_with(&query).cmp(&a_name.starts_with(&query)))
                            .then(a_name.len().cmp(&b_name.len())).then(a.name.cmp(&b_name))
                    });
                } else {
                    self.filtered_packages.sort_by(|a, b| a.name.cmp(&b.name));
                }
            }
        }

        if self.filtered_packages.is_empty() { self.list_state.select(None); } 
        else { let idx = self.list_state.selected().unwrap_or(0).min(self.filtered_packages.len() - 1); self.list_state.select(Some(idx)); }
    }

    pub fn update_group_filter(&mut self) {
        let query = self.group_search_query.to_lowercase();
        self.filtered_groups = self.groups.iter().filter(|g| {
            query.is_empty() || g.name.to_lowercase().contains(&query)
        }).cloned().collect();

        self.filtered_groups.sort_by(|a, b| {
            b.is_favorite.cmp(&a.is_favorite).then_with(|| {
                match self.group_sort_mode {
                    GroupSortMode::Alphabetical => a.name.cmp(&b.name),
                    GroupSortMode::PackageCount => b.packages.len().cmp(&a.packages.len()),
                    GroupSortMode::InstallCompletion => {
                        let a_inst = a.packages.iter().filter(|p| p.1).count() as f64 / a.packages.len().max(1) as f64;
                        let b_inst = b.packages.iter().filter(|p| p.1).count() as f64 / b.packages.len().max(1) as f64;
                        b_inst.partial_cmp(&a_inst).unwrap_or(std::cmp::Ordering::Equal)
                    }
                }
            })
        });

        if self.filtered_groups.is_empty() {
            self.group_state.select(None);
        } else {
            let idx = self.group_state.selected().unwrap_or(0).min(self.filtered_groups.len() - 1);
            self.group_state.select(Some(idx));
        }
    }

    pub fn next_item(&mut self) {
        if self.filtered_packages.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i >= self.filtered_packages.len() - 1 { 0 } else { i + 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
        if self.filtered_packages.is_empty() { return; }
        let i = match self.list_state.selected() {
            Some(i) => if i == 0 { self.filtered_packages.len() - 1 } else { i - 1 },
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn go_to_top(&mut self) { if !self.filtered_packages.is_empty() { self.list_state.select(Some(0)); } }
    pub fn go_to_bottom(&mut self) { if !self.filtered_packages.is_empty() { self.list_state.select(Some(self.filtered_packages.len() - 1)); } }
}

pub struct App {
    pub should_quit: bool,
    pub shared: SharedState,
    pub active_screen: ActiveScreen,
    pub show_help: bool,
    pub command_palette_active: bool,
    pub command_buffer: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            shared: SharedState::new(),
            active_screen: ActiveScreen::Dashboard,
            show_help: false,
            command_palette_active: false,
            command_buffer: String::new(),
        }
    }
}
