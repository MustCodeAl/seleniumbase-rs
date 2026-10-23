//! Page scripts behind `BaseCase` methods whose behaviour has to match Python
//! SeleniumBase.
//!
//! Each builder returns the *body* of a script, in the form WebDriver's
//! `execute_script` takes: it may `return` a value. Every argument is spliced
//! in as a JSON string literal, so a selector or a message cannot end the
//! literal and run as code. Keeping the scripts here, away from the methods
//! that send them, lets the tests run them in a real page (those tests are
//! `#[ignore]`d, because they need Chrome) and check what the page did.

/// What `find_text`, `find_exact_text` and `find_non_empty_text` look for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TextMatch {
    /// The element's visible text contains the text.
    Contains,
    /// The element's visible text, trimmed, is the text.
    Exact,
    /// The element's visible text is not blank.
    NonEmpty,
}

impl TextMatch {
    /// Whether `shown`, the visible text of an element, is what this kind of
    /// match looks for with `wanted`.
    pub(crate) fn holds(self, shown: &str, wanted: &str) -> bool {
        match self {
            Self::Contains => shown.contains(wanted),
            Self::Exact => shown.trim() == wanted.trim(),
            Self::NonEmpty => !shown.trim().is_empty(),
        }
    }
}

/// Upstream SeleniumBase's `AD_BLOCK_LIST`: selectors for the elements
/// `ad_block` removes, each ready for `querySelectorAll`.
pub(crate) const AD_BLOCK_SELECTORS: &[&str] = &[
    r#"[aria-label="Ads"]"#,
    r#"[src*="adservice."]"#,
    r#"[src*="adclick"]"#,
    r#"[src*="doubleclick"]"#,
    r#"[src*="snigelweb.com"]"#,
    r#"[src*="tagservices.com"]"#,
    r#"[src*="adsby"]"#,
    r#"[src*="adroll.com"]"#,
    r#"[src*="pagead"]"#,
    r#"[src*="3lift"]"#,
    r#"[src*="smartads."]"#,
    r#"[src*="ad_nexus"]"#,
    r#"[src*="/ads/"]"#,
    r#"[src*="moatads.com"]"#,
    r#"[src*="adsystem"]"#,
    r#"[src*="connectad"]"#,
    r#"[src*="/adservice."]"#,
    r#"[src*="syndication.com"]"#,
    r#"[src*="/ads."]"#,
    r#"[src*="lijit"]"#,
    r#"[src*="pagead"]"#,
    r#"[src*="adnxs.com"]"#,
    r#"[src*="onetag-sys.com"]"#,
    r#"[src*="indexww.com"]"#,
    r#"[src*="3lift.com"]"#,
    r#"[src*="rubiconproject.com"]"#,
    r#"[src*="brealtime.com"]"#,
    r#"[src*="33across.com"]"#,
    r#"[src*="adsrvr"]"#,
    r#"[type="data-doubleclick"]"#,
    "iframe[data-google-container-id]",
    r#"iframe[src*="doubleclick"]"#,
    r#"iframe[src*="/AdServer/"]"#,
    r#"iframe[src*="openx.net"]"#,
    r#"iframe[onload*="doWithAds"]"#,
    r#"iframe[id*="_ads_frame"]"#,
    r#"iframe[style="height:0px;width:0px;display:none;"]"#,
    r#"[aria-label="Ad"]"#,
    r#"[aria-label="Timeline: Trending now"]"#,
    r#"[aria-label="Timeline: Carousel"]"#,
    r#"[aria-roledescription="carousel"]"#,
    r#"[aria-label="Who to follow"]"#,
    r#"[class*="sponsored-content"]"#,
    r#"[class*="adsbygoogle"]"#,
    r#"[class^="adroll"]"#,
    r#"[data-ad-details*="Advertisement"]"#,
    r#"[data-native_ad*="placement"]"#,
    r#"[data-provider="dianomi"]"#,
    r#"[data-type="ad"]"#,
    r#"[data-track-event-label*="-taboola-"]"#,
    r#"[data-ad-feedback-beacon*="AD_"]"#,
    "[data-ad-feedback-beacon]",
    r#"[data-dcm-click-tracker*="/adclick."]"#,
    "[data-google-av-adk]",
    "[data-google-query-id]",
    r#"[data-ylk*="sponsored_cluster"]"#,
    "[data-google-av-cxn]",
    "[data-ad-client]",
    "[data-ad-slot]",
    r#"[href*="doubleclick"]"#,
    r#"[href*="amazon-adsystem"]"#,
    r#"[alt="Advertisement"]"#,
    r#"[alt$=" Ad"]"#,
    r#"[id*="-ad-"]"#,
    r#"[id*="_ads_"]"#,
    r#"[id*="AdFrame"]"#,
    r#"[id*="carbonads"]"#,
    r#"[id^="ad-"]"#,
    r#"[id^="my-ads"]"#,
    r#"[id^="outbrain_widget"]"#,
    r#"[id^="taboola-"]"#,
    r#"[id^="google_ads_frame"]"#,
    r#"[id^="google_ads_iframe"]"#,
    r#"[id="tryitLeaderboard"]"#,
    r#"[id="dianomiRightRail"]"#,
    r#"[allow*="advertising.com"]"#,
    "ins.adsby",
    "li.strm-ad-clusters",
    "li.js-stream-ad",
    "div.after_ad",
    "div.ad-container",
    "div.ad_module",
    "div.ad-subnav-container",
    "div.ad-wrapper",
    "div.adroll-block",
    "div.data-ad-container",
    "div.GoogleActiveViewElement",
    "div.l-ad",
    "div.right-ad",
    "div.wx-adWrapper",
    r#"div.image > a > img[src*="HomepageAd"]"#,
    r#"img[src*="HomepageAd"]"#,
    "img.img_ad",
    r#"link[href*="/adservice."]"#,
    "section.dianomi-ad",
    "ytd-promoted-video-renderer",
    "ytd-video-masthead-ad-v3-renderer",
];

