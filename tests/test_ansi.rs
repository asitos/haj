use ansi_to_tui::IntoText;

#[test]
fn test_ansi_parsing() {
    let s = "\x1B[2J\x1B[10;1HHello";
    let text = s.into_text().unwrap();
    println!("Lines: {}", text.lines.len());
    panic!("show me");
}
