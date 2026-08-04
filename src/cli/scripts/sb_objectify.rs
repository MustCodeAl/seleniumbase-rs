use std::path::PathBuf;

pub fn objectify_page() -> std::io::Result<PathBuf> {
    let path = PathBuf::from("page_object.rs");
    let content = r#"use seleniumbase_rs::{BaseCase, SeleniumBaseError};

pub struct LoginPage<'a> {
    sb: &'a mut BaseCase,
}

impl<'a> LoginPage<'a> {
    pub fn new(sb: &'a mut BaseCase) -> Self {
        Self { sb }
    }

    pub async fn open(&mut self) -> Result<(), SeleniumBaseError> {
        self.sb.open("https://example.com/login").await
    }
}
"#;
    std::fs::write(&path, content)?;
    Ok(path)
}
