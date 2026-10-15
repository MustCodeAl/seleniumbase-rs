//! Visual regression testing (feature `visual`): screenshots compared with a
//! saved baseline by structural similarity.
//!
//! A [`VisualBaseline`] keeps one PNG per check in a folder
//! (`visual_baseline/` unless you name another). The first
//! [`check`](VisualBaseline::check) of a name saves the screenshot as its
//! baseline; every later one compares the new screenshot with it and fails
//! when the two are less similar than the [threshold](VisualBaseline::with_threshold).
//! A failure writes `<name>.actual.png` (the new screenshot) and
//! `<name>.diff.png` (where they differ) next to the baseline, so you can look
//! at both and either fix the page or accept the change.
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//! use seleniumbase_rs::visual::VisualBaseline;
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let browser = Browser::launch(LaunchOptions::default()).await?;
//! let page = browser.default_page().await?;
//! page.goto("https://example.com").await?;
//!
//! let baselines = VisualBaseline::default(); // ./visual_baseline/
//! baselines.check_page("example_home", &page).await?; // saves it the first time
//! # Ok(())
//! # }
//! ```
//!
//! # The comparison
//!
//! Both images are laid over white (so a transparent pixel counts as white),
//! split into their red, green and blue channels, and each channel is compared
//! with the mean structural similarity index (MSSIM): SSIM (Wang, Bovik, Sheikh
//! and Simoncelli, 2004: luminance, contrast and structure, with the usual
//! constants K1 = 0.01 and K2 = 0.03) over 8 × 8 pixel windows, averaged by
//! area. The score is the lowest of the three channel means: 1.0 for identical
//! images, lower the more differs (SSIM can even go below 0 for inverted
//! structure). This is `rgb_similarity_structure` with
//! `Algorithm::MSSIMSimple` from the [image-compare] crate.
//!
//! [image-compare]: https://github.com/ChrisRega/image-compare
//!
//! # The threshold
//!
//! The default, [`DEFAULT_THRESHOLD`] (0.999), is strict on purpose. Chrome
//! renders the same page the same way on the same machine, so two screenshots
//! of an unchanged page score 1.0 and the threshold only has to absorb small
//! noise, such as a blinking caret or a few anti-aliased pixels, which covers
//! far less than 0.1 % of a page. Because the score is a mean over the whole
//! image, it falls by
//! roughly the share of the image that changed times how much it changed: 0.999
//! fails once about 0.1 % of the image is repainted outright (on a 1280 × 720
//! viewport, a 30 × 30 pixel block). Measured in headless Chrome on an
//! 800 × 600 viewport (`tests/visual_chrome.rs`): the unchanged page scores
//! 1.0, one word of 16 px text changed scores 0.99926 and passes, and a
//! rewritten 48 px heading scores 0.980 and fails. To catch changes as small
//! as one word, compare a screenshot of the element instead of the whole page,
//! or raise the threshold toward 1.0. Baselines taken on another machine,
//! operating system or Chrome version differ in font rendering across all the
//! text, so keep one set of baselines per platform rather than lowering the
//! threshold until they pass.
//!
//! # Sizes
//!
//! Images of different sizes are not compared: the check fails, says both
//! sizes, and saves the new screenshot as `<name>.actual.png`. They are not
//! scaled to fit, because a page that grew or shrank has changed (that is
//! usually what a visual test is for), and scaling blurs every edge so the
//! score would no longer mean what it says. Set a fixed window size for
//! visual tests.
//!
//! # Not `check_window`
//!
//! [`BaseCase::check_window`](crate::BaseCase::check_window) is the port of
//! Python SeleniumBase's `check_window`, which compares the page's *layout*
//! (the tag structure of the HTML, at three levels of detail) and only saves
//! the screenshot for a person to look at. It stays as it is. This module
//! compares *pixels*: it catches a changed colour, font, image or position that
//! leaves the HTML the same, and it does not notice an HTML change that looks
//! the same. Use both where both matter.

