use std::process::Command;

#[test]
fn test_display3d_lines() {
    let mut child = Command::new("display3d")
        .args(&["resources/blahaj.obj", "-t", "0,0.5,7.5"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
        
    std::thread::sleep(std::time::Duration::from_millis(100));
    child.kill().unwrap();
    
    let output = child.wait_with_output().unwrap();
    
    let s = String::from_utf8_lossy(&output.stdout);
    
    let mut h_count = 0;
    let mut j_count = 0;
    for (i, _) in s.match_indices("\x1B[H") { h_count += 1; }
    for (i, _) in s.match_indices("\x1B[2J") { j_count += 1; }
    
    println!("H count: {}, 2J count: {}", h_count, j_count);
    
    let first_h = s.find("\x1B[H").unwrap_or(0);
    let second_h = s[first_h+1..].find("\x1B[H").unwrap_or(s.len());
    let frame = &s[first_h..first_h+1+second_h];
    
    let lines: Vec<&str> = frame.split('\n').collect();
    println!("Lines in frame: {}", lines.len());
    
    panic!("show me");
}
