//! The helpers that take Python's keyword arguments as call builders, against
//! a real Chrome driven through chromedriver: a drag by an offset, the
//! 1-based `click_nth_visible_element`, the arrow keys with `selector` and
//! `times`, `scroll_down` and `scroll_up` by a percentage, the hover-and-click
//! pair, `wait_for_attribute` with a value, `open_new_tab` that stays where it
//! is, `add_cookie` with a cookie object, and the HTTP `download_file` and
//! `save_file_as`.
//!
//! They need Chrome and chromedriver, so they are ignored by default. Run them
//! one at a time:
//!
//! ```text
//! cargo test --test parity_signatures_chrome -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};
use serde_json::json;

/// A grey 60 x 40 box that follows the mouse while it is held down.
const DRAG_PAGE: &str = "data:text/html,<html><body style=\"margin:0\">\
    <div id=box style=\"position:absolute;left:50px;top:50px;width:60px;height:40px;background:silver\"></div>\
    <script>var b=document.getElementById('box'),s=null;\
    b.onmousedown=function(e){s={x:e.clientX,y:e.clientY,l:b.offsetLeft,t:b.offsetTop}};\
    document.onmousemove=function(e){if(s){b.style.left=(s.l+e.clientX-s.x)+'px';b.style.top=(s.t+e.clientY-s.y)+'px'}};\
    document.onmouseup=function(){s=null}</script></body></html>";

/// Three buttons, the second not displayed, each naming itself in the title.
const NTH_PAGE: &str = "data:text/html,<html><head><title>none</title></head><body>\
    <button onclick=\"document.title='one'\">1</button>\
    <button style=\"display:none\" onclick=\"document.title='two'\">2</button>\
    <button onclick=\"document.title='three'\">3</button></body></html>";

/// A body that writes the keys it gets into the title.
const KEYS_PAGE: &str = "data:text/html,<html><head><title></title></head>\
    <body style=\"height:200px\" onkeydown=\"document.title+=event.key+'|'\">\
    <input id=field onkeydown=\"event.stopPropagation();document.title+='field:'+event.key+'|'\"></body></html>";

/// A menu whose item shows on hover and says how it was clicked in the title.
const HOVER_PAGE: &str = "data:text/html,<html><head><title>none</title>\
    <style>.item{display:none}.menu:hover .item{display:block}</style></head><body>\
    <div class=menu style=\"width:100px;height:40px;background:silver\">menu\
    <div class=item style=\"height:30px;background:gold\" \
    onclick=\"document.title='clicked'\" ondblclick=\"document.title='double'\">item</div></div>\
    </body></html>";

/// An element that gains `data-ready` and loses `data-state` after 400 ms.
const ATTRIBUTE_PAGE: &str = "data:text/html,<html><body><div id=t data-state=on></div><script>\
    setTimeout(function(){var t=document.getElementById('t');\
    t.setAttribute('data-ready','yes');t.removeAttribute('data-state')},400)</script></body></html>";

/// A page 5000 pixels high.
const TALL_PAGE: &str =
    "data:text/html,<html><body style=\"margin:0\"><div style=\"height:5000px\"></div></body></html>";

async fn case(page: &str) -> BaseCase {
    let config = BrowserConfig::default().with_headless(true);
    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.open(page).await.expect("the page opens");
    sb
}

