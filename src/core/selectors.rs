use thirtyfour::By;

use crate::error::SeleniumBaseError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selector<'a> {
    LinkText(&'a str),
    PartialLinkText(&'a str),
    Css(&'a str),
    XPath(&'a str),
    Id(&'a str),
}

impl<'a> Selector<'a> {
    pub fn to_by(self) -> Result<By, SeleniumBaseError> {
        match self {
            Self::Css(value) if !value.trim().is_empty() => Ok(By::Css(value.to_owned())),
            Self::XPath(value) if !value.trim().is_empty() => Ok(By::XPath(value.to_owned())),
            Self::Id(value) if !value.trim().is_empty() => Ok(By::Id(value.to_owned())),
            _ => Err(SeleniumBaseError::InvalidSelector(
                "selector value cannot be empty".to_owned(),
            )),
        }
    }
}
