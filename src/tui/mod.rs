use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    text::Text,
    widgets::ListState,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    io,
    time::{Duration, Instant},
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt};
use tokio::sync::mpsc;

use crate::core;
use ansi_to_tui::IntoText;

pub mod browser;
pub mod dashboard;
pub mod news;
pub mod reddit;
pub mod transaction;

#[derive(PartialEq)]
pub enum CurrentScreen {
    Dashboard,
    Browser,
    News,
    Reddit,
}

#[derive(PartialEq, Clone, Copy)]
pub enum DashboardWidget {
    Blahaj,
    News,
    Reddit,
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
    pub desc: String,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedditItem {
    pub title: String,
    pub author: String,
    pub score: i64,
    pub num_comments: i64,
    pub selftext: String,
    pub url: String,
    pub thumbnail: String,
    pub created_utc: f64,
    pub link_flair_text: Option<String>,
    pub nsfw: bool,
    pub pinned: bool,
    pub post_hint: Option<String>,
    pub permalink: String,
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
    RedditFetched(Vec<RedditItem>, String),
    RedditFetchFailed(String),
}

pub struct App {
    pub should_quit: bool,
    pub screen: CurrentScreen,
    pub input_mode: InputMode,
    pub last_activity: Instant,

    pub active_widget: DashboardWidget,

    pub filters: Vec<PackageFilter>,
    pub filter_idx: usize,
    pub sort_mode: SortMode,
    pub pending_g: bool,
    pub orphan_count: usize,
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

    pub reddit_items: Vec<RedditItem>,
    pub filtered_reddit: Vec<RedditItem>,
    pub reddit_list_state: ListState,
    pub reddit_scroll: u16,
    pub reddit_search_query: String,
    pub reddit_focus: NewsFocus,
    pub is_fetching_reddit: bool,
    pub reddit_last_updated: String,
    pub reddit_error: String,
    pub show_reddit_image: bool,
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

        let home = dirs::home_dir().unwrap_or_default();
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

            active_widget: DashboardWidget::Blahaj,

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

            reddit_items: Vec::new(),
            filtered_reddit: Vec::new(),
            reddit_list_state: ListState::default(),
            reddit_scroll: 0,
            reddit_search_query: String::new(),
            reddit_focus: NewsFocus::List,
            is_fetching_reddit: true,
            reddit_last_updated: "checking...".to_string(),
            reddit_error: String::new(),
            show_reddit_image: true,
        };