use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use image::{ImageFormat, Rgb, RgbImage, RgbaImage};
use image_compare::Algorithm;

use crate::artifacts::safe_stem;
use crate::error::SeleniumBaseError;
use crate::plugins::observer::PageEvidence;

/// The similarity below which a check fails unless
/// [`with_threshold`](VisualBaseline::with_threshold) sets another; see
/// [the module docs](self#the-threshold) for why 0.999.
pub const DEFAULT_THRESHOLD: f64 = 0.999;

/// The folder baselines are kept in unless [`VisualBaseline::new`] names
/// another, relative to the working directory.
pub const DEFAULT_DIR: &str = "visual_baseline";

/// How much a pixel's similarity has to fall (out of 255) before the diff
/// image paints it, so rounding in the similarity map does not show.
const DIFF_NOISE: u8 = 3;

/// A folder of baseline screenshots and the rules for comparing with them.
///
/// See [the module docs](self) for how images are compared.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualBaseline {
    dir: PathBuf,
    threshold: f64,
    update: bool,
}

impl Default for VisualBaseline {
    /// Baselines in [`DEFAULT_DIR`] with the [`DEFAULT_THRESHOLD`].
    fn default() -> Self {
        Self::new(DEFAULT_DIR)
    }
}

/// What a successful check did.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum VisualOutcome {
    /// There was no baseline, so the screenshot was saved as the baseline.
    Created {
        /// Where the baseline was written.
        baseline: PathBuf,
    },
    /// The baseline was replaced, because baselines are being
    /// [updated](VisualBaseline::update_baselines).
    Updated {
        /// Where the baseline was written.
        baseline: PathBuf,
    },
    /// The screenshot matched the baseline.
    Matched {
        /// The similarity, at or above the threshold (1.0 is identical).
        score: f64,
    },
}

/// The files of one check.
struct CheckFiles {
    baseline: PathBuf,
    actual: PathBuf,
    diff: PathBuf,
}

