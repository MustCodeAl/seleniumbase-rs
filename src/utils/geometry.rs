//! Screen-coordinate geometry shared by the WebDriver and Pure CDP engines.

use crate::sb_cdp::Rect;

/// A script returning `[screenX, screenY, chromeWidth, chromeHeight, scrollX,
/// scrollY]`: where the window is on the screen, how much of its outer size is
/// browser chrome rather than page, and how far the page is scrolled.
pub(crate) const WINDOW_METRICS_SCRIPT: &str = "[window.screenX, window.screenY, \
    window.outerWidth - window.innerWidth, window.outerHeight - window.innerHeight, \
    window.scrollX, window.scrollY]";

/// An element's rectangle on the screen, given its rectangle in the document
/// and the window measurements from [`WINDOW_METRICS_SCRIPT`].
///
/// The browser's toolbar sits above the page and the window frame is split
/// evenly on both sides, so the page origin is the window origin plus half the
/// horizontal chrome and all of the vertical chrome.
pub(crate) fn screen_rect(element: Rect, metrics: [f64; 6]) -> Rect {
    let [window_x, window_y, chrome_width, chrome_height, scroll_x, scroll_y] = metrics;
    Rect {
        x: window_x + chrome_width / 2.0 + element.x - scroll_x,
        y: window_y + chrome_height + element.y - scroll_y,
        ..element
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_rect_adds_the_window_origin_and_toolbar_and_removes_the_scroll() {
        let element = Rect {
            x: 100.0,
            y: 500.0,
            width: 40.0,
            height: 20.0,
        };
        // Window at (10, 20); 16px of side frame in total, 80px of toolbar;
        // scrolled 300px down.
        let screen = screen_rect(element, [10.0, 20.0, 16.0, 80.0, 0.0, 300.0]);
        assert_eq!(
            screen,
            Rect {
                x: 118.0,
                y: 300.0,
                width: 40.0,
                height: 20.0
            }
        );
    }

    #[test]
    fn with_no_chrome_and_no_scroll_the_document_position_is_the_screen_position() {
        let element = Rect {
            x: 8.0,
            y: 8.0,
            width: 100.0,
            height: 30.0,
        };
        assert_eq!(screen_rect(element, [0.0; 6]), element);
    }

    #[test]
    fn a_window_on_a_monitor_to_the_left_has_a_negative_origin() {
        let element = Rect {
            x: 10.0,
            y: 10.0,
            width: 5.0,
            height: 5.0,
        };
        let screen = screen_rect(element, [-1920.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!((screen.x, screen.y), (-1910.0, 10.0));
        assert_eq!((screen.width, screen.height), (5.0, 5.0));
    }

    #[test]
    fn a_headless_window_has_no_chrome_so_only_its_origin_and_scroll_count() {
        // Headless Chrome reports equal outer and inner sizes.
        let element = Rect {
            x: 30.0,
            y: 60.5,
            width: 1.0,
            height: 2.0,
        };
        let screen = screen_rect(element, [0.0, 0.0, 0.0, 0.0, 0.0, 0.5]);
        assert_eq!((screen.x, screen.y), (30.0, 60.0));
    }

    #[test]
    fn the_metrics_script_is_a_six_element_array_of_window_properties() {
        let script = WINDOW_METRICS_SCRIPT;

        assert!(script.starts_with('[') && script.ends_with(']'), "{script}");
        assert_eq!(script.matches("window.").count(), 8, "{script}");
        for property in ["screenX", "screenY", "scrollX", "scrollY"] {
            assert!(script.contains(&format!("window.{property}")), "{property}");
        }
        // Chrome is the outer size minus the inner size, width then height.
        let chrome_width = script.find("outerWidth - window.innerWidth").unwrap();
        let chrome_height = script.find("outerHeight - window.innerHeight").unwrap();
        assert!(chrome_width < chrome_height);
        assert_eq!(script.split(',').count(), 6, "{script}");
    }

    #[test]
    fn scrolling_moves_the_element_up_and_left_on_the_screen() {
        let element = Rect {
            x: 50.0,
            y: 400.0,
            width: 10.0,
            height: 10.0,
        };
        let screen = screen_rect(element, [0.0, 0.0, 0.0, 0.0, 20.0, 150.0]);
        assert_eq!((screen.x, screen.y), (30.0, 250.0));
    }
}