/// What `script` returns, as text.
async fn js(sb: &BaseCase, script: &str) -> String {
    let value = sb.execute_script(script).await.expect("the script runs");
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_drag_by_an_offset_moves_the_element() {
    let mut sb = case(DRAG_PAGE).await;

    sb.drag_and_drop_with_offset("#box", 40, 20).await.unwrap();

    assert_eq!(
        js(&sb, "return b.offsetLeft+','+b.offsetTop").await,
        "90,70"
    );
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn the_nth_visible_element_counts_from_one() {
    let mut sb = case(NTH_PAGE).await;

    let error = sb.click_nth_visible_element("button", 0).await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "{error:?}"
    );
    assert_eq!(js(&sb, "return document.title").await, "none");

    // The hidden second button does not count.
    sb.click_nth_visible_element("button", 2).await.unwrap();
    assert_eq!(js(&sb, "return document.title").await, "three");
    sb.click_nth_visible_element("button", 1)
        .scroll(false)
        .await
        .unwrap();
    assert_eq!(js(&sb, "return document.title").await, "one");

    let error = sb.click_nth_visible_element("button", 3).await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::AssertionFailed(_)),
        "{error:?}"
    );
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn an_arrow_key_goes_to_an_element_as_often_as_asked() {
    let mut sb = case(KEYS_PAGE).await;

    sb.press_down_arrow().times(3).await.unwrap();
    sb.press_up_arrow().await.unwrap();
    sb.press_left_arrow()
        .selector("#field")
        .times(2)
        .await
        .unwrap();
    sb.press_right_arrow().times(0).await.unwrap();

    let title = js(&sb, "return document.title.trim()").await;
    let keys: Vec<&str> = title.trim_end_matches('|').split('|').collect();
    assert_eq!(
        keys,
        [
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
            "ArrowUp",
            "field:ArrowLeft",
            "field:ArrowLeft"
        ]
    );
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_scroll_moves_a_percentage_of_the_window_height() {
    let mut sb = case(TALL_PAGE).await;
    let (_, height) = sb.get_window_size().await.unwrap();
    let height = f64::from(height);

    sb.scroll_down().await.unwrap();
    let y: f64 = js(&sb, "return window.scrollY").await.parse().unwrap();
    assert!(
        (y - height * 0.25).abs() <= 1.0,
        "{y} after 25 percent of {height}"
    );

    sb.scroll_down().amount(50.0).await.unwrap();
    sb.scroll_up().amount(25.0).await.unwrap();
    let y: f64 = js(&sb, "return window.scrollY").await.parse().unwrap();
    assert!(
        (y - height * 0.5).abs() <= 1.5,
        "{y} after +25, +50, -25 percent of {height}"
    );
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn the_hover_and_click_pair_takes_the_hover_and_the_click_selector() {
    let mut sb = case(HOVER_PAGE).await;

    sb.hover_and_double_click(".menu", ".item")
        .timeout(Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(js(&sb, "return document.title").await, "double");

    sb.open(HOVER_PAGE).await.unwrap();
    sb.hover_and_js_click(".menu", ".item").await.unwrap();
    assert_eq!(js(&sb, "return document.title").await, "clicked");
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn waiting_for_an_attribute_can_ask_for_its_value() {
    let mut sb = case(ATTRIBUTE_PAGE).await;

    let wrong = sb
        .wait_for_attribute("#t", "data-ready")
        .value("no")
        .timeout(Duration::from_secs(1))
        .await;
    assert!(
        matches!(wrong, Err(SeleniumBaseError::WaitTimeout(_))),
        "{wrong:?}"
    );

    sb.wait_for_attribute("#t", "data-ready")
        .value("yes")
        .timeout(Duration::from_secs(5))
        .await
        .unwrap();
    sb.wait_for_attribute_not_present("#t", "data-state")
        .timeout(Duration::from_secs(5))
        .await
        .unwrap();
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_new_tab_stays_in_the_background_when_switch_to_is_false() {
    let mut sb = case(TALL_PAGE).await;
    let first = sb.get_current_window_handle().await.unwrap();

    sb.open_new_tab().switch_to(false).await.unwrap();
    assert_eq!(sb.get_current_window_handle().await.unwrap(), first);

    sb.open_new_window().await.unwrap();
    assert_ne!(sb.get_current_window_handle().await.unwrap(), first);
    sb.quit().await.unwrap();
}

/// Serves a page at `/` and a small file at `/files/report.csv`, and keeps the
/// head of the request for the file.
fn serve(heads: Arc<Mutex<Vec<String>>>) -> String {
    fn answer(mut stream: TcpStream, heads: &Mutex<Vec<String>>) {
        let mut buffer = [0_u8; 8192];
        let read = stream.read(&mut buffer).unwrap_or(0);
        let head = String::from_utf8_lossy(&buffer[..read]).into_owned();
        let (kind, body) = if head.starts_with("GET /files/report.csv") {
            heads.lock().unwrap().push(head);
            ("text/csv", "id,name\n1,Ada\n")
        } else {
            ("text/html", "<html><body>cookies</body></html>")
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    }
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let base = format!("http://{}", listener.local_addr().expect("bound address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let heads = Arc::clone(&heads);
            std::thread::spawn(move || answer(stream, &heads));
        }
    });
    base
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn a_cookie_object_is_added_and_a_bad_one_is_refused() {
    let heads = Arc::new(Mutex::new(Vec::new()));
    let base = serve(heads);
    let mut sb = case(&format!("{base}/")).await;

    sb.add_cookie(&json!({ "name": "k", "value": "v", "path": "/" }))
        .await
        .unwrap();
    sb.add_cookie(
        &json!({ "name": "k2", "value": "v2", "path": "/", "expiry": 4_102_444_800_i64 }),
    )
    .expiry(true)
    .await
    .unwrap();
    let cookies = js(&sb, "return document.cookie").await;
    assert!(
        cookies.contains("k=v") && cookies.contains("k2=v2"),
        "{cookies}"
    );

    let error = sb.add_cookie(&json!({ "name": "k3" })).await.unwrap_err();
    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "{error:?}"
    );
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn download_file_and_save_file_as_fetch_a_url_into_a_folder() {
    let heads = Arc::new(Mutex::new(Vec::new()));
    let base = serve(Arc::clone(&heads));
    let mut sb = case(&format!("{base}/")).await;
    let folder = tempfile::tempdir().expect("a temporary folder");
    let url = format!("{base}/files/report.csv");

    let saved = sb
        .download_file(&url)
        .destination_folder(folder.path())
        .await
        .unwrap();
    assert_eq!(saved, folder.path().join("report.csv"));
    assert_eq!(std::fs::read_to_string(&saved).unwrap(), "id,name\n1,Ada\n");

    let renamed = sb
        .save_file_as(&url, "copy.csv")
        .destination_folder(folder.path().join("nested"))
        .await
        .unwrap();
    assert_eq!(renamed, folder.path().join("nested").join("copy.csv"));
    assert_eq!(
        std::fs::read_to_string(&renamed).unwrap(),
        "id,name\n1,Ada\n"
    );

    // The request carried the browser's own user agent.
    let heads = heads.lock().unwrap().clone();
    assert_eq!(heads.len(), 2);
    assert!(
        heads.iter().all(|head| head.contains("Chrome")),
        "{heads:?}"
    );

    sb.quit().await.unwrap();
}