        app.refresh_state();
        app
    }

    pub fn mark_news_read(&mut self, link: String) {
        if self.read_news.insert(link) {
            let home = dirs::home_dir().unwrap_or_default();
            let data = serde_json::to_string(&self.read_news).unwrap_or_default();
            // FIX: File writing moved to an async task to prevent UI lockups
            tokio::spawn(async move {
                let _ = tokio::fs::create_dir_all(home.join(".cache/haj")).await;
                let _ = tokio::fs::write(home.join(".cache/haj/read_news.json"), data).await;
            });
        }
    }

    pub fn update_news_search(&mut self) {
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
        self.news_list_state
            .select(if self.filtered_news.is_empty() {
                None
            } else {
                Some(0)
            });
        self.news_scroll = 0;
    }

    pub fn update_reddit_search(&mut self) {
        let query = self.reddit_search_query.to_lowercase();
        self.filtered_reddit = self
            .reddit_items
            .iter()
            .filter(|r| {
                query.is_empty()
                    || r.title.to_lowercase().contains(&query)
                    || r.selftext.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        self.reddit_list_state
            .select(if self.filtered_reddit.is_empty() {
                None
            } else {
                Some(0)
            });
        self.reddit_scroll = 0;
    }

    pub fn refresh_state(&mut self) {
        if let Ok(output) = std::process::Command::new("pacman").arg("-Qdtq").output() {
            self.orphan_count = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .count();
        } else {
            self.orphan_count = 0;
        }

        if let Ok(alpm) = core::alpm_init::init_alpm() {
            let local_db = alpm.localdb();
            let mut seen_packages = HashSet::new();
            let mut new_list = Vec::new();

            for db in alpm.syncdbs() {
                for pkg in db.pkgs() {
                    let name = pkg.name().to_string();
                    let local_pkg = local_db.pkg(name.as_str());
                    let is_installed = local_pkg.is_ok();
                    let mut is_upgradable = false;

                    if let Ok(l_pkg) = local_pkg {
                        if alpm::vercmp(pkg.version().to_string(), l_pkg.version().to_string())
                            == std::cmp::Ordering::Greater
                        {
                            is_upgradable = true;
                        }
                    }

                    seen_packages.insert(name.clone());
                    new_list.push(PackageInfo {
                        name,
                        version: pkg.version().to_string(),
                        desc: pkg.desc().unwrap_or("none").to_string(),
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
                    new_list.push(PackageInfo {
                        name,
                        version: pkg.version().to_string(),
                        desc: pkg.desc().unwrap_or("none").to_string(),
                        repo: "local/aur".to_string(),
                        is_installed: true,
                        is_upgradable: false,
                        size_mb: pkg.isize() as f64 / 1_048_576.0,
                    });
                }
            }

            new_list.sort_by(|a, b| a.name.cmp(&b.name));
            self.package_list = new_list;
        }

        self.update_search();
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

fn parse_arch_xml(xml: &str) -> Vec<NewsItem> {
    let mut items = Vec::new();
    let critical_words = [
        "manual intervention",
        "requires intervention",
        "breaking change",
        "filesystem",
        "pacman",
        "keyring",
        "glibc",
    ];

    let mut search_idx = 0;
    while let Some(item_start) = xml[search_idx..].find("<item>") {
        let absolute_start = search_idx + item_start;
        if let Some(item_end) = xml[absolute_start..].find("</item>") {
            let item_str = &xml[absolute_start..absolute_start + item_end];

            let extract = |tag: &str, end_tag: &str| -> String {
                if let (Some(s), Some(e)) = (item_str.find(tag), item_str.find(end_tag)) {
                    item_str[s + tag.len()..e].to_string()
                } else {
                    String::new()
                }
            };

            let decode_html = |s: &str| {
                s.replace("&gt;", ">")
                    .replace("&lt;", "<")
                    .replace("&quot;", "\"")
                    .replace("&amp;", "&")
                    .replace("&#39;", "'")
            };
            let title = decode_html(&extract("<title>", "</title>"));
            let link = extract("<link>", "</link>");
            let pub_date = extract("<pubDate>", "</pubDate>");
            let mut desc = decode_html(&extract("<description>", "</description>"));

            // FIX: Stripping HTML *during the fetch* prevents the 60FPS render loop from catching on fire.
            desc = desc
                .replace("<![CDATA[", "")
                .replace("]]>", "")
                .replace("<p>", "")
                .replace("</p>", "\n\n")
                .replace("<li>", "• ")
                .replace("</li>", "\n")
                .replace("<ul>", "")
                .replace("</ul>", "\n")
                .replace("<br>", "\n")
                .replace("<br/>", "\n");

            while let Some(start) = desc.find('<') {
                if let Some(end) = desc[start..].find('>') {
                    let tag = &desc[start..=start + end];
                    if tag == "<code>" || tag == "</code>" {
                        desc.replace_range(
                            start..=start + end,
                            if tag == "<code>" {
                                "[[CODE_START]]"
                            } else {
                                "[[CODE_END]]"
                            },
                        );
                    } else {
                        desc.replace_range(start..=start + end, "");
                    }
                } else {
                    break;
                }
            }
            desc = desc
                .replace("[[CODE_START]]", "<code>")
                .replace("[[CODE_END]]", "</code>");

            let is_crit = critical_words
                .iter()
                .any(|&w| title.to_lowercase().contains(w) || desc.to_lowercase().contains(w));
            if !title.is_empty() {
                items.push(NewsItem {
                    title,
                    link,
                    pub_date,
                    description: desc,
                    is_critical: is_crit,
                });
            }
            search_idx = absolute_start + item_end;
        } else {
            break;
        }
    }
    items
}

fn parse_reddit_json(json: &serde_json::Value) -> Vec<RedditItem> {
    let mut items = Vec::new();
    if let Some(children) = json.pointer("/data/children").and_then(|c| c.as_array()) {
        for child in children {
            if let Some(data) = child.get("data") {
                let item = RedditItem {
                    title: data
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    author: data
                        .get("author")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    score: data.get("score").and_then(|v| v.as_i64()).unwrap_or(0),
                    num_comments: data
                        .get("num_comments")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0),
                    selftext: data
                        .get("selftext")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: data
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    thumbnail: data
                        .get("thumbnail")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    created_utc: data
                        .get("created_utc")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                    link_flair_text: data
                        .get("link_flair_text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    nsfw: data
                        .get("over_18")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    pinned: data
                        .get("stickied")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    post_hint: data
                        .get("post_hint")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    permalink: data
                        .get("permalink")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                };
                if !item.pinned {
                    items.push(item);
                }
            }
        }
    }
    items
}

pub fn fetch_arch_news(tx: mpsc::Sender<TuiEvent>) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .user_agent("haj/0.2.4 (https://github.com/asitos/haj)")
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();

        let home = dirs::home_dir().unwrap_or_default();
        let cache_path = home.join(".cache/haj/news.json");

        match client.get("https://archlinux.org/feeds/news/").send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(xml) = resp.text().await {
                    let items = parse_arch_xml(&xml);
                    if !items.is_empty() {
                        if let Ok(cache_data) = serde_json::to_string(&items) {
                            let _ = tokio::fs::write(&cache_path, cache_data).await;
                        }
                        let _ = tx
                            .send(TuiEvent::NewsFetched(items, "just now".into()))
                            .await;
                        return;
                    }
                }
                let _ = tx
                    .send(TuiEvent::NewsFetchFailed(
                        "Failed to parse Arch news".into(),
                    ))
                    .await;
            }
            Ok(resp) => {
                let _ = tx
                    .send(TuiEvent::NewsFetchFailed(format!("HTTP {}", resp.status())))
                    .await;
            }
            Err(e) => {
                if let Ok(data) = std::fs::read_to_string(&cache_path) {
                    if let Ok(items) = serde_json::from_str::<Vec<NewsItem>>(&data) {
                        let _ = tx.send(TuiEvent::NewsFetched(items, "cached".into())).await;
                        return;
                    }
                }
                let _ = tx.send(TuiEvent::NewsFetchFailed(e.to_string())).await;
            }
        }
    });
}

