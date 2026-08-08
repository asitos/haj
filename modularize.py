import os

with open('src/tui/mod.rs', 'r') as f:
    lines = f.readlines()

# Find fetch_arch_news and run_app
start_news = -1
end_news = -1
start_run_app = -1
end_run_app = -1

for i, line in enumerate(lines):
    if line.startswith('fn parse_arch_xml('):
        start_news = i
    elif line.startswith('pub async fn run() -> Result<()>'):
        end_news = i
    elif line.startswith('async fn run_app<B: Backend>('):
        start_run_app = i

end_run_app = len(lines)

with open('src/tui/news_fetch.rs', 'w') as f:
    f.write('use super::{NewsItem, TuiEvent};\nuse tokio::sync::mpsc;\nuse std::time::Duration;\n\n')
    f.writelines(lines[start_news:end_news])

with open('src/tui/events.rs', 'w') as f:
    f.write('use super::*;\nuse ratatui::backend::Backend;\nuse ratatui::Terminal;\nuse anyhow::Result;\nuse crossterm::event::{self, Event, KeyCode, KeyModifiers};\nuse tokio::sync::mpsc;\nuse std::time::{Duration, Instant};\n\n')
    f.writelines(lines[start_run_app:end_run_app])

with open('src/tui/mod.rs', 'w') as f:
    for i in range(len(lines)):
        if i == start_news:
            f.write('pub mod news_fetch;\npub mod events;\nuse news_fetch::*;\n')
        if i >= start_news and i < end_news:
            continue
        if i >= start_run_app and i < end_run_app:
            continue
        f.write(lines[i])

