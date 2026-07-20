// Cookie and storage helpers.

impl BaseCase {
    /// Deletes all cookies for the current domain.
    pub async fn clear_all_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.session.driver().delete_all_cookies().await.map_err(SeleniumBaseError::WebDriver)
    }

    /// Clears all local storage.
    pub async fn delete_local_storage(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("window.localStorage.clear();").await?;
        Ok(())
    }

    /// Clears all session storage.
    pub async fn delete_session_storage(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script("window.sessionStorage.clear();").await?;
        Ok(())
    }

    /// Returns cookies formatted as a single `name=value; ...` string.
    pub async fn get_cookie_string(&self) -> Result<String, SeleniumBaseError> {
        let cookies = self.session.get_cookies().await?;
        let mut parts = Vec::new();
        if let Value::Array(arr) = cookies {
            for cookie in arr {
                if let (Some(name), Some(value)) = (cookie["name"].as_str(), cookie["value"].as_str()) {
                    parts.push(format!("{}={}", name, value));
                }
            }
        }
        Ok(parts.join("; "))
    }
}
