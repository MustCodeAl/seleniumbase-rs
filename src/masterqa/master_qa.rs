use crate::BaseCase;
use std::io::{self, Write};

pub struct MasterQA {
    pub sb: BaseCase,
}

impl MasterQA {
    pub fn verify(&self, question: &str) -> bool {
        print!("Manual QA verification required: {} [Y/n] ", question);
        let _ = io::stdout().flush();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let input = input.trim().to_lowercase();
            if input == "n" || input == "no" {
                return false;
            }
        }
        true
    }
}
