---
title: "Files, downloads and PDF"
sidebarTitle: "Files and PDF"
description: "Download and upload files and work with PDFs."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## File downloads, uploads, and PDF

| Method | Description |
|--------|-------------|
| `choose_file(selector, path)` | Sets a file input value. |
| `download_file(file_url)` | Fetches `file_url` over HTTP with the browser's user agent, sends none of the page's cookies, and saves the body in `get_downloads_folder()` under the last part of the URL's path. Returns the file's `PathBuf`; fails with `Network` when the request fails or the status is not a success. Returns a call builder with `.destination_folder(impl AsRef<Path>)`, a folder that is created when it is missing. |
| `save_file_as(file_url, new_file_name)` | The same as `download_file`, under `new_file_name`, which must be a plain file name. Same call builder. |
| `download_file_in_browser(url, filename, timeout_secs)` | Navigates to `url` in a new tab, waits up to `timeout_secs` for `filename` in the downloads folder, returns to the first tab and returns the file's `PathBuf`. This is the browser-tab download that `download_file` used to be. |
| `save_data_as(data, filename)` | Writes the text `data` to `filename` in the logs directory, replacing the file. |
| `print_to_pdf(filename)` | Saves the current page as a PDF. |
| `get_downloads_folder()` | Returns the default downloads directory. |
| `get_downloaded_files()` | Returns a list of downloaded file names. |
| `is_downloaded_file_present(filename)` | Returns true if the file exists in downloads. |
| `assert_downloaded_file(filename)` | Waits up to the default timeout for the file, then fails if it is not present. |
| `get_pdf_text(filename)` | Extracts text from a PDF file. |
| `assert_pdf_text(filename, text)` | Fails if the PDF does not contain `text`. |
