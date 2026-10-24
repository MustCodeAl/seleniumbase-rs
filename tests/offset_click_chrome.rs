//! `BaseCase::click_with_offset` against a real Chrome driven through
//! chromedriver: where the page sees the click.
//!
//! WebDriver counts an offset from the centre of the element, Python's
//! `click_with_offset` from its top-left corner (unless `center`), and the
//! method converts. A mock cannot say which one the driver really means, so
//! this does. It needs Chrome and chromedriver, so it is ignored by default:
//!
//! ```text
//! cargo test --test offset_click_chrome -- --ignored
//! ```

use seleniumbase_rs::{BaseCase, BrowserConfig};

/// A 200 x 100 box at (50, 50) that writes where it was clicked, relative to
/// its own top-left corner, into `#out`. The `%` and `#` characters a data URL
/// cannot hold are left out of it.
const PAGE: &str = "data:text/html,<html><body style=\"margin:0\">\
    <div id=box style=\"position:absolute;left:50px;top:50px;width:200px;height:100px;background:silver\" \
    onclick=\"document.getElementById('out').textContent=e(event)\"></div>\
    <div id=out style=\"position:absolute;left:300px;top:300px\">none</div>\
    <script>function e(ev){return Math.round(ev.offsetX)+','+Math.round(ev.offsetY)}</script>\
    </body></html>";

async fn case() -> BaseCase {
    let config = BrowserConfig::default().with_headless(true);
    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.open(PAGE).await.expect("the page opens");
    sb
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn an_offset_counts_from_the_top_left_corner_unless_it_says_centre() {
    let mut sb = case().await;

    sb.click_with_offset("#box", 10, 5).await.unwrap();
    assert_eq!(sb.get_text("#out").await.unwrap(), "10,5");

    // From the centre of a 200 x 100 box, (0, 0) is (100, 50) in the box.
    sb.click_with_offset("#box", 0, 0)
        .center(true)
        .await
        .unwrap();
    assert_eq!(sb.get_text("#out").await.unwrap(), "100,50");
    sb.click_with_offset("#box", -20, 10)
        .center(true)
        .await
        .unwrap();
    assert_eq!(sb.get_text("#out").await.unwrap(), "80,60");

    sb.double_click_with_offset("#box", 30, 40).await.unwrap();
    assert_eq!(sb.get_text("#out").await.unwrap(), "30,40");

    sb.quit().await.unwrap();
}
