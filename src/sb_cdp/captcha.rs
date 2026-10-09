//! Passing CAPTCHA widgets that ask for a click or a drag.

use std::fmt;
use std::time::Duration;

use super::{Page, Point, Rect};
use crate::error::SeleniumBaseError;

/// How far into a checkbox widget its box sits, in CSS pixels.
///
/// Every supported widget draws its checkbox against the left edge.
const CHECKBOX_INSET: f64 = 28.0;

/// How far from each end of a slider widget the handle starts and the drag
/// stops, in CSS pixels.
const SLIDER_INSET: f64 = 30.0;

/// The pause between moving the pointer onto a widget and pressing.
const SETTLE: Duration = Duration::from_millis(150);

/// A CAPTCHA widget that [`Page::solve_captcha`] knows how to attempt.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::Captcha;
///
/// assert_eq!(Captcha::Turnstile.to_string(), "Cloudflare Turnstile");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Captcha {
    /// Cloudflare Turnstile.
    Turnstile,
    /// Google reCAPTCHA.
    Recaptcha,
    /// hCaptcha.
    Hcaptcha,
    /// Friendly Captcha.
    FriendlyCaptcha,
    /// The `DataDome` slider.
    DataDome,
}

impl Captcha {
    /// Every supported widget, in the order they are looked for.
    pub const ALL: [Self; 5] = [
        Self::Turnstile,
        Self::Recaptcha,
        Self::Hcaptcha,
        Self::FriendlyCaptcha,
        Self::DataDome,
    ];

    /// Matches the widget's visible frame or container.
    fn selector(self) -> &'static str {
        match self {
            Self::Turnstile => r#"iframe[src*="challenges.cloudflare.com"], .cf-turnstile"#,
            Self::Recaptcha => {
                r#"iframe[src*="recaptcha/api2/anchor"], iframe[src*="recaptcha/enterprise/anchor"]"#
            }
            Self::Hcaptcha => r#"iframe[src*="hcaptcha.com"][src*="checkbox"]"#,
            Self::FriendlyCaptcha => r#".frc-captcha, iframe[src*="friendlycaptcha"]"#,
            Self::DataDome => r#"iframe[src*="captcha-delivery.com"]"#,
        }
    }

    /// How to operate a widget occupying `area` (in viewport coordinates).
    pub(crate) fn plan(self, area: Rect) -> Plan {
        let inset = match self {
            Self::DataDome => SLIDER_INSET,
            _ => CHECKBOX_INSET,
        };
        let y = area.y + area.height / 2.0;
        let press = Point {
            x: area.x + inset.min(area.width / 2.0),
            y,
        };
        let release = (self == Self::DataDome).then(|| Point {
            x: area.x + area.width - SLIDER_INSET.min(area.width / 2.0),
            y,
        });
        Plan { press, release }
    }

    /// A script that scrolls the first visible widget to the middle of the
    /// viewport and returns `{ kind, x, y, width, height }` for it, or `null`.
    ///
    /// The rectangle is in viewport coordinates, which is what mouse events
    /// use. `kind` is an index into [`ALL`](Self::ALL).
    pub(crate) fn locate_script() -> String {
        let selectors: Vec<&str> = Self::ALL.iter().map(|kind| kind.selector()).collect();
        let selectors = serde_json::to_string(&selectors).unwrap_or_else(|_| "[]".to_owned());
        format!(
            "(() => {{
              const selectors = {selectors};
              for (let kind = 0; kind < selectors.length; kind++) {{
                for (const el of document.querySelectorAll(selectors[kind])) {{
                  const style = getComputedStyle(el);
                  const r = el.getBoundingClientRect();
                  if (style.display === 'none' || style.visibility === 'hidden') continue;
                  if (r.width <= 0 || r.height <= 0) continue;
                  el.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }});
                  const b = el.getBoundingClientRect();
                  return {{ kind, x: b.left, y: b.top, width: b.width, height: b.height }};
                }}
              }}
              return null;
            }})()"
        )
    }

    /// Reads the answer of [`locate_script`](Self::locate_script).
    pub(crate) fn parse_located(found: &serde_json::Value) -> Option<(Self, Rect)> {
        let kind = Self::ALL.get(usize::try_from(found.get("kind")?.as_u64()?).ok()?)?;
        let number = |key: &str| found.get(key).and_then(serde_json::Value::as_f64);
        Some((
            *kind,
            Rect {
                x: number("x")?,
                y: number("y")?,
                width: number("width")?,
                height: number("height")?,
            },
        ))
    }
}

