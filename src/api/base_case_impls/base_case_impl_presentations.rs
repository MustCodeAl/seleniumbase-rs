// Presentation helpers.

/// Writes `presentation` as a new HTML file in `dir`, and returns the file
/// together with its `file://` URL. The file is left in place.
fn write_presentation_page(
    presentation: &Presentation,
    dir: &Path,
) -> Result<(PathBuf, String), SeleniumBaseError> {
    let path = artifact_path(dir, "presentation", "html");
    presentation.save(&path)?;
    let url = file_url(&path, "presentation")?;
    Ok((path, url))
}

impl BaseCase {
    /// Creates a presentation, saves it as an HTML file in the logs folder, and
    /// opens it in the browser.
    ///
    /// Returns the path of the file, which stays in the logs folder. Slides
    /// added afterwards are not in that file until it is saved again with
    /// [`BaseCase::save_presentation`].
    ///
    /// # Errors
    ///
    /// Returns an I/O error if the file cannot be written. The page loads
    /// reveal.js from a CDN, so it shows unstyled slides when the browser is
    /// offline.
    pub async fn begin_presentation(&mut self, title: &str) -> Result<PathBuf, SeleniumBaseError> {
        self.create_presentation(title).await?;
        let presentation = self.presentation.as_ref().ok_or_else(|| {
            SeleniumBaseError::InvalidConfig("No presentation created.".to_owned())
        })?;
        let (path, url) = write_presentation_page(presentation, &ensure_latest_logs_dir()?)?;
        self.open(&url).await?;
        Ok(path)
    }

    /// Adds a slide to the current presentation.
    pub async fn add_slide(&mut self, content: &str) -> Result<(), SeleniumBaseError> {
        self.add_presentation_slide(content).await
    }
}

#[cfg(test)]
mod tests_presentations {
    use super::*;

    #[test]
    fn a_begun_presentation_stays_on_disk_for_the_path_it_returns() {
        let logs = tempfile::tempdir().unwrap();
        let mut presentation = Presentation::new("Demo");
        presentation.add_slide("# Intro");

        let (path, url) = write_presentation_page(&presentation, logs.path()).unwrap();
        assert!(path.starts_with(logs.path()));
        assert!(path.is_file(), "the returned path must exist: {}", path.display());
        assert!(std::fs::read_to_string(&path).unwrap().contains("<h1>Intro</h1>"));
        assert!(url.starts_with("file://") && url.ends_with(".html"), "{url}");
    }

    #[tokio::test]
    async fn slides_need_a_presentation() {
        let mut sb = BaseCase::with_session(BrowserConfig::default(), BrowserSession::disconnected());
        let error = sb.add_slide("# x").await.unwrap_err();
        assert!(matches!(error, SeleniumBaseError::InvalidConfig(m) if m.contains("No presentation created")));
        sb.create_presentation("t").await.unwrap();
        sb.add_slide("# x").await.unwrap();
        assert_eq!(sb.presentation.as_ref().unwrap().slides.len(), 1);
    }
}
