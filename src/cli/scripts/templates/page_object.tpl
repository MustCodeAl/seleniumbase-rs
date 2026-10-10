use seleniumbase_rs::{BaseCase, SeleniumBaseError};

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
