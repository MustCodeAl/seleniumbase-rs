//! The other steps: sleeping, screenshots, the page saved to the logs, demo
//! mode and CDP mode.

use super::{normalize_text, with_png};

sb_steps! {
    /// Sleeps.
    fn sleep(sb, seconds: f64) {
        sb.sleep(seconds).await;
        Ok(())
    }
        "Sleep for {seconds:f64} seconds", "Wait for {seconds:f64} seconds",
        "User sleeps for {seconds:f64} seconds", "User waits for {seconds:f64} seconds";

    /// Saves a screenshot under a name.
    fn save_screenshot_as(sb, name: String) {
        sb.save_screenshot(&with_png(&normalize_text(&name))).await.map(drop)
    }
        "Save screenshot as {name:string}", "User saves screenshot as {name:string}";

    /// Saves a screenshot into a folder.
    fn save_screenshot_to_folder_as(sb, folder: String, name: String) {
        std::fs::create_dir_all(&folder)?;
        let path = std::path::Path::new(&folder).join(with_png(&normalize_text(&name)));
        sb.save_screenshot_to_path(path).await
    }
        "Save screenshot to {folder:string} as {name:string}",
        "User saves screenshot to {folder:string} as {name:string}";

    /// Saves a screenshot to the logs.
    fn save_screenshot_to_logs(sb) { sb.save_screenshot_to_logs().await.map(drop) }
        "Save screenshot to logs", "Save a screenshot to the logs",
        "User saves screenshot to logs", "User saves a screenshot to the logs";

    /// Saves the page as a PDF in the logs.
    fn save_as_pdf_to_logs(sb) { sb.save_as_pdf_to_logs("page.pdf").await.map(drop) }
        "Save as PDF to logs", "Save as PDF to the logs", "User saves page as PDF to logs",
        "User saves page as PDF to the logs";

    /// Saves the source of the page in the logs.
    fn save_page_source_to_logs(sb) { sb.save_page_source_to_logs().await.map(drop) }
        "Save page source to logs", "Save the page source to the logs",
        "User saves page source to logs", "User saves the page source to the logs";

    /// Turns demo mode on.
    fn activate_demo_mode(sb) { sb.activate_demo_mode().await }
        "Activate Demo Mode", "User activates Demo Mode";

    /// Turns demo mode off.
    fn deactivate_demo_mode(sb) { sb.deactivate_demo_mode().await }
        "Deactivate Demo Mode", "User deactivates Demo Mode";

    /// Switches the browser to CDP mode.
    fn activate_cdp_mode(sb) { sb.activate_cdp_mode().await }
        "Activate CDP Mode", "User activates CDP Mode";
}
