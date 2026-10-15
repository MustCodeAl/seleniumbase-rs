//! Reading text from screenshots and images (feature `ocr`).
//!
//! [`Ocr`] runs [ocrs](https://github.com/robertknight/ocrs), a pure-Rust OCR
//! engine: nothing to install. Its two models (about 12 MB) are downloaded once
//! into the cache folder and checked against pinned SHA-256 hashes. To use your
//! own copy, for example on a machine without network access, set
//! `SB_OCR_MODELS` to a folder that holds `text-detection.rten` and
//! `text-recognition.rten`.
//!
//! ```no_run
//! use seleniumbase_rs::ocr::Ocr;
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let ocr = Ocr::load().await?;
//! let browser = Browser::launch(LaunchOptions::default()).await?;
//! let page = browser.default_page().await?;
//! page.goto("https://example.com").await?;
//! let text = ocr.read_page(&page).await?; // the text the page shows, line by line
//! assert!(text.contains("Example Domain"));
//! # Ok(())
//! # }
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;

use crate::error::SeleniumBaseError;
use crate::plugins::observer::PageEvidence;

/// The model files and their SHA-256 hashes.
const MODELS: [(&str, &str); 2] = [
    (
        "text-detection.rten",
        "f15cfb56bd02c4bf478a20343986504a1f01e1665c2b3a0ad66340f054b1b5ca",
    ),
    (
        "text-recognition.rten",
        "e484866d4cce403175bd8d00b128feb08ab42e208de30e42cd9889d8f1735a6e",
    ),
];
const MODEL_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/";

/// An OCR engine with its models loaded. Loading is the slow part; clone it to
/// share the engine.
#[derive(Clone)]
pub struct Ocr {
    engine: Arc<OcrEngine>,
}

impl std::fmt::Debug for Ocr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Ocr")
    }
}

impl Ocr {
    /// Loads the models from `SB_OCR_MODELS`, or from the cache folder,
    /// downloading them there first when they are missing or damaged.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Network`] when a download fails,
    /// [`SeleniumBaseError::InvalidConfig`] when a model does not match its
    /// hash or cannot be loaded.
    pub async fn load() -> Result<Self, SeleniumBaseError> {
        let dir = if let Some(dir) = std::env::var_os("SB_OCR_MODELS") {
            PathBuf::from(dir)
        } else {
            let dir = dirs::cache_dir()
                .ok_or_else(|| {
                    SeleniumBaseError::InvalidConfig(
                        "no cache folder for the OCR models; set SB_OCR_MODELS".into(),
                    )
                })?
                .join("seleniumbase-rs")
                .join("ocr");
            download_missing(&dir).await?;
            dir
        };
        tokio::task::spawn_blocking(move || Self::from_dir(&dir))
            .await
            .map_err(|error| {
                SeleniumBaseError::InvalidConfig(format!("loading the OCR models stopped: {error}"))
            })?
    }

    /// Loads `text-detection.rten` and `text-recognition.rten` from `dir`.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] when a model cannot be loaded.
    pub fn from_dir(dir: &Path) -> Result<Self, SeleniumBaseError> {
        let load = |name: &str| {
            let path = dir.join(name);
            Model::load_file(&path).map_err(|error| {
                SeleniumBaseError::InvalidConfig(format!(
                    "cannot load OCR model {}: {error}",
                    path.display()
                ))
            })
        };
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(load(MODELS[0].0)?),
            recognition_model: Some(load(MODELS[1].0)?),
            ..OcrEngineParams::default()
        })
        .map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!("cannot start the OCR engine: {error}"))
        })?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }

    /// The text in a PNG image, one line per line of text, top to bottom.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] when the bytes are not a PNG image
    /// or the engine fails.
    pub async fn text_in_png(&self, png: &[u8]) -> Result<String, SeleniumBaseError> {
        let engine = Arc::clone(&self.engine);
        let png = png.to_vec();
        tokio::task::spawn_blocking(move || read(&engine, &png))
            .await
            .map_err(|error| SeleniumBaseError::InvalidConfig(format!("OCR stopped: {error}")))?
    }

    /// The text a page shows, read from its screenshot. Works with a
    /// `BaseCase` and with an `sb_cdp::Page`.
    ///
    /// # Errors
    ///
    /// The screenshot's error, or [`text_in_png`](Self::text_in_png)'s.
    pub async fn read_page(&self, page: &dyn PageEvidence) -> Result<String, SeleniumBaseError> {
        let png = page.screenshot_png().await?;
        self.text_in_png(&png).await
    }
}

fn read(engine: &OcrEngine, png: &[u8]) -> Result<String, SeleniumBaseError> {
    let failed = |what: &str, error: &dyn std::fmt::Display| {
        SeleniumBaseError::InvalidConfig(format!("OCR {what}: {error}"))
    };
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|error| failed("cannot read the image", &error))?
        .into_rgb8();
    let source = ImageSource::from_bytes(image.as_raw(), image.dimensions())
        .map_err(|error| failed("cannot use the image", &error))?;
    let input = engine
        .prepare_input(source)
        .map_err(|error| failed("cannot prepare the image", &error))?;
    let words = engine
        .detect_words(&input)
        .map_err(|error| failed("found no words", &error))?;
    let lines = engine.find_text_lines(&input, &words);
    let text = engine
        .recognize_text(&input, &lines)
        .map_err(|error| failed("cannot read the lines", &error))?;
    Ok(text
        .iter()
        .flatten()
        .map(ToString::to_string)
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Downloads each model that is missing or does not match its hash.
async fn download_missing(dir: &Path) -> Result<(), SeleniumBaseError> {
    tokio::fs::create_dir_all(dir).await?;
    crate::install_tls_provider();
    let client = reqwest::Client::new();
    for (name, hash) in MODELS {
        let path = dir.join(name);
        let complete = tokio::fs::read(&path)
            .await
            .is_ok_and(|existing| sha256_hex(&existing) == hash);
        if complete {
            continue;
        }
        let url = format!("{MODEL_URL}{name}");
        let network = |error: reqwest::Error| {
            SeleniumBaseError::network(
                &url,
                error.status().map_or(0, |s| s.as_u16()),
                error.to_string(),
            )
        };
        let bytes = client
            .get(&url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(network)?
            .bytes()
            .await
            .map_err(network)?;
        if sha256_hex(&bytes) != hash {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "the OCR model from {url} does not match its expected hash"
            )));
        }
        let dir = dir.to_path_buf();
        tokio::task::spawn_blocking(move || save(&dir, &path, &bytes))
            .await
            .map_err(|error| {
                SeleniumBaseError::InvalidConfig(format!("saving an OCR model stopped: {error}"))
            })??;
    }
    Ok(())
}

/// Writes `bytes` to a temporary file of its own in `dir` and renames it to
/// `path`: a cut write never looks complete, and two programs that download at
/// once do not write into the same file.
fn save(dir: &Path, path: &Path, bytes: &[u8]) -> Result<(), SeleniumBaseError> {
    use std::io::Write as _;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(bytes)?;
    file.persist(path)
        .map_err(|error| SeleniumBaseError::Io(error.error))?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let mut hex = String::with_capacity(digest.as_ref().len() * 2);
    for byte in digest.as_ref() {
        // Writing to a String cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_lower_case_hex_sha256() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn bytes_that_are_not_png_are_refused_without_loading_models() {
        let error = image::load_from_memory_with_format(b"not a png", image::ImageFormat::Png);
        assert!(error.is_err());
    }
}