pub fn fetch_reddit(tx: mpsc::Sender<TuiEvent>) {
    tokio::spawn(async move {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let client_res = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/115.0") // Standard browser UA to bypass 403
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .build();

        let client = match client_res {
            Ok(c) => c,
            Err(e) => {
                let _ = tx
                    .send(TuiEvent::RedditFetchFailed(format!("Client err: {}", e)))
                    .await;
                return;
            }
        };

        let home = dirs::home_dir().unwrap_or_default();
        let cache_path = home.join(".cache/haj/reddit.json");

        match client
            .get("https://old.reddit.com/r/blahaj/hot.json?limit=25")
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    let items = parse_reddit_json(&json);
                    if !items.is_empty() {
                        if let Ok(cache_data) = serde_json::to_string(&items) {
                            let _ = tokio::fs::write(&cache_path, cache_data).await;
                        }
                        let _ = tx
                            .send(TuiEvent::RedditFetched(items, "just now".into()))
                            .await;
                        return;
                    }
                }
                let _ = tx
                    .send(TuiEvent::RedditFetchFailed(
                        "Failed to parse Reddit response".into(),
                    ))
                    .await;
            }
            Ok(resp) => {
                let status = resp.status();
                let msg = if status.as_u16() == 403 {
                    "HTTP 403 Forbidden".to_string()
                } else if status.as_u16() == 429 {
                    "HTTP 429 Too Many Requests".to_string()
                } else {
                    format!("HTTP {}", status)
                };

                if let Ok(data) = std::fs::read_to_string(&cache_path) {
                    if let Ok(items) = serde_json::from_str::<Vec<RedditItem>>(&data) {
                        let _ = tx.send(TuiEvent::RedditFetchFailed(msg)).await;
                        let _ = tx
                            .send(TuiEvent::RedditFetched(items, "cached".into()))
                            .await;
                        return;
                    }
                }
                let _ = tx.send(TuiEvent::RedditFetchFailed(msg)).await;
            }
            Err(e) => {
                if let Ok(data) = std::fs::read_to_string(&cache_path) {
                    if let Ok(items) = serde_json::from_str::<Vec<RedditItem>>(&data) {
                        let _ = tx
                            .send(TuiEvent::RedditFetchFailed("Network offline".into()))
                            .await;
                        let _ = tx
                            .send(TuiEvent::RedditFetched(items, "cached".into()))
                            .await;
                        return;
                    }
                }
                let _ = tx.send(TuiEvent::RedditFetchFailed(e.to_string())).await;
            }
        }
    });
}

