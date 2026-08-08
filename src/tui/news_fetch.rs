use super::{NewsItem, TuiEvent};
use std::time::Duration;
use tokio::sync::mpsc;

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

pub fn fetch_article_body(tx: mpsc::Sender<TuiEvent>, link: String) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .user_agent("haj/0.2.5 (https://github.com/asitos/haj)")
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        if let Ok(resp) = client.get(&link).send().await
            && resp.status().is_success()
            && let Ok(html) = resp.text().await
        {
            let desc = {
                if let Some(start) = html.find("class=\"article-content\">") {
                    let content_start = start + "class=\"article-content\">".len();
                    if let Some(end) = html[content_start..].find("</div>") {
                        let mut desc = html[content_start..content_start + end].to_string();
                        desc = desc
                            .replace("<p>", "")
                            .replace("</p>", "\n\n")
                            .replace("<li>", "• ")
                            .replace("</li>", "\n")
                            .replace("<ul>", "")
                            .replace("</ul>", "\n")
                            .replace("<br>", "\n")
                            .replace("<br/>", "\n")
                            .replace("<br />", "\n");

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

                        desc = desc
                            .replace("&gt;", ">")
                            .replace("&lt;", "<")
                            .replace("&quot;", "\"")
                            .replace("&amp;", "&")
                            .replace("&#39;", "'");
                        Some(desc)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(d) = desc {
                let _ = tx.send(TuiEvent::NewsBodyFetched(link, d)).await;
            }
        }
    });
}

pub fn fetch_arch_news(tx: mpsc::Sender<TuiEvent>, page: usize) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .user_agent("haj/0.2.5 (https://github.com/asitos/haj)")
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let cache_path = home.join(".cache/haj/news.json");

        let mut items = Vec::new();
        if page == 1
            && let Ok(resp) = client.get("https://archlinux.org/feeds/news/").send().await
            && resp.status().is_success()
            && let Ok(xml) = resp.text().await
        {
            items = parse_arch_xml(&xml);
        }

        let url = format!("https://archlinux.org/news/?page={page}");
        match client.get(&url).send().await {
            Ok(resp_idx) if resp_idx.status().is_success() => {
                if let Ok(html_idx) = resp_idx.text().await {
                    let mut parsed_total = None;
                    if let Some(idx) = html_idx.find(" news items") {
                        let slice = &html_idx[..idx];
                        let num_str: String = slice
                            .chars()
                            .rev()
                            .take_while(char::is_ascii_digit)
                            .collect();
                        let num_str: String = num_str.chars().rev().collect();
                        if let Ok(total) = num_str.parse::<usize>() {
                            parsed_total = Some(total);
                        }
                    }

                    let new_items = {
                        let mut parsed_items = Vec::new();
                        let mut current_html = html_idx.as_str();
                        while let Some(tr_start) = current_html.find("<tr>") {
                            current_html = &current_html[tr_start + 4..];
                            if let Some(tr_end) = current_html.find("</tr>") {
                                let tr_content = &current_html[..tr_end];
                                current_html = &current_html[tr_end + 5..];

                                let mut tds = Vec::new();
                                let mut tr_search = tr_content;
                                while let Some(td_start) = tr_search.find("<td>") {
                                    tr_search = &tr_search[td_start + 4..];
                                    if let Some(td_end) = tr_search.find("</td>") {
                                        tds.push(&tr_search[..td_end]);
                                        tr_search = &tr_search[td_end + 5..];
                                    }
                                }

                                if tds.len() >= 2 {
                                    let date_str = tds[0].trim().to_string();
                                    let td1 = tds[1];
                                    if let Some(href_start) = td1.find("href=\"") {
                                        let href_rest = &td1[href_start + 6..];
                                        if let Some(href_end) = href_rest.find('"') {
                                            let path = &href_rest[..href_end];
                                            let link = format!("https://archlinux.org{path}");

                                            if let Some(title_start) = href_rest.find('>') {
                                                let title_rest = &href_rest[title_start + 1..];
                                                if let Some(title_end) = title_rest.find("</a>") {
                                                    let title =
                                                        title_rest[..title_end].trim().to_string();

                                                    if !items.iter().any(|item| item.link == link) {
                                                        let pub_date = if let Ok(dt) =
                                                            chrono::NaiveDate::parse_from_str(
                                                                &date_str, "%Y-%m-%d",
                                                            ) {
                                                            dt.and_hms_opt(0, 0, 0).map_or_else(|| date_str.clone(), |dt_time| {
                                                                    dt_time
                                                                        .format("%a, %d %b %Y 00:00:00 +0000")
                                                                        .to_string()
                                                                })
                                                        } else {
                                                            date_str.clone()
                                                        };

                                                        let critical_words = [
                                                            "manual intervention",
                                                            "requires intervention",
                                                            "breaking change",
                                                            "filesystem",
                                                            "pacman",
                                                            "keyring",
                                                            "glibc",
                                                        ];

                                                        let is_crit =
                                                            critical_words.iter().any(|&w| {
                                                                title.to_lowercase().contains(w)
                                                            });

                                                        parsed_items.push(NewsItem {
                                                            title,
                                                            link,
                                                            pub_date,
                                                            description:
                                                                "loading article content..."
                                                                    .to_string(),
                                                            is_critical: is_crit,
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        parsed_items
                    };

                    for fetched_item in new_items {
                        if !items.iter().any(|item| item.link == fetched_item.link) {
                            items.push(fetched_item);
                        }
                    }

                    let mut cached_items = Vec::new();
                    if let Ok(data) = std::fs::read_to_string(&cache_path)
                        && let Ok(parsed) = serde_json::from_str::<Vec<NewsItem>>(&data)
                    {
                        cached_items = parsed;
                    }

                    for fetched_item in items {
                        if let Some(pos) = cached_items
                            .iter()
                            .position(|x| x.link == fetched_item.link)
                        {
                            let mut cached_item = cached_items[pos].clone();
                            if fetched_item.description != "loading article content..." {
                                cached_item.description = fetched_item.description;
                            }
                            cached_items[pos] = cached_item;
                        } else {
                            cached_items.push(fetched_item);
                        }
                    }

                    cached_items.sort_by(|a, b| {
                        let da = chrono::DateTime::parse_from_rfc2822(&a.pub_date);
                        let db = chrono::DateTime::parse_from_rfc2822(&b.pub_date);
                        match (da, db) {
                            (Ok(ta), Ok(tb)) => tb.cmp(&ta),
                            _ => b.pub_date.cmp(&a.pub_date),
                        }
                    });

                    cached_items.truncate(1000);

                    if let Ok(cache_data) = serde_json::to_string(&cached_items) {
                        let _ = tokio::fs::write(&cache_path, cache_data).await;
                    }
                    let _ = tx
                        .send(TuiEvent::NewsFetched(cached_items, "just now".into()))
                        .await;
                    if let Some(total) = parsed_total {
                        let _ = tx.send(TuiEvent::NewsTotalCount(total)).await;
                    }
                    return;
                }
            }
            _ => {}
        }

        if let Ok(data) = std::fs::read_to_string(&cache_path)
            && let Ok(items) = serde_json::from_str::<Vec<NewsItem>>(&data)
        {
            let _ = tx.send(TuiEvent::NewsFetched(items, "cached".into())).await;
            return;
        }
        let _ = tx
            .send(TuiEvent::NewsFetchFailed(
                "failed to fetch arch news".into(),
            ))
            .await;
    });
}
