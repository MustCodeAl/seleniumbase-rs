use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Exception {
    TimeoutException(String),
    NoSuchElementException(String),
    TextNotVisibleException(String),
    ElementNotVisibleException(String),
    WebDriverException(String),
    NotConnectedException(String),
    OutOfScopeException(String),
    InvalidInputException(String),
}

impl Exception {
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::TimeoutException(message.into())
    }

    pub fn no_such_element(message: impl Into<String>) -> Self {
        Self::NoSuchElementException(message.into())
    }

    pub fn text_not_visible(message: impl Into<String>) -> Self {
        Self::TextNotVisibleException(message.into())
    }

    pub fn element_not_visible(message: impl Into<String>) -> Self {
        Self::ElementNotVisibleException(message.into())
    }

    pub fn webdriver(message: impl Into<String>) -> Self {
        Self::WebDriverException(message.into())
    }

    pub fn not_connected(message: impl Into<String>) -> Self {
        Self::NotConnectedException(message.into())
    }

    pub fn out_of_scope(message: impl Into<String>) -> Self {
        Self::OutOfScopeException(message.into())
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInputException(message.into())
    }

    pub fn message(&self) -> &str {
        match self {
            Self::TimeoutException(m) => m,
            Self::NoSuchElementException(m) => m,
            Self::TextNotVisibleException(m) => m,
            Self::ElementNotVisibleException(m) => m,
            Self::WebDriverException(m) => m,
            Self::NotConnectedException(m) => m,
            Self::OutOfScopeException(m) => m,
            Self::InvalidInputException(m) => m,
        }
    }

    fn variant_name(&self) -> &'static str {
        match self {
            Self::TimeoutException(_) => "TimeoutException",
            Self::NoSuchElementException(_) => "NoSuchElementException",
            Self::TextNotVisibleException(_) => "TextNotVisibleException",
            Self::ElementNotVisibleException(_) => "ElementNotVisibleException",
            Self::WebDriverException(_) => "WebDriverException",
            Self::NotConnectedException(_) => "NotConnectedException",
            Self::OutOfScopeException(_) => "OutOfScopeException",
            Self::InvalidInputException(_) => "InvalidInputException",
        }
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.variant_name(), self.message())
    }
}

impl std::error::Error for Exception {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exception_constructors() {
        let ex = Exception::timeout("waited too long");
        assert!(matches!(ex, Exception::TimeoutException(_)));
        assert_eq!(ex.message(), "waited too long");
    }

    #[test]
    fn exception_display_includes_name_and_message() {
        let ex = Exception::no_such_element("button missing");
        let text = ex.to_string();
        assert!(text.contains("NoSuchElementException"));
        assert!(text.contains("button missing"));
    }

    #[test]
    fn common_exception_variants_exist() {
        let variants = vec![
            Exception::text_not_visible("t"),
            Exception::element_not_visible("e"),
            Exception::webdriver("w"),
            Exception::not_connected("n"),
            Exception::out_of_scope("o"),
            Exception::invalid_input("i"),
        ];
        for ex in variants {
            assert!(!ex.message().is_empty());
        }
    }
}