/// `s` as a JSON string literal, which is also a JavaScript one. JSON lets the
/// line and paragraph separators through; JavaScript before ES2019 ends a
/// string at them, so they are written as escapes.
fn literal(s: &str) -> String {
    serde_json::to_string(s)
        .unwrap_or_else(|_| "\"\"".to_owned())
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// Sets the text of the element `css` picks: its `value` when it is an
/// `<input>` or `<textarea>` (firing `input` and `change`), its `textContent`
/// otherwise. Returns `false` when no element matches.
pub(crate) fn set_text_script(css: &str, text: &str) -> String {
    format!(
        "var el = document.querySelector({css}); \
         if (!el) {{ return false; }} \
         var text = {text}; \
         var tag = el.tagName.toLowerCase(); \
         if (tag === 'input' || tag === 'textarea') {{ \
             el.value = text; \
             el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
             el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
         }} else {{ \
             el.textContent = text; \
         }} \
         return true;",
        css = literal(css),
        text = literal(text)
    )
}

/// Raises the z-index of the element `css` picks. Returns `false` when no
/// element matches.
pub(crate) fn bring_to_front_script(css: &str) -> String {
    format!(
        "var el = document.querySelector({css}); \
         if (!el) {{ return false; }} \
         el.style.zIndex = '999999'; \
         return true;",
        css = literal(css)
    )
}

/// Selects option number `index` of the `<select>` `css` picks and fires
/// `input` and `change`. Returns `"ok"`, `"no-select"` when `css` is not a
/// `<select>`, `"no-option"` when it has no such option, or `"disabled"`.
pub(crate) fn select_by_index_script(css: &str, index: usize) -> String {
    format!(
        "var select = document.querySelector({css}); \
         if (!select || select.tagName.toLowerCase() !== 'select') {{ return 'no-select'; }} \
         var option = select.options[{index}]; \
         if (!option) {{ return 'no-option'; }} \
         if (option.disabled) {{ return 'disabled'; }} \
         select.selectedIndex = {index}; \
         select.dispatchEvent(new Event('input', {{ bubbles: true }})); \
         select.dispatchEvent(new Event('change', {{ bubbles: true }})); \
         return 'ok';",
        css = literal(css)
    )
}

