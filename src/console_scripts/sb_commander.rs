use std::process::Command;

pub fn run_commander() {
    println!("Executing SeleniumBase commander command: default");
}

pub fn commander(command: &str) {
    println!("Executing SeleniumBase commander command: {}", command);
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output();
        
    match output {
        Ok(out) => {
            if !out.stdout.is_empty() {
                println!("{}", String::from_utf8_lossy(&out.stdout));
            }
            if !out.stderr.is_empty() {
                eprintln!("{}", String::from_utf8_lossy(&out.stderr));
            }
        }
        Err(e) => {
            eprintln!("Failed to execute commander command: {}", e);
        }
    }
}
