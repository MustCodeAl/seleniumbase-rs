// Cookie and storage helpers.

/// `cookies` as one `name=value; name=value` string, the form of a `Cookie`
/// request header.
fn cookie_header<'a>(cookies: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    cookies
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

impl BaseCase {
    /// Deletes all cookies for the current domain.
    pub async fn clear_all_cookies(&self) -> Result<(), SeleniumBaseError> {
        self.session.webdriver()?.delete_all_cookies().await.map_err(SeleniumBaseError::WebDriver)
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

    /// Returns the cookies of the current page formatted as a single
    /// `name=value; ...` string, the form of a `Cookie` request header.
    ///
    /// Cookies that scripts cannot read (`HttpOnly`) are included. Works in
    /// any WebDriver session; it does not need CDP.
    ///
    /// # Errors
    ///
    /// Returns the WebDriver error if the cookies cannot be read.
    pub async fn get_cookie_string(&self) -> Result<String, SeleniumBaseError> {
        let cookies = self.session.webdriver()?.get_all_cookies().await?;
        Ok(cookie_header(
            cookies
                .iter()
                .map(|cookie| (cookie.name.as_str(), cookie.value.as_str())),
        ))
    }
}

#[cfg(test)]
mod tests_storage {
    use super::*;

    #[test]
    fn cookies_are_joined_the_way_a_cookie_header_is() {
        assert_eq!(cookie_header([("sid", "abc123"), ("theme", "dark")]), "sid=abc123; theme=dark");
        assert_eq!(cookie_header([("only", "one")]), "only=one");
    }

    #[test]
    fn no_cookies_make_an_empty_string() {
        assert_eq!(cookie_header([]), "");
    }

    #[test]
    fn a_value_is_passed_through_exactly() {
        // Values may hold `=` and quotes (a base64 padding, a JSON blob).
        assert_eq!(cookie_header([("t", "a==")]), "t=a==");
        assert_eq!(cookie_header([("j", "{\"a\":1}")]), "j={\"a\":1}");
        assert_eq!(cookie_header([("empty", "")]), "empty=");
    }
}