/// Where to press a widget and, for a slider, where to let go.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Plan {
    pub(crate) press: Point,
    pub(crate) release: Option<Point>,
}

impl fmt::Display for Captcha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Turnstile => "Cloudflare Turnstile",
            Self::Recaptcha => "reCAPTCHA",
            Self::Hcaptcha => "hCaptcha",
            Self::FriendlyCaptcha => "Friendly Captcha",
            Self::DataDome => "DataDome",
        })
    }
}

impl Page {
    /// Attempts the first supported CAPTCHA widget found on the page.
    ///
    /// Checkbox widgets are clicked and the `DataDome` slider is dragged, with
    /// trusted mouse events. The widgets live in cross-origin frames whose
    /// insides cannot be inspected, so the position pressed is where these
    /// widgets draw their control; whether the challenge is then accepted is
    /// up to the site, and a harder challenge may follow. Check the page
    /// afterwards.
    ///
    /// Returns the widget that was attempted, or `None` if the page shows no
    /// supported widget.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects the
    /// input.
    pub async fn solve_captcha(&self) -> Result<Option<Captcha>, SeleniumBaseError> {
        let found = self.evaluate(Captcha::locate_script()).await?;
        let Some((kind, area)) = Captcha::parse_located(&found) else {
            return Ok(None);
        };

        let plan = kind.plan(area);
        let mouse = self.mouse();
        mouse.move_to(plan.press).await?;
        tokio::time::sleep(SETTLE).await;
        match plan.release {
            Some(release) => mouse.drag(plan.press, release).await?,
            None => mouse.click(plan.press).await?,
        }
        Ok(Some(kind))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const AREA: Rect = Rect {
        x: 100.0,
        y: 200.0,
        width: 300.0,
        height: 65.0,
    };

    #[test]
    fn checkboxes_are_pressed_near_the_left_edge_at_mid_height() {
        let plan = Captcha::Turnstile.plan(AREA);
        assert_eq!(plan.press, Point { x: 128.0, y: 232.5 });
        assert_eq!(plan.release, None);
    }

    #[test]
    fn a_narrow_widget_is_pressed_no_further_than_its_middle() {
        let narrow = Rect {
            width: 20.0,
            ..AREA
        };
        assert_eq!(Captcha::Recaptcha.plan(narrow).press.x, 110.0);
    }

    #[test]
    fn the_slider_is_dragged_across_its_width() {
        let plan = Captcha::DataDome.plan(AREA);
        let release = plan.release.expect("a slider is dragged");
        assert_eq!(plan.press.x, 130.0);
        assert_eq!(release.x, 370.0);
        assert_eq!(plan.press.y, release.y);
    }

    #[test]
    fn every_widget_has_a_distinct_selector_and_name() {
        let selectors: std::collections::HashSet<_> =
            Captcha::ALL.iter().map(|kind| kind.selector()).collect();
        let names: std::collections::HashSet<_> =
            Captcha::ALL.iter().map(ToString::to_string).collect();
        assert_eq!(selectors.len(), Captcha::ALL.len());
        assert_eq!(names.len(), Captcha::ALL.len());
    }

    #[test]
    fn the_locate_script_carries_every_selector_as_data() {
        let script = Captcha::locate_script();
        for kind in Captcha::ALL {
            let quoted = serde_json::to_string(kind.selector()).unwrap();
            assert!(script.contains(&quoted), "{kind} is missing from {script}");
        }
    }

    #[test]
    fn a_located_widget_is_read_back_with_its_kind_and_box() {
        let found = json!({ "kind": 2, "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 });
        let (kind, area) = Captcha::parse_located(&found).expect("a well-formed answer");
        assert_eq!(kind, Captcha::Hcaptcha);
        assert_eq!(
            area,
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0
            }
        );
    }

    #[test]
    fn no_widget_and_nonsense_answers_read_as_none() {
        assert!(Captcha::parse_located(&json!(null)).is_none());
        assert!(Captcha::parse_located(
            &json!({ "kind": 99, "x": 0, "y": 0, "width": 1, "height": 1 })
        )
        .is_none());
        assert!(Captcha::parse_located(&json!({ "kind": 0 })).is_none());
    }
}
