pub fn print_rich_text(text: &str) {
    println!("\x1b[1;32m{}\x1b[0m", text);
}

pub fn print_info(text: &str) {
    println!("\x1b[1;34m[INFO]\x1b[0m {}", text);
}

pub fn print_warning(text: &str) {
    println!("\x1b[1;33m[WARN]\x1b[0m {}", text);
}

pub fn print_error(text: &str) {
    eprintln!("\x1b[1;31m[ERROR]\x1b[0m {}", text);
}
