// Download helpers.

/// Checks that the file `filename` in `folder` holds `data`, ignoring the
/// whitespace at the ends of `data`, as Python's `assert_data_in_downloaded_file`
/// does.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::AssertionFailed`] naming both when the data is
/// not in the file, and the error of reading it otherwise.
fn check_downloaded_data(
    folder: &Path,
    data: &str,
    filename: &str,
) -> Result<(), SeleniumBaseError> {
    let path = crate::artifacts::confined_path(folder, filename)?;
    let actual = fs::read_to_string(&path)?;
    if actual.contains(data.trim()) {
        Ok(())
    } else {
        Err(SeleniumBaseError::AssertionFailed(format!(
            "Expected data [{}] is not in downloaded file [{filename}]!",
            data.trim()
        )))
    }
}

/// Whether a file in `folder` has a name that `re` matches. A folder that is
/// not there yet holds no file.
fn folder_has_match(folder: &Path, re: &Regex) -> Result<bool, SeleniumBaseError> {
    if !folder.exists() {
        return Ok(false);
    }
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| re.is_match(name))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Waits up to `timeout_secs` for a file in `folder` whose name `regex`
/// matches, for a download that is still being written.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for a regex that does not
/// compile and [`SeleniumBaseError::AssertionFailed`] if no file matches when
/// the timeout ends.
async fn wait_for_downloaded_name(
    folder: &Path,
    regex: &str,
    timeout_secs: u64,
) -> Result<(), SeleniumBaseError> {
    let re = regex_or_err(regex)?;
    poll_until_true(timeout_secs, String::new(), || async {
        folder_has_match(folder, &re)
    })
    .await
    .map_err(|error| match error {
        SeleniumBaseError::WaitTimeout(_) => SeleniumBaseError::AssertionFailed(format!(
            "No downloaded file matched regex '{regex}' after {timeout_secs}s"
        )),
        other => other,
    })
}

impl BaseCase {
    /// Returns the path to the browser downloads folder.
    pub fn get_downloads_folder(&self) -> PathBuf {
        default_download_dir()
    }

    /// Returns the full path of `filename` inside the downloads folder.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `filename` is not a plain
    /// file name (no directory part, no `..`), so it cannot reach a file
    /// outside the downloads folder.
    pub fn get_path_of_downloaded_file(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        crate::artifacts::confined_path(&default_download_dir(), filename)
    }

    /// Returns true if `filename` exists in the downloads folder.
    pub fn is_downloaded_file_present(&self, filename: &str) -> bool {
        self.get_path_of_downloaded_file(filename)
            .is_ok_and(|path| path.exists())
    }

    /// Returns true if any downloaded file matches `regex`.
    pub fn is_downloaded_file_regex_present(&self, regex: &str) -> Result<bool, SeleniumBaseError> {
        folder_has_match(&default_download_dir(), &regex_or_err(regex)?)
    }

    /// Reads the contents of `filename` from the downloads folder.
    pub fn get_data_from_downloaded_file(&self, filename: &str) -> Result<String, SeleniumBaseError> {
        let path = self.get_path_of_downloaded_file(filename)?;
        Ok(fs::read_to_string(&path)?)
    }

    /// Asserts that the name of a downloaded file matches `regex`, waiting up
    /// to the default timeout ([`set_timeout`](Self::set_timeout)) for a
    /// download that is still being written, as Python's
    /// `assert_downloaded_file_regex` does.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a regex that does not
    /// compile and [`SeleniumBaseError::AssertionFailed`] if no file matches
    /// when the timeout ends.
    pub async fn assert_downloaded_file_regex(&self, regex: &str) -> Result<(), SeleniumBaseError> {
        wait_for_downloaded_name(&default_download_dir(), regex, self.default_wait_secs()).await?;
        let message = demo::bracketed_message("ASSERT DOWNLOADED FILE REGEX", regex);
        self.demo_assert_success(&message, None).await;
        Ok(())
    }

    /// Asserts that the downloaded file `filename` contains `data` (ignoring
    /// whitespace at its ends), waiting up to the default timeout for the file
    /// to arrive. The data comes first and the file second, as in Python's
    /// `assert_data_in_downloaded_file(data, file)`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] if the file does not
    /// arrive or does not hold the data.
    pub async fn assert_data_in_downloaded_file(
        &self,
        data: &str,
        filename: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.assert_downloaded_file(filename).await?;
        check_downloaded_data(&default_download_dir(), data, filename)
    }

    /// Deletes `filename` from the downloads folder if it exists.
    pub fn delete_downloaded_file_if_present(&self, filename: &str) -> Result<(), SeleniumBaseError> {
        let path = self.get_path_of_downloaded_file(filename)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_downloads {
    use super::*;

    #[test]
    fn the_data_comes_first_and_the_file_second() {
        // The arguments used to be (file, data), the reverse of Python's, so a
        // call written like Python's looked for a file named after the data.
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("report.csv"), "id,name\n1,Ada\n").unwrap();

        check_downloaded_data(folder.path(), "1,Ada", "report.csv").unwrap();
        let error = check_downloaded_data(folder.path(), "2,Grace", "report.csv").unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::AssertionFailed(m)
                if m.contains("2,Grace") && m.contains("report.csv")),
            "{error:?}"
        );
    }

    #[test]
    fn whitespace_at_the_ends_of_the_data_is_ignored() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("a.txt"), "hello world").unwrap();
        check_downloaded_data(folder.path(), "  hello\n", "a.txt").unwrap();
        // What is inside the data still counts.
        check_downloaded_data(folder.path(), "hello  world", "a.txt").unwrap_err();
    }

    #[tokio::test]
    async fn the_regex_wait_sees_a_file_that_arrives_late() {
        let folder = tempfile::tempdir().unwrap();
        let arriving = folder.path().join("report-7.csv");
        let writer = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            fs::write(arriving, "id\n").unwrap();
        });

        wait_for_downloaded_name(folder.path(), r"^report-\d+\.csv$", 5)
            .await
            .expect("the file showed up within the timeout");
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn the_regex_wait_fails_after_the_timeout_and_for_a_bad_regex() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("other.txt"), "x").unwrap();

        let error = wait_for_downloaded_name(folder.path(), r"^report", 0)
            .await
            .unwrap_err();
        assert!(
            matches!(&error, SeleniumBaseError::AssertionFailed(m) if m.contains("report")),
            "{error:?}"
        );
        let error = wait_for_downloaded_name(folder.path(), "(", 0)
            .await
            .unwrap_err();
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(_)), "{error:?}");
    }

    #[test]
    fn a_folder_that_is_not_there_holds_no_match() {
        let folder = tempfile::tempdir().unwrap();
        let re = Regex::new(".").unwrap();
        assert!(!folder_has_match(&folder.path().join("nope"), &re).unwrap());
    }

    #[test]
    fn a_missing_file_or_a_path_instead_of_a_name_is_an_error() {
        let folder = tempfile::tempdir().unwrap();
        assert!(matches!(
            check_downloaded_data(folder.path(), "x", "missing.txt"),
            Err(SeleniumBaseError::Io(_))
        ));
        assert!(matches!(
            check_downloaded_data(folder.path(), "x", "../a.txt"),
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
    }
}