pub async fn run() -> Result<()> {
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
        println!("{:?}", err);
    }
    std::process::exit(0);
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    <B as Backend>::Error: Send + Sync + 'static,
{
    let config = crate::config::load_config();
    let (tx, mut rx) = mpsc::channel::<TuiEvent>(100);

    fetch_arch_news(tx.clone());
    fetch_reddit(tx.clone());

    let tx_input = tx.clone();
    tokio::spawn(async move {
        loop {
            if tx_input.is_closed() {
                break;
            }
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    // FIX: try_send prevents the entire UI thread from permanently deadlocking if channels fill up
                    let _ = tx_input.try_send(TuiEvent::Key(key));
                }
            } else {
                let _ = tx_input.try_send(TuiEvent::_Tick);
            }
        }
    });

    let tx_art = tx.clone();
    tokio::spawn(async move {
        if config.general.animations {
            let temp_dir = std::env::temp_dir();
            let obj_path = temp_dir.join("blahaj.obj");
            let mtl_path = temp_dir.join("blahaj.mtl");

            if !obj_path.exists() {
                let _ = std::fs::write(&obj_path, include_bytes!("../../resources/blahaj.obj"));
            }
            if !mtl_path.exists() {
                let _ = std::fs::write(&mtl_path, include_bytes!("../../resources/blahaj.mtl"));
            }

            let child = tokio::process::Command::new("display3d")
                .args([&obj_path.to_string_lossy(), "-t", "0,0.5,7.5"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn();

            if let Ok(mut child_proc) = child {
                let mut stdout = child_proc.stdout.take().unwrap();
                let mut buf = vec![0; 8192];
                let mut frame_buffer = Vec::new();
                let mut first_frame = true;

                loop {
                    let read_fut = stdout.read(&mut buf);
                    let read_res = if first_frame {
                        tokio::time::timeout(Duration::from_secs(2), read_fut)
                            .await
                            .unwrap_or(Ok(0))
                    } else {
                        read_fut.await
                    };

                    match read_res {
                        Ok(0) => break, // Fallback triggered below
                        Ok(n) => {
                            first_frame = false;
                            frame_buffer.extend_from_slice(&buf[..n]);
                            while let Some(pos) =
                                frame_buffer.windows(3).position(|w| w == b"\x1b[H")
                            {
                                let frame = frame_buffer[..pos].to_vec();
                                frame_buffer.drain(..=pos + 2);
                                if let Ok(text) = frame.into_text() {
                                    let _ = tx_art.send(TuiEvent::DashboardArtFrame(text)).await;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        let fallback_str = include_str!("../../resources/ascii.txt");
        let text = fallback_str
            .as_bytes()
            .into_text()
            .unwrap_or_else(|_| Text::raw(fallback_str));
        let _ = tx_art.send(TuiEvent::DashboardArtFrame(text)).await;
    });

    let spawn_pacman = |tx_channel: mpsc::Sender<TuiEvent>,
                        mut args: Vec<String>,
                        _action_name: String,
                        mut abort_rx: mpsc::Receiver<()>| {
        tokio::spawn(async move {
            if !args.contains(&"--color=never".to_string()) {
                args.push("--color=never".into());
            }

            let child_res = tokio::process::Command::new("sudo")
                .args(args)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn();

            if let Ok(mut child) = child_res {
                let pid = child.id();
                let stdout = child.stdout.take().unwrap();
                let stderr = child.stderr.take().unwrap();

                let abort_task = tokio::spawn(async move {
                    if let Some(()) = abort_rx.recv().await {
                        if let Some(p) = pid {
                            let _ = tokio::process::Command::new("sudo")
                                .args(["kill", "-INT", &p.to_string()])
                                .status()
                                .await;
                        }
                    }
                });

                let tx_out = tx_channel.clone();
                let out_task = tokio::spawn(async move {
                    let mut reader = tokio::io::BufReader::new(stdout).lines();
                    let mut in_hook_phase = false;

                    while let Ok(Some(line)) = reader.next_line().await {
                        let clean = line.trim();
                        if clean.is_empty() {
                            continue;
                        }
                        let lower_clean = clean.to_lowercase();

                        if lower_clean.contains("resolving dependencies")
                            || lower_clean.contains("conflicting packages")
                        {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "resolving package dependencies...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("checking keys")
                            || lower_clean.contains("checking package integrity")
                        {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "verifying package integrity...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("loading package files") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction("loading package files...".into()))
                                .await;
                        } else if lower_clean.contains("checking for file conflicts") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "checking for file conflicts...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("checking available disk space") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "checking available disk space...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("retrieving packages") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction("downloading packages...".into()))
                                .await;
                        } else if lower_clean.contains("processing package changes") {
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "processing package changes...".into(),
                                ))
                                .await;
                        } else if clean.starts_with('(')
                            && (lower_clean.contains(") upgrading")
                                || lower_clean.contains(") installing")
                                || lower_clean.contains(") removing"))
                        {
                            if let Some(idx_end) = clean.find(')') {
                                let counter = &clean[..=idx_end];
                                let action = if lower_clean.contains("installing") {
                                    "installing"
                                } else if lower_clean.contains("removing") {
                                    "removing"
                                } else {
                                    "upgrading"
                                };
                                let _ = tx_out
                                    .send(TuiEvent::UpdateAction(format!(
                                        "{} packages {}...",
                                        counter, action
                                    )))
                                    .await;
                            }
                        } else if lower_clean.contains("running pre-transaction hooks") {
                            in_hook_phase = true;
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "running pre-transaction hooks...".into(),
                                ))
                                .await;
                        } else if lower_clean.contains("running post-transaction hooks") {
                            in_hook_phase = true;
                            let _ = tx_out
                                .send(TuiEvent::UpdateAction(
                                    "running post-transaction hooks...".into(),
                                ))
                                .await;
                        } else if in_hook_phase
                            && (clean.starts_with("==> Building image")
                                || clean.starts_with("==> Install DKMS")
                                || clean.starts_with("==> Generating"))
                        {
                            let _ = tx_out.send(TuiEvent::UpdateAction(clean.to_string())).await;
                        }

                        if lower_clean.contains("error:")
                            || lower_clean.contains("warning:")
                            || lower_clean.contains("failed")
                        {
                            let _ = tx_out.send(TuiEvent::PacmanLog(clean.to_string())).await;
                        } else if in_hook_phase
                            && (lower_clean.contains("missing")
                                || lower_clean.contains("not found"))
                        {
                            let _ = tx_out.send(TuiEvent::PacmanLog(clean.to_string())).await;
                        }

                        if clean.contains('%') {
                            let parts: Vec<&str> = clean.split('%').collect();
                            if !parts.is_empty() {
                                let num_str = parts[0].split_whitespace().last().unwrap_or("0");
                                if let Ok(val) = num_str.parse::<u16>() {
                                    let _ = tx_out.send(TuiEvent::PacmanProgress(val)).await;
                                }
                            }
                        }
                    }
                });

                let tx_err = tx_channel.clone();
                let err_task = tokio::spawn(async move {
                    let mut reader = tokio::io::BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = reader.next_line().await {
                        let clean = line.trim();
                        if clean.is_empty() {
                            continue;
                        }
                        let _ = tx_err
                            .send(TuiEvent::PacmanLog(format!("error: {}", clean)))
                            .await;
                    }
                });

                let _ = tokio::join!(out_task, err_task);

                let status = child
                    .wait()
                    .await
                    .unwrap_or_else(|_| std::os::unix::process::ExitStatusExt::from_raw(1));

                abort_task.abort();

                let _ = tx_channel.send(TuiEvent::PacmanProgress(100)).await;

                if status.success() {
                    let _ = tx_channel.send(TuiEvent::TransactionComplete).await;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                } else {
                    let _ = tx_channel.send(TuiEvent::TransactionFailed).await;
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            } else {
                let _ = tx_channel.send(TuiEvent::TransactionFailed).await;
                let _ = tx_channel
                    .send(TuiEvent::PacmanLog(
                        "fatal: failed to spawn sudo process".into(),
                    ))
                    .await;
                tokio::time::sleep(Duration::from_secs(4)).await;
            }

            let _ = tx_channel.send(TuiEvent::CloseTransaction).await;
        });
    };

    loop {
        terminal.draw(|f| {
            match app.screen {
                CurrentScreen::Dashboard => dashboard::render(f, app),
                CurrentScreen::Browser => browser::render(f, app),
                CurrentScreen::News => news::render(f, app),
                CurrentScreen::Reddit => reddit::render(f, app),
            }
            transaction::render_popup(f, app);
            transaction::render_confirm_popup(f, app);
        })?;

        if let Some(event) = rx.recv().await {
            match event {
                TuiEvent::_Tick => {
                    if app.screen == CurrentScreen::Dashboard
                        && app.active_widget != DashboardWidget::Blahaj
                    {
                        if app.last_activity.elapsed() > Duration::from_secs(10) {
                            let unread_news = app
                                .news_items
                                .iter()
                                .filter(|n| !app.read_news.contains(&n.link))
                                .count();
                            let updates =
                                app.package_list.iter().filter(|p| p.is_upgradable).count();
                            if unread_news == 0 && updates == 0 {
                                app.active_widget = DashboardWidget::Blahaj;
                            }
                        }
                    }
                }

                TuiEvent::DashboardArtFrame(text) => app.dashboard_art = text,
                TuiEvent::PacmanLog(log) => {
                    app.transaction_logs.push(log);
                    if app.transaction_logs.len() > 30 {
                        app.transaction_logs.remove(0);
                    }
                }
                TuiEvent::UpdateAction(action) => app.current_action = action,
                TuiEvent::PacmanProgress(val) => app.progress = val.min(100),
                TuiEvent::TransactionComplete => {
                    app.current_action = "changes complete ✓".to_string()
                }
                TuiEvent::TransactionFailed => {
                    app.current_action = "transaction failed ❌".to_string()
                }
                TuiEvent::CloseTransaction => {
                    app.is_installing = false;
                    app.progress = 0;
                    app.refresh_state();
                }

                TuiEvent::NewsFetched(items, time) => {
                    app.is_fetching_news = false;
                    app.news_error.clear();
                    app.news_items = items;
                    app.news_last_updated = time;
                    app.update_news_search();
                }
                TuiEvent::NewsFetchFailed(err) => {
                    app.is_fetching_news = false;
                    app.news_error = err;
                }

                TuiEvent::RedditFetched(items, time) => {
                    app.is_fetching_reddit = false;
                    app.reddit_error.clear();
                    app.reddit_items = items;
                    app.reddit_last_updated = time;
                    app.update_reddit_search();
                }
                TuiEvent::RedditFetchFailed(err) => {
                    app.is_fetching_reddit = false;
                    app.reddit_error = err;
                }

                TuiEvent::Key(key) => {
                    app.last_activity = Instant::now();
                    if app.is_installing {
                        if let KeyCode::Char('q') = key.code {
                            if let Some(tx_abort) = app.abort_tx.take() {
                                let _ = tx_abort.try_send(());
                                app.current_action = "aborting safely...".to_string();
                                app.transaction_logs.push("".into());
                                app.transaction_logs.push(
                                    "==> user triggered abort. sending SIGINT to pacman...".into(),
                                );
                            }
                        }
                        continue;
                    }

                    if app.show_prompt {
                        match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                                app.show_prompt = false;
                                app.is_installing = true;
                                app.transaction_logs.clear();
                                app.progress = 0;

                                app.current_action = if app.prompt_targets.len() > 1 {
                                    format!(
                                        "{}ing {} packages...",
                                        app.prompt_type,
                                        app.prompt_targets.len()
                                    )
                                } else {
                                    format!("{}ing {}...", app.prompt_type, app.prompt_targets[0])
                                };

                                let mut args = vec!["pacman".into()];
                                if app.prompt_type == "install" {
                                    args.extend(vec!["-S".into(), "--noconfirm".into()]);
                                } else {
                                    args.extend(vec!["-Rs".into(), "--noconfirm".into()]);
                                }
                                args.extend(app.prompt_targets.clone());

                                let (abort_tx, abort_rx) = mpsc::channel(1);
                                app.abort_tx = Some(abort_tx);
                                spawn_pacman(tx.clone(), args, app.prompt_type.clone(), abort_rx);

                                app.selected_packages.clear();
                                app.prompt_targets.clear();
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.show_prompt = false;
                                app.prompt_targets.clear();
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match app.screen {
                        CurrentScreen::Dashboard => match key.code {
                            KeyCode::Char('q') => app.should_quit = true,
                            KeyCode::Char('/') | KeyCode::Char('f') => {
                                app.screen = CurrentScreen::Browser;
                                app.input_mode = InputMode::Editing;
                            }

                            KeyCode::Char('n') => {
                                if app.active_widget == DashboardWidget::News {
                                    app.screen = CurrentScreen::News;
                                } else {
                                    app.active_widget = DashboardWidget::News;
                                }
                            }
                            KeyCode::Char('r') => {
                                if app.active_widget == DashboardWidget::Reddit {
                                    app.screen = CurrentScreen::Reddit;
                                } else {
                                    app.active_widget = DashboardWidget::Reddit;
                                }
                            }
                            KeyCode::Char('b') => {
                                app.active_widget = DashboardWidget::Blahaj;
                            }
                            KeyCode::Enter => match app.active_widget {
                                DashboardWidget::News => app.screen = CurrentScreen::News,
                                DashboardWidget::Reddit => app.screen = CurrentScreen::Reddit,
                                DashboardWidget::Blahaj => app.screen = CurrentScreen::Browser,
                            },
                            KeyCode::Tab => {
                                app.active_widget = match app.active_widget {
                                    DashboardWidget::Blahaj => DashboardWidget::News,
                                    DashboardWidget::News => DashboardWidget::Reddit,
                                    DashboardWidget::Reddit => DashboardWidget::Blahaj,
                                };
                            }
                            KeyCode::BackTab => {
                                app.active_widget = match app.active_widget {
                                    DashboardWidget::Blahaj => DashboardWidget::Reddit,
                                    DashboardWidget::News => DashboardWidget::Blahaj,
                                    DashboardWidget::Reddit => DashboardWidget::News,
                                };
                            }
                            _ => {}
                        },

                        CurrentScreen::News => match app.input_mode {
                            InputMode::Normal => match key.code {
                                KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Char('/') | KeyCode::Char('f') | KeyCode::Char('s') => {
                                    app.input_mode = InputMode::Editing
                                }
                                KeyCode::Char('r') => {
                                    app.is_fetching_news = true;
                                    fetch_arch_news(tx.clone());
                                }
                                KeyCode::Tab | KeyCode::Enter => {
                                    app.news_focus = if app.news_focus == NewsFocus::List {
                                        NewsFocus::Article
                                    } else {
                                        NewsFocus::List
                                    };
                                }
                                KeyCode::Char('o') => {
                                    if let Some(idx) = app.reddit_list_state.selected() {
                                        // OR news_list_state
                                        if let Some(item) = app.filtered_reddit.get(idx) {
                                            app.current_action =
                                                "opening in browser...".to_string();
                                            let url = item.url.clone(); // (or item.link)
                                            // FIX: Background process spawn so the UI thread doesn't freeze
                                            tokio::spawn(async move {
                                                let _ = tokio::process::Command::new("xdg-open")
                                                    .arg(&url)
                                                    .output()
                                                    .await;
                                            });
                                        }
                                    }
                                }
                                KeyCode::Char('y') => {
                                    if let Some(idx) = app.news_list_state.selected() {
                                        if let Some(item) = app.filtered_news.get(idx) {
                                            if std::process::Command::new("wl-copy")
                                                .arg(&item.link)
                                                .output()
                                                .is_err()
                                            {
                                                let mut child = std::process::Command::new("xclip")
                                                    .args(["-selection", "clipboard"])
                                                    .stdin(std::process::Stdio::piped())
                                                    .spawn()
                                                    .unwrap();
                                                if let Some(mut stdin) = child.stdin.take() {
                                                    use std::io::Write;
                                                    let _ = stdin.write_all(item.link.as_bytes());
                                                }
                                            }
                                        }
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if app.news_focus == NewsFocus::List {
                                        let i = match app.news_list_state.selected() {
                                            Some(i) => {
                                                if i >= app.filtered_news.len().saturating_sub(1) {
                                                    0
                                                } else {
                                                    i + 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.news_list_state.select(Some(i));
                                        app.news_scroll = 0;
                                        if let Some(item) = app.filtered_news.get(i) {
                                            let link = item.link.clone();
                                            app.mark_news_read(link);
                                        }
                                    } else {
                                        app.news_scroll = app.news_scroll.saturating_add(1);
                                    }
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if app.news_focus == NewsFocus::List {
                                        let i = match app.news_list_state.selected() {
                                            Some(i) => {
                                                if i == 0 {
                                                    app.filtered_news.len().saturating_sub(1)
                                                } else {
                                                    i - 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.news_list_state.select(Some(i));
                                        app.news_scroll = 0;
                                        if let Some(item) = app.filtered_news.get(i) {
                                            let link = item.link.clone();
                                            app.mark_news_read(link);
                                        }
                                    } else {
                                        app.news_scroll = app.news_scroll.saturating_sub(1);
                                    }
                                }
                                KeyCode::PageDown => {
                                    if app.news_focus == NewsFocus::Article {
                                        app.news_scroll = app.news_scroll.saturating_add(15);
                                    }
                                }
                                KeyCode::PageUp => {
                                    if app.news_focus == NewsFocus::Article {
                                        app.news_scroll = app.news_scroll.saturating_sub(15);
                                    }
                                }
                                KeyCode::Home => {
                                    if app.news_focus == NewsFocus::Article {
                                        app.news_scroll = 0;
                                    }
                                }
                                KeyCode::End => {
                                    if app.news_focus == NewsFocus::Article {
                                        app.news_scroll = 999;
                                    }
                                }
                                _ => {}
                            },
                            InputMode::Editing => match key.code {
                                KeyCode::Esc | KeyCode::Enter => app.input_mode = InputMode::Normal,
                                KeyCode::Char('l')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.news_search_query.clear();
                                    app.update_news_search();
                                }
                                KeyCode::Backspace | KeyCode::Delete => {
                                    app.news_search_query.pop();
                                    app.update_news_search();
                                }
                                KeyCode::Char(c) => {
                                    app.news_search_query.push(c);
                                    app.update_news_search();
                                }
                                _ => {}
                            },
                        },

                        CurrentScreen::Browser => match app.input_mode {
                            InputMode::Normal => match key.code {
                                KeyCode::Char('x') => {
                                    app.search_query.pop();
                                    app.update_search();
                                }
                                KeyCode::Tab => {
                                    app.filter_idx = (app.filter_idx + 1) % app.filters.len();
                                    app.update_search();
                                }
                                KeyCode::BackTab => {
                                    app.filter_idx = if app.filter_idx == 0 {
                                        app.filters.len() - 1
                                    } else {
                                        app.filter_idx - 1
                                    };
                                    app.update_search();
                                }
                                KeyCode::Char('S') => {
                                    app.sort_mode = match app.sort_mode {
                                        SortMode::Alphabetical => SortMode::Installed,
                                        SortMode::Installed => SortMode::Repository,
                                        SortMode::Repository => SortMode::Relevance,
                                        SortMode::Relevance => SortMode::Alphabetical,
                                    };
                                    app.update_search();
                                }
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Esc => app.screen = CurrentScreen::Dashboard,

                                KeyCode::Char('/') | KeyCode::Char('s') | KeyCode::Char('f') => {
                                    app.input_mode = InputMode::Editing
                                }

                                KeyCode::Down | KeyCode::Char('j') => {
                                    app.next_item();
                                    app.pending_g = false;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    app.previous_item();
                                    app.pending_g = false;
                                }
                                KeyCode::Char('g') => {
                                    if app.pending_g {
                                        app.go_to_top();
                                        app.pending_g = false;
                                    } else {
                                        app.pending_g = true;
                                    }
                                }
                                KeyCode::Char('G') => {
                                    app.go_to_bottom();
                                    app.pending_g = false;
                                }

                                KeyCode::Char(' ') => {
                                    if let Some(idx) = app.list_state.selected() {
                                        if let Some(pkg) = app.filtered_packages.get(idx) {
                                            let name = pkg.name.clone();
                                            if app.selected_packages.contains(&name) {
                                                app.selected_packages.remove(&name);
                                            } else {
                                                app.selected_packages.insert(name);
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('c') => {
                                    app.selected_packages.clear();
                                }

                                KeyCode::Char('i') => {
                                    let targets: Vec<String> = if !app.selected_packages.is_empty()
                                    {
                                        app.selected_packages.iter().cloned().collect()
                                    } else if let Some(idx) = app.list_state.selected()
                                        && let Some(pkg) = app.filtered_packages.get(idx)
                                    {
                                        vec![pkg.name.clone()]
                                    } else {
                                        vec![]
                                    };

                                    if !targets.is_empty() {
                                        app.prompt_targets = targets;
                                        app.prompt_type = "install".to_string();
                                        app.show_prompt = true;
                                    }
                                }

                                KeyCode::Char('r') => {
                                    let targets: Vec<String> = if !app.selected_packages.is_empty()
                                    {
                                        app.selected_packages.iter().cloned().collect()
                                    } else if let Some(idx) = app.list_state.selected()
                                        && let Some(pkg) = app.filtered_packages.get(idx)
                                    {
                                        vec![pkg.name.clone()]
                                    } else {
                                        vec![]
                                    };

                                    if !targets.is_empty() {
                                        app.prompt_targets = targets;
                                        app.prompt_type = "remove".to_string();
                                        app.show_prompt = true;
                                    }
                                }
                                _ => {
                                    app.pending_g = false;
                                }
                            },

                            InputMode::Editing => match key.code {
                                KeyCode::Esc | KeyCode::Enter => app.input_mode = InputMode::Normal,
                                KeyCode::Backspace | KeyCode::Delete => {
                                    app.search_query.pop();
                                    app.update_search();
                                }
                                KeyCode::Char(c) => {
                                    app.search_query.push(c);
                                    app.update_search();
                                }
                                _ => {}
                            },
                        },

                        CurrentScreen::Reddit => match app.input_mode {
                            InputMode::Normal => match key.code {
                                KeyCode::Esc => app.screen = CurrentScreen::Dashboard,
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Char('/') | KeyCode::Char('f') | KeyCode::Char('s') => {
                                    app.input_mode = InputMode::Editing
                                }
                                KeyCode::Char('r') => {
                                    app.is_fetching_reddit = true;
                                    app.reddit_error.clear();
                                    fetch_reddit(tx.clone());
                                }
                                KeyCode::Tab | KeyCode::Enter => {
                                    app.reddit_focus = if app.reddit_focus == NewsFocus::List {
                                        NewsFocus::Article
                                    } else {
                                        NewsFocus::List
                                    };
                                }
                                KeyCode::Char('o') => {
                                    if let Some(idx) = app.reddit_list_state.selected() {
                                        // OR news_list_state
                                        if let Some(item) = app.filtered_reddit.get(idx) {
                                            app.current_action =
                                                "opening in browser...".to_string();
                                            let url = item.url.clone(); // (or item.link)
                                            // FIX: Background process spawn so the UI thread doesn't freeze
                                            tokio::spawn(async move {
                                                let _ = tokio::process::Command::new("xdg-open")
                                                    .arg(&url)
                                                    .output()
                                                    .await;
                                            });
                                        }
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if app.reddit_focus == NewsFocus::List {
                                        let i = match app.reddit_list_state.selected() {
                                            Some(i) => {
                                                if i >= app.filtered_reddit.len().saturating_sub(1)
                                                {
                                                    0
                                                } else {
                                                    i + 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.reddit_list_state.select(Some(i));
                                        app.reddit_scroll = 0;
                                    } else {
                                        app.reddit_scroll = app.reddit_scroll.saturating_add(1);
                                    }
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if app.reddit_focus == NewsFocus::List {
                                        let i = match app.reddit_list_state.selected() {
                                            Some(i) => {
                                                if i == 0 {
                                                    app.filtered_reddit.len().saturating_sub(1)
                                                } else {
                                                    i - 1
                                                }
                                            }
                                            None => 0,
                                        };
                                        app.reddit_list_state.select(Some(i));
                                        app.reddit_scroll = 0;
                                    } else {
                                        app.reddit_scroll = app.reddit_scroll.saturating_sub(1);
                                    }
                                }
                                KeyCode::PageDown => {
                                    if app.reddit_focus == NewsFocus::Article {
                                        app.reddit_scroll = app.reddit_scroll.saturating_add(15);
                                    }
                                }
                                KeyCode::PageUp => {
                                    if app.reddit_focus == NewsFocus::Article {
                                        app.reddit_scroll = app.reddit_scroll.saturating_sub(15);
                                    }
                                }
                                KeyCode::Home => {
                                    if app.reddit_focus == NewsFocus::Article {
                                        app.reddit_scroll = 0;
                                    }
                                }
                                KeyCode::End => {
                                    if app.reddit_focus == NewsFocus::Article {
                                        app.reddit_scroll = 999;
                                    }
                                }
                                _ => {}
                            },
                            InputMode::Editing => match key.code {
                                KeyCode::Esc | KeyCode::Enter => app.input_mode = InputMode::Normal,
                                KeyCode::Char('l')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    app.reddit_search_query.clear();
                                    app.update_reddit_search();
                                }
                                KeyCode::Backspace | KeyCode::Delete => {
                                    app.reddit_search_query.pop();
                                    app.update_reddit_search();
                                }
                                KeyCode::Char(c) => {
                                    app.reddit_search_query.push(c);
                                    app.update_reddit_search();
                                }
                                _ => {}
                            },
                        },
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