impl VisualBaseline {
    /// Baselines kept in `dir`, compared with the [`DEFAULT_THRESHOLD`].
    ///
    /// The folder is created when the first baseline is saved.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            threshold: DEFAULT_THRESHOLD,
            update: false,
        }
    }

    /// Fails a check when the similarity is below `threshold`, a number from
    /// 0.0 to 1.0 (1.0 accepts only identical images). A value outside that
    /// range makes every check fail with
    /// [`SeleniumBaseError::InvalidConfig`].
    #[must_use]
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// With `true`, every check saves its screenshot as the new baseline
    /// instead of comparing with the old one; use it once after an intended
    /// change of the page.
    #[must_use]
    pub fn update_baselines(mut self, update: bool) -> Self {
        self.update = update;
        self
    }

    /// The folder the baselines are kept in.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The similarity below which a check fails.
    #[must_use]
    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    /// Where the baseline of the check `name` is kept: `<dir>/<name>.png`.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] if `name` is not 1 to 80 ASCII
    /// letters, digits, `-` or `_`.
    pub fn baseline_path(&self, name: &str) -> Result<PathBuf, SeleniumBaseError> {
        Ok(self.files(name)?.baseline)
    }

    /// Compares the PNG image `png` with the baseline saved as `name`, or
    /// saves it as that baseline when there is none yet (or when baselines
    /// are being [updated](Self::update_baselines)).
    ///
    /// On a mismatch the image is saved as `<name>.actual.png` and a picture
    /// of the differences as `<name>.diff.png`: the new image dimmed to grey,
    /// with what changed painted red, brighter where it changed more. A match
    /// removes the two files a previous failure left.
    ///
    /// # Errors
    ///
    /// - [`SeleniumBaseError::AssertionFailed`] when the image differs from
    ///   the baseline (the message gives the score, the threshold and the
    ///   files) or has another size (the message gives both sizes).
    /// - [`SeleniumBaseError::InvalidConfig`] for a `name` that is not 1 to 80
    ///   ASCII letters, digits, `-` or `_`, or a threshold outside 0.0 to 1.0.
    /// - [`SeleniumBaseError::Screenshot`] when `png` or the baseline file is
    ///   not a PNG image.
    /// - [`SeleniumBaseError::Io`] when a file cannot be read or written.
    pub fn check(&self, name: &str, png: &[u8]) -> Result<VisualOutcome, SeleniumBaseError> {
        let threshold = self.valid_threshold()?;
        let files = self.files(name)?;
        let actual = decode(png, "the screenshot")?;

        let existed = files
            .baseline
            .try_exists()
            .map_err(|error| with_path(&error, "look for", &files.baseline))?;
        if self.update || !existed {
            create_dir(&self.dir)?;
            write(&files.baseline, png)?;
            remove_stale(&files)?;
            return Ok(if existed {
                VisualOutcome::Updated {
                    baseline: files.baseline,
                }
            } else {
                VisualOutcome::Created {
                    baseline: files.baseline,
                }
            });
        }

        let stored = fs::read(&files.baseline)
            .map_err(|error| with_path(&error, "read", &files.baseline))?;
        let baseline = decode(&stored, &files.baseline.display().to_string())?;
        if baseline.dimensions() != actual.dimensions() {
            write(&files.actual, png)?;
            remove_file_if_present(&files.diff)?;
            let (width, height) = actual.dimensions();
            let (base_width, base_height) = baseline.dimensions();
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "visual check {name:?}: the screenshot is {width}x{height} pixels but the \
                 baseline {} is {base_width}x{base_height}, so nothing was compared; the \
                 screenshot is saved as {}. Fix the window size, or update the baseline if \
                 the new size is intended",
                files.baseline.display(),
                files.actual.display(),
            )));
        }

        let comparison = compare(&baseline, &actual)?;
        if comparison.score >= threshold {
            remove_stale(&files)?;
            return Ok(VisualOutcome::Matched {
                score: comparison.score,
            });
        }
        write(&files.actual, png)?;
        write(&files.diff, &encode(&comparison.diff)?)?;
        Err(SeleniumBaseError::AssertionFailed(format!(
            "visual check {name:?}: similarity {:.5} is below the threshold {threshold}; \
             baseline {}, screenshot {}, differences {}",
            comparison.score,
            files.baseline.display(),
            files.actual.display(),
            files.diff.display(),
        )))
    }

    /// Takes a screenshot of `page` (a [`BaseCase`](crate::BaseCase) or an
    /// [`sb_cdp::Page`](crate::sb_cdp::Page)) and [`check`](Self::check)s it
    /// as `name`. The comparison runs on a blocking thread.
    ///
    /// # Errors
    ///
    /// The screenshot's error, or [`check`](Self::check)'s.
    pub async fn check_page(
        &self,
        name: &str,
        page: &dyn PageEvidence,
    ) -> Result<VisualOutcome, SeleniumBaseError> {
        // A bad name or threshold fails before the browser is asked for anything.
        self.valid_threshold()?;
        self.files(name)?;
        let png = page.screenshot_png().await?;
        let baselines = self.clone();
        let name = name.to_owned();
        tokio::task::spawn_blocking(move || baselines.check(&name, &png))
            .await
            .map_err(|error| {
                SeleniumBaseError::screenshot(format!("the visual comparison stopped: {error}"))
            })?
    }

    fn valid_threshold(&self) -> Result<f64, SeleniumBaseError> {
        if (0.0..=1.0).contains(&self.threshold) {
            Ok(self.threshold)
        } else {
            Err(SeleniumBaseError::InvalidConfig(format!(
                "a visual threshold must be between 0.0 and 1.0, not {}",
                self.threshold
            )))
        }
    }

    fn files(&self, name: &str) -> Result<CheckFiles, SeleniumBaseError> {
        if name.is_empty() || safe_stem(name, "") != name {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "a visual baseline name must be 1 to 80 ASCII letters, digits, '-' or '_', \
                 not {name:?}"
            )));
        }
        Ok(CheckFiles {
            baseline: self.dir.join(format!("{name}.png")),
            actual: self.dir.join(format!("{name}.actual.png")),
            diff: self.dir.join(format!("{name}.diff.png")),
        })
    }
}

