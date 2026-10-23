//! Steps about the page as a whole: navigation, windows and drivers, alerts,
//! cookies and storage, downloads.

use super::{file_name_of, DOWNLOAD_SECONDS};

sb_steps! {
    /// Opens a URL.
    fn open_url(sb, url: String) { sb.open(&url).await }
        "Open {url:string}", "Open URL {url:string}", "User opens {url:string}",
        "User opens URL {url:string}", "User goes to {url:string}",
        "User goes to URL {url:string}";

    /// Opens a URL unless the browser is already there.
    fn open_if_not_url(sb, url: String) { sb.open_if_not_url(&url).await }
        "Open if not {url:string}", "Open if not URL {url:string}",
        "User opens {url:string} if not on page", "User opens URL {url:string} if not on page";

    /// Reloads the page.
    fn refresh_page(sb) { sb.refresh_page().await }
        "Refresh page", "Reload page", "User refreshes the page", "User reloads the page";

    /// Goes back in the history.
    fn go_back(sb) { sb.go_back().await }
        "Go back", "User goes back", "User navigates back";

    /// Goes forward in the history.
    fn go_forward(sb) { sb.go_forward().await }
        "Go forward", "User goes forward", "User navigates forward";

    /// Opens a new tab and switches to it.
    fn open_new_window(sb) { sb.open_new_window().await }
        "Open new tab", "Open new window", "User opens new tab", "User opens new window";

    /// Maximizes the window.
    fn maximize_window(sb) { sb.maximize_window().await }
        "Maximize window", "Maximize the window", "User maximizes window",
        "User maximizes the window";

    /// Starts another browser and makes it the current one.
    fn get_new_driver(sb) { sb.get_new_driver().await.map(drop) }
        "Get new driver", "User gets new driver";

    /// Goes back to the first browser.
    fn switch_to_default_driver(sb) { sb.switch_to_default_driver().await }
        "Switch to default driver", "User switches to default driver";

    /// Accepts the alert.
    fn accept_alert(sb) { sb.accept_alert().await }
        "Accept alert", "User accepts alert";

    /// Dismisses the alert.
    fn dismiss_alert(sb) { sb.dismiss_alert().await }
        "Dismiss alert", "User dismisses alert";

    /// Deletes all cookies.
    fn delete_all_cookies(sb) { sb.delete_all_cookies().await }
        "Clear all cookies", "Delete all cookies", "User clears all cookies",
        "User deletes all cookies";

    /// Clears the local storage.
    fn clear_local_storage(sb) { sb.clear_local_storage().await }
        "Clear Local Storage", "Delete Local Storage", "User clears Local Storage",
        "User deletes Local Storage";

    /// Clears the session storage.
    fn clear_session_storage(sb) { sb.clear_session_storage().await }
        "Clear Session Storage", "Delete Session Storage", "User clears Session Storage",
        "User deletes Session Storage";

    /// Downloads a URL into the downloads folder.
    fn download_file(sb, file: String) {
        let name = file_name_of(&file);
        sb.download_file(&file, &name, DOWNLOAD_SECONDS).await.map(drop)
    }
        "Download {file:string} to downloads", "Download file {file:string} to downloads",
        "User downloads {file:string} to downloads";

    /// Downloads a URL, then copies the file into a folder.
    fn download_file_to_destination(sb, file: String, destination: String) {
        let name = file_name_of(&file);
        let downloaded = sb.download_file(&file, &name, DOWNLOAD_SECONDS).await?;
        std::fs::create_dir_all(&destination)?;
        std::fs::copy(&downloaded, std::path::Path::new(&destination).join(&name))?;
        Ok(())
    }
        "Download {file:string} to {destination:string}",
        "Download file {file:string} to {destination:string}",
        "User downloads {file:string} to {destination:string}";
}
