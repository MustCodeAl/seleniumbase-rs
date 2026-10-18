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
| `download_file(url, filename, timeout_secs)` | Navigates to `url` in a new tab, waits up to `timeout_secs` for `filename` in the downloads folder, returns to the first tab and returns the file's `PathBuf`. |
| `save_file_as(data, filename)` | Saves raw bytes to the logs directory. `filename` must be a plain file name. |
| `print_to_pdf(filename)` | Saves the current page as a PDF. |
| `get_downloads_folder()` | Returns the default downloads directory. |
| `get_downloaded_files()` | Returns a list of downloaded file names. |
| `is_downloaded_file_present(filename)` | Returns true if the file exists in downloads. |
| `assert_downloaded_file(filename)` | Waits up to the default timeout for the file, then fails if it is not present. |
| `get_pdf_text(filename)` | Extracts text from a PDF file. |
| `assert_pdf_text(filename, text)` | Fails if the PDF does not contain `text`. |