/// The result of comparing two images of the same size.
struct Comparison {
    score: f64,
    diff: RgbImage,
}

/// Compares two images of the same size; see the module docs.
fn compare(baseline: &RgbaImage, actual: &RgbaImage) -> Result<Comparison, SeleniumBaseError> {
    let baseline = over_white(baseline);
    let actual = over_white(actual);
    let similarity =
        image_compare::rgb_similarity_structure(&Algorithm::MSSIMSimple, &baseline, &actual)
            .map_err(|error| {
                SeleniumBaseError::screenshot(format!("the images cannot be compared: {error}"))
            })?;
    // Each channel of the map is that channel's similarity, 255 where it is
    // the same and darker where it differs.
    let map = similarity.image.to_color_map().into_rgb8();
    Ok(Comparison {
        score: similarity.score,
        diff: diff_image(&actual, &map),
    })
}

/// `image` laid over a white background.
fn over_white(image: &RgbaImage) -> RgbImage {
    let blend = |channel: u8, alpha: u8| {
        let (channel, alpha) = (u32::from(channel), u32::from(alpha));
        let mixed = (channel * alpha + 255 * (255 - alpha) + 127) / 255;
        u8::try_from(mixed).unwrap_or(u8::MAX)
    };
    RgbImage::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b, a] = image.get_pixel(x, y).0;
        Rgb([blend(r, a), blend(g, a), blend(b, a)])
    })
}

/// `actual` dimmed to grey, with red painted where `map` shows a difference.
fn diff_image(actual: &RgbImage, map: &RgbImage) -> RgbImage {
    RgbImage::from_fn(actual.width(), actual.height(), |x, y| {
        let [r, g, b] = actual.get_pixel(x, y).0;
        let luma = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
        let grey = 40.0 + luma * 0.3;
        let [mr, mg, mb] = map.get_pixel(x, y).0;
        let change = u8::MAX - mr.min(mg).min(mb);
        if change < DIFF_NOISE {
            let grey = to_channel(grey);
            return Rgb([grey, grey, grey]);
        }
        // Any real difference is clearly visible; a bigger one is brighter.
        let strength = (f32::from(change) / 255.0).max(0.5);
        Rgb([
            to_channel(grey + (255.0 - grey) * strength),
            to_channel(grey * (1.0 - strength)),
            to_channel(grey * (1.0 - strength)),
        ])
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to 0..=255 first"
)]
fn to_channel(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

/// `png` decoded, or a [`SeleniumBaseError::Screenshot`] naming `what`.
fn decode(png: &[u8], what: &str) -> Result<RgbaImage, SeleniumBaseError> {
    let image = image::load_from_memory_with_format(png, ImageFormat::Png)
        .map_err(|error| SeleniumBaseError::screenshot(format!("{what} is not a PNG: {error}")))?
        .into_rgba8();
    if image.width() == 0 || image.height() == 0 {
        return Err(SeleniumBaseError::screenshot(format!(
            "{what} is an empty image"
        )));
    }
    Ok(image)
}

fn encode(image: &RgbImage) -> Result<Vec<u8>, SeleniumBaseError> {
    let mut png = Cursor::new(Vec::new());
    image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|error| SeleniumBaseError::screenshot(format!("cannot encode a PNG: {error}")))?;
    Ok(png.into_inner())
}

fn with_path(error: &io::Error, what: &str, path: &Path) -> SeleniumBaseError {
    SeleniumBaseError::Io(io::Error::new(
        error.kind(),
        format!("cannot {what} {}: {error}", path.display()),
    ))
}

fn create_dir(dir: &Path) -> Result<(), SeleniumBaseError> {
    fs::create_dir_all(dir).map_err(|error| with_path(&error, "create the folder", dir))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), SeleniumBaseError> {
    fs::write(path, bytes).map_err(|error| with_path(&error, "write", path))
}

fn remove_file_if_present(path: &Path) -> Result<(), SeleniumBaseError> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(with_path(&error, "remove", path))
        }
        _ => Ok(()),
    }
}