/// Logs the value of the JavaScript expression `expression` to the console.
pub(crate) fn console_log_expression_script(expression: &str) -> String {
    format!("console.log({expression});")
}

/// Logs the string `text` to the console.
pub(crate) fn console_log_string_script(text: &str) -> String {
    format!("console.log({});", literal(text))
}

/// Removes every element [`AD_BLOCK_SELECTORS`] picks. Returns how many.
pub(crate) fn ad_block_script() -> String {
    let selectors = serde_json::to_string(AD_BLOCK_SELECTORS).unwrap_or_else(|_| "[]".to_owned());
    format!(
        "var selectors = {selectors}; \
         var removed = 0; \
         selectors.forEach(function (selector) {{ \
             try {{ \
                 document.querySelectorAll(selector).forEach(function (el) {{ el.remove(); removed++; }}); \
             }} catch (e) {{}} \
         }}); \
         return removed;"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::base_case::chrome_support::open_html;
    use serde_json::Value;

    /// Runs a script body in the page the way WebDriver's `execute_script` does.
    fn as_call(body: &str) -> String {
        format!("(function () {{ {body} }})()")
    }

    #[test]
    fn a_hostile_selector_and_text_stay_inside_their_string_literals() {
        let css = "a\"]; window.pwned = 1; //";
        let text = "x'); window.pwned = 2; //";
        // Each payload may only appear as the JSON literal of an argument.
        for (script, args) in [
            (set_text_script(css, text), vec![css, text]),
            (bring_to_front_script(css), vec![css]),
            (select_by_index_script(css, 1), vec![css]),
            (console_log_string_script(text), vec![text]),
        ] {
            let mut rest = script.clone();
            for arg in args {
                assert!(
                    rest.contains(&literal(arg)),
                    "{arg:?} is not a literal in {script}"
                );
                rest = rest.replace(&literal(arg), "");
            }
            assert!(
                !rest.contains("pwned"),
                "payload escaped its literal: {script}"
            );
        }
    }

    #[test]
    fn a_literal_holds_no_raw_line_or_paragraph_separator() {
        let written = literal("a\u{2028}b\u{2029}c\nd");
        assert_eq!(written, "\"a\\u2028b\\u2029c\\nd\"");
    }

    #[test]
    fn the_ad_block_list_is_upstreams_not_a_generic_guess() {
        let has = |s: &str| AD_BLOCK_SELECTORS.contains(&s);
        assert!(has("[src*=\"adnxs.com\"]"));
        assert!(has("[src*=\"rubiconproject.com\"]"));
        assert!(has("[aria-label=\"Ads\"]"));
        assert!(has("iframe[data-google-container-id]"));
        // A bare `.ad` class is not on upstream's list: it removes legitimate elements.
        assert!(
            !has(".ad") && !has(".ads"),
            "generic class guesses are not upstream's"
        );
        assert!(
            AD_BLOCK_SELECTORS.len() > 90,
            "{}",
            AD_BLOCK_SELECTORS.len()
        );
        let script = ad_block_script();
        assert!(script.contains("adnxs.com") && script.contains("rubiconproject.com"));
    }

    #[test]
    fn console_scripts_log_an_expression_and_a_string() {
        assert_eq!(
            console_log_expression_script("document.title"),
            "console.log(document.title);"
        );
        assert_eq!(
            console_log_string_script("Hello"),
            "console.log(\"Hello\");"
        );
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn set_text_puts_text_in_an_input_and_a_textarea_and_in_a_div() {
        let (browser, page) = open_html(
            "<!doctype html><input id=a oninput='window.inputs=(window.inputs||0)+1' \
             onchange='window.changes=(window.changes||0)+1'>\
             <textarea id=t></textarea><div id=d>old</div>",
        )
        .await;
        for (css, text) in [("#a", "ann"), ("#t", "multi\nline"), ("#d", "new")] {
            let found = page
                .evaluate(as_call(&set_text_script(css, text)))
                .await
                .unwrap();
            assert_eq!(found, Value::Bool(true), "{css}");
        }
        assert_eq!(
            page.evaluate("document.querySelector('#a').value")
                .await
                .unwrap(),
            "ann"
        );
        assert_eq!(
            page.evaluate("document.querySelector('#t').value")
                .await
                .unwrap(),
            "multi\nline"
        );
        assert_eq!(
            page.evaluate("document.querySelector('#d').textContent")
                .await
                .unwrap(),
            "new"
        );
        assert_eq!(
            page.evaluate("window.inputs").await.unwrap(),
            1,
            "input fired once"
        );
        assert_eq!(
            page.evaluate("window.changes").await.unwrap(),
            1,
            "change fired once"
        );
        let missing = page
            .evaluate(as_call(&set_text_script("#nope", "x")))
            .await
            .unwrap();
        assert_eq!(missing, Value::Bool(false));
        browser.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn bring_to_front_raises_the_z_index_of_an_element() {
        let (browser, page) = open_html("<!doctype html><button id=b>b</button>").await;
        let found = page
            .evaluate(as_call(&bring_to_front_script("#b")))
            .await
            .unwrap();
        assert_eq!(found, Value::Bool(true));
        let z = page
            .evaluate("document.querySelector('#b').style.zIndex")
            .await
            .unwrap();
        assert_eq!(z, "999999");
        let missing = page
            .evaluate(as_call(&bring_to_front_script("#nope")))
            .await
            .unwrap();
        assert_eq!(missing, Value::Bool(false));
        browser.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn select_by_index_selects_fires_events_and_refuses_a_missing_option() {
        let (browser, page) = open_html(
            "<!doctype html><select id=s onchange='window.changed=(window.changed||0)+1'>\
             <option>a</option><option>b</option><option>c</option>\
             <option disabled>d</option></select><input id=i>",
        )
        .await;
        let run = |css: &'static str, index: usize| {
            page.evaluate(as_call(&select_by_index_script(css, index)))
        };
        assert_eq!(run("#s", 99).await.unwrap(), "no-option");
        assert_eq!(
            page.evaluate("window.changed").await.unwrap(),
            Value::Null,
            "no event for a miss"
        );
        assert_eq!(
            page.evaluate("document.querySelector('#s').selectedIndex")
                .await
                .unwrap(),
            0
        );
        assert_eq!(run("#s", 3).await.unwrap(), "disabled");
        assert_eq!(run("#i", 0).await.unwrap(), "no-select");
        assert_eq!(run("#nope", 0).await.unwrap(), "no-select");
        assert_eq!(run("#s", 2).await.unwrap(), "ok");
        assert_eq!(
            page.evaluate("document.querySelector('#s').selectedIndex")
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            page.evaluate("window.changed").await.unwrap(),
            1,
            "a change event ran the page's handler"
        );
        browser.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn console_scripts_write_to_the_console() {
        let (browser, page) = open_html("<!doctype html><title>T</title>").await;
        page.evaluate(
            "window.seen = []; console.log = function () { \
             window.seen.push(Array.prototype.join.call(arguments, ' ')); }; 1",
        )
        .await
        .unwrap();
        page.evaluate(as_call(&console_log_string_script(
            "it's `quoted` \"text\"",
        )))
        .await
        .unwrap();
        page.evaluate(as_call(&console_log_expression_script("document.title")))
            .await
            .unwrap();
        let seen = page.evaluate("window.seen.join('|')").await.unwrap();
        assert_eq!(seen, "it's `quoted` \"text\"|T");
        browser.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn ad_block_removes_upstreams_ads_and_leaves_ordinary_ad_classes() {
        let (browser, page) = open_html(
            "<!doctype html><div id=adn src='//x.adnxs.com/a'>network ad</div>\
             <div id=aria aria-label='Ads'>x</div><div id=keep class='ad'>a real class named ad</div>",
        )
        .await;
        let removed = page.evaluate(as_call(&ad_block_script())).await.unwrap();
        assert!(removed.as_u64().unwrap() >= 2, "{removed}");
        let left = page
            .evaluate("['adn','aria','keep'].filter(function (i) { return document.getElementById(i); }).join()")
            .await
            .unwrap();
        assert_eq!(left, "keep");
        browser.close().await.unwrap();
    }
}
