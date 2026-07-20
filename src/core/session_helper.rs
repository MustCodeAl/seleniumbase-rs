use std::collections::HashSet;
use thirtyfour::WindowHandle;

/// Close all other windows and return to the original window handle.
pub async fn close_other_windows<T: AsRef<str>>(
    driver: &thirtyfour::WebDriver,
    current: T,
) -> Result<(), thirtyfour::error::WebDriverError> {
    let current: WindowHandle = current.as_ref().into();
    let handles = driver.windows().await?;
    for h in handles {
        if h != current {
            driver.switch_to_window(h).await?;
            driver.close_window().await?;
        }
    }
    driver.switch_to_window(current).await?;
    Ok(())
}

/// Switch to the newest open window.
pub async fn switch_to_newest(
    driver: &thirtyfour::WebDriver,
) -> Result<String, thirtyfour::error::WebDriverError> {
    let handles = driver.windows().await?;
    let newest = handles.into_iter().last();
    match newest {
        Some(handle) => {
            let s = handle.to_string();
            driver.switch_to_window(handle).await?;
            Ok(s)
        }
        None => Ok(String::new()),
    }
}

/// Collect unique window handles.
pub async fn collect_window_handles(
    driver: &thirtyfour::WebDriver,
) -> Result<HashSet<String>, thirtyfour::error::WebDriverError> {
    let handles = driver.windows().await?;
    Ok(handles.into_iter().map(|h| h.to_string()).collect())
}