/// Removes the files a previous failed check of the same name left.
fn remove_stale(files: &CheckFiles) -> Result<(), SeleniumBaseError> {
    remove_file_if_present(&files.actual)?;
    remove_file_if_present(&files.diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;
    use image::Rgba;

    /// A made-up page: a vertical gradient, a dark "button" and a few
    /// stripes of "text", so every channel has structure to compare.
    fn page_image(width: u32, height: u32) -> RgbImage {
        RgbImage::from_fn(width, height, |x, y| {
            let shade = u8::try_from(200 + (y * 40 / height)).unwrap();
            if (10..50).contains(&x) && (10..26).contains(&y) {
                Rgb([20, 60, 160])
            } else if y % 10 < 2 && x > 60 && x < width - 10 {
                Rgb([30, 30, 30])
            } else {
                Rgb([shade, shade, 250])
            }
        })
    }

    fn png(image: &RgbImage) -> Vec<u8> {
        encode(image).unwrap()
    }

    fn baselines(dir: &Path) -> VisualBaseline {
        VisualBaseline::new(dir.join("baselines"))
    }

    fn assertion_message(error: SeleniumBaseError) -> String {
        match error {
            SeleniumBaseError::AssertionFailed(message) => message,
            other => panic!("expected an assertion failure, got {other:?}"),
        }
    }

    #[test]
    fn the_first_check_saves_the_screenshot_as_the_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let shot = png(&page_image(120, 80));

        let outcome = baselines(dir.path()).check("home", &shot).unwrap();

        let saved = dir.path().join("baselines").join("home.png");
        assert_eq!(
            outcome,
            VisualOutcome::Created {
                baseline: saved.clone()
            }
        );
        assert_eq!(fs::read(saved).unwrap(), shot, "saved byte for byte");
    }

    #[test]
    fn an_identical_screenshot_matches_with_a_score_of_one() {
        let dir = tempfile::tempdir().unwrap();
        let checks = baselines(dir.path());
        let shot = png(&page_image(120, 80));
        checks.check("home", &shot).unwrap();

        let outcome = checks.check("home", &shot).unwrap();

        let VisualOutcome::Matched { score } = outcome else {
            panic!("expected a match, got {outcome:?}");
        };
        assert!((score - 1.0).abs() < 1e-9, "score {score}");
    }

    #[test]
    fn a_few_slightly_changed_pixels_still_match() {
        let dir = tempfile::tempdir().unwrap();
        let checks = baselines(dir.path());
        let original = page_image(120, 80);
        checks.check("home", &png(&original)).unwrap();

        // Anti-aliasing noise: three pixels off by a few levels.
        let mut noisy = original.clone();
        for (x, y) in [(70, 30), (71, 30), (100, 55)] {
            let Rgb([r, g, b]) = *noisy.get_pixel(x, y);
            noisy.put_pixel(x, y, Rgb([r.saturating_sub(4), g.saturating_add(3), b]));
        }

        let outcome = checks.check("home", &png(&noisy)).unwrap();

        let VisualOutcome::Matched { score } = outcome else {
            panic!("expected a match, got {outcome:?}");
        };
        assert!(
            (DEFAULT_THRESHOLD..1.0).contains(&score),
            "a small difference lowers the score a little: {score}"
        );
    }

    #[test]
    fn a_very_different_screenshot_fails_and_leaves_the_actual_and_diff_images() {
        let dir = tempfile::tempdir().unwrap();
        let checks = baselines(dir.path());
        let original = page_image(120, 80);
        checks.check("home", &png(&original)).unwrap();

        // A banner painted over the lower half.
        let mut changed = original.clone();
        for y in 40..80 {
            for x in 0..120 {
                changed.put_pixel(x, y, Rgb([220, 40, 40]));
            }
        }
        let shot = png(&changed);

        let message = assertion_message(checks.check("home", &shot).unwrap_err());

        let folder = dir.path().join("baselines");
        let actual = folder.join("home.actual.png");
        let diff = folder.join("home.diff.png");
        assert!(message.contains("similarity 0."), "{message}");
        assert!(message.contains("threshold 0.999"), "{message}");
        assert!(message.contains(&actual.display().to_string()), "{message}");
        assert!(message.contains(&diff.display().to_string()), "{message}");
        assert_eq!(fs::read(&actual).unwrap(), shot);
        assert_eq!(
            fs::read(folder.join("home.png")).unwrap(),
            png(&original),
            "the baseline is kept"
        );

        // The diff highlights the banner in red and leaves the rest grey.
        let diff = decode(&fs::read(diff).unwrap(), "the diff").unwrap();
        assert_eq!(diff.dimensions(), (120, 80));
        let Rgba([r, g, b, _]) = *diff.get_pixel(60, 60);
        assert!(
            r > 200 && g < 60 && b < 60,
            "changed area is red: {r},{g},{b}"
        );
        let Rgba([r, g, b, _]) = *diff.get_pixel(5, 5);
        assert!(r == g && g == b, "unchanged area is grey: {r},{g},{b}");
    }

    #[test]
    fn a_screenshot_of_another_size_fails_without_a_comparison() {
        let dir = tempfile::tempdir().unwrap();
        let checks = baselines(dir.path());
        checks.check("home", &png(&page_image(120, 80))).unwrap();
        let smaller = png(&page_image(100, 80));

        let message = assertion_message(checks.check("home", &smaller).unwrap_err());

        assert!(message.contains("100x80"), "{message}");
        assert!(message.contains("120x80"), "{message}");
        assert!(message.contains("nothing was compared"), "{message}");
        let folder = dir.path().join("baselines");
        assert_eq!(fs::read(folder.join("home.actual.png")).unwrap(), smaller);
        assert!(!folder.join("home.diff.png").exists());
    }

    #[test]
    fn a_match_removes_what_an_earlier_failure_left() {
        let dir = tempfile::tempdir().unwrap();
        let checks = baselines(dir.path());
        let original = png(&page_image(120, 80));
        checks.check("home", &original).unwrap();
        let inverted = png(&RgbImage::from_fn(120, 80, |x, y| {
            let Rgb([r, g, b]) = *page_image(120, 80).get_pixel(x, y);
            Rgb([255 - r, 255 - g, 255 - b])
        }));
        checks.check("home", &inverted).unwrap_err();

        checks.check("home", &original).unwrap();

        let folder = dir.path().join("baselines");
        assert!(!folder.join("home.actual.png").exists());
        assert!(!folder.join("home.diff.png").exists());
    }

    #[test]
    fn updating_baselines_replaces_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let first = png(&page_image(120, 80));
        let second = png(&page_image(100, 60));
        baselines(dir.path()).check("home", &first).unwrap();

        let outcome = baselines(dir.path())
            .update_baselines(true)
            .check("home", &second)
            .unwrap();

        let saved = dir.path().join("baselines").join("home.png");
        assert_eq!(
            outcome,
            VisualOutcome::Updated {
                baseline: saved.clone()
            }
        );
        assert_eq!(fs::read(saved).unwrap(), second);
    }

    #[test]
    fn a_lower_threshold_accepts_a_bigger_difference() {
        let dir = tempfile::tempdir().unwrap();
        let original = page_image(120, 80);
        let mut changed = original.clone();
        for y in 60..80 {
            for x in 0..120 {
                changed.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
        baselines(dir.path())
            .check("home", &png(&original))
            .unwrap();

        assert!(baselines(dir.path()).check("home", &png(&changed)).is_err());
        let lenient = baselines(dir.path()).with_threshold(0.5);
        assert!(matches!(
            lenient.check("home", &png(&changed)),
            Ok(VisualOutcome::Matched { .. })
        ));
    }

    #[test]
    fn transparent_pixels_count_as_white() {
        let dir = tempfile::tempdir().unwrap();
        let checks = baselines(dir.path());
        let white = png(&RgbImage::from_pixel(16, 16, Rgb([255, 255, 255])));
        checks.check("blank", &white).unwrap();

        let mut transparent = Cursor::new(Vec::new());
        RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0]))
            .write_to(&mut transparent, ImageFormat::Png)
            .unwrap();

        assert!(matches!(
            checks.check("blank", transparent.get_ref()),
            Ok(VisualOutcome::Matched { .. })
        ));
    }

    #[test]
    fn a_name_that_is_not_plain_is_refused_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let shot = png(&page_image(20, 20));
        let too_long = "x".repeat(81);
        for name in ["", "../escape", "a/b", "home.png", "with space", &too_long] {
            let error = baselines(dir.path()).check(name, &shot).unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig(_)),
                "{name:?}: {error:?}"
            );
        }
        assert!(!dir.path().join("baselines").exists());
        assert!(!dir.path().join("escape.png").exists());
    }

    #[test]
    fn a_threshold_outside_zero_to_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let shot = png(&page_image(20, 20));
        for threshold in [-0.1, 1.5, f64::NAN] {
            let error = baselines(dir.path())
                .with_threshold(threshold)
                .check("home", &shot)
                .unwrap_err();
            assert!(
                matches!(error, SeleniumBaseError::InvalidConfig(_)),
                "{threshold}: {error:?}"
            );
        }
    }

    #[test]
    fn bytes_that_are_not_a_png_are_a_screenshot_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = baselines(dir.path())
            .check("home", b"not a png")
            .unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::Screenshot(_)),
            "{error:?}"
        );
    }

    #[test]
    fn the_defaults_are_the_documented_folder_and_threshold() {
        let checks = VisualBaseline::default();
        assert_eq!(checks.dir(), Path::new("visual_baseline"));
        assert!((checks.threshold() - 0.999).abs() < f64::EPSILON);
        assert_eq!(
            checks.baseline_path("home").unwrap(),
            Path::new("visual_baseline").join("home.png")
        );
    }

    /// A page whose screenshot is a fixed image.
    struct Painted(Vec<u8>);

    #[async_trait]
    impl PageEvidence for Painted {
        async fn screenshot_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
            Ok(self.0.clone())
        }

        async fn page_source(&self) -> Result<String, SeleniumBaseError> {
            Ok(String::new())
        }
    }

    #[tokio::test]
    async fn check_page_compares_the_page_screenshot() {
        let dir = tempfile::tempdir().unwrap();
        let checks = baselines(dir.path());
        let page = Painted(png(&page_image(120, 80)));

        let first = checks.check_page("home", &page).await.unwrap();
        let second = checks.check_page("home", &page).await.unwrap();

        assert!(matches!(first, VisualOutcome::Created { .. }), "{first:?}");
        assert!(
            matches!(second, VisualOutcome::Matched { .. }),
            "{second:?}"
        );
        let refused = checks.check_page("../x", &page).await.unwrap_err();
        assert!(matches!(refused, SeleniumBaseError::InvalidConfig(_)));
    }
}
