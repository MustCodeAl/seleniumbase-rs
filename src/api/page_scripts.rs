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

/// `s` as a JSON string literal, which is also a JavaScript one.
fn literal(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_owned())
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

/// Outlines the element `css` picks for `restore_after_ms` milliseconds, then
/// puts back the outline it had. Returns `false` when no element matches.
pub(crate) fn highlight_script(css: &str, restore_after_ms: u32) -> String {
    // The old outline is kept on the element, so highlighting it again before the
    // timer fires restores the real outline, not the magenta one.
    format!(
        "var el = document.querySelector({css}); \
         if (!el) {{ return false; }} \
         try {{ el.scrollIntoView({{ block: 'center', inline: 'nearest' }}); }} catch (e) {{}} \
         if (el.__sbOutline === undefined) {{ \
             el.__sbOutline = [el.style.outline, el.style.outlineOffset]; \
         }} \
         clearTimeout(el.__sbOutlineTimer); \
         el.style.outline = '3px solid magenta'; \
         el.style.outlineOffset = '2px'; \
         el.__sbOutlineTimer = setTimeout(function () {{ \
             var old = el.__sbOutline; \
             el.style.outline = old[0]; \
             el.style.outlineOffset = old[1]; \
             delete el.__sbOutline; \
             delete el.__sbOutlineTimer; \
         }}, {restore_after_ms}); \
         return true;",
        css = literal(css)
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

/// Shows `message` in a corner of the page on a `color` background, replacing
/// the one already there, and removes it after `seconds`.
pub(crate) fn toast_script(message: &str, color: &str, seconds: u64) -> String {
    let millis = seconds.saturating_mul(1000);
    format!(
        "var old = document.getElementById('sb-toast-message'); \
         if (old) {{ old.remove(); }} \
         var box = document.createElement('div'); \
         box.id = 'sb-toast-message'; \
         box.textContent = {message}; \
         box.style.cssText = 'position:fixed;top:10px;right:10px;padding:12px;color:#fff;z-index:999999;'; \
         box.style.background = {color}; \
         document.body.appendChild(box); \
         setTimeout(function () {{ if (box.parentNode) {{ box.parentNode.removeChild(box); }} }}, {millis}); \
         return true;",
        message = literal(message),
        color = literal(color)
    )
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

/// Finds the deepest displayed element under `<body>` whose visible text
/// matches, or `null`.
pub(crate) fn find_visible_text_script(kind: TextMatch, text: &str) -> String {
    let kind = match kind {
        TextMatch::Contains => "contains",
        TextMatch::Exact => "exact",
        TextMatch::NonEmpty => "nonempty",
    };
    // Children are searched first, so the deepest element that fits wins; an element
    // is only a candidate if it shows the text, which leaves out `display: none`
    // subtrees and `<script>` text.
    format!(
        "var kind = '{kind}'; \
         var needle = {needle}; \
         function displayed(el) {{ \
             var style = window.getComputedStyle(el); \
             if (style.display === 'none' || style.visibility === 'hidden') {{ return false; }} \
             return el.getClientRects().length > 0; \
         }} \
         function could(text) {{ \
             if (kind === 'nonempty') {{ return text.trim().length > 0; }} \
             return text.indexOf(kind === 'exact' ? needle.trim() : needle) !== -1; \
         }} \
         function search(el) {{ \
             if (!displayed(el)) {{ return null; }} \
             var text = el.innerText || ''; \
             if (!could(text)) {{ return null; }} \
             for (var child = el.firstElementChild; child; child = child.nextElementSibling) {{ \
                 var found = search(child); \
                 if (found) {{ return found; }} \
             }} \
             if (kind === 'exact' && text.trim() !== needle.trim()) {{ return null; }} \
             return el; \
         }} \
         return document.body ? search(document.body) : null;",
        needle = literal(text)
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
            (highlight_script(css, 10), vec![css]),
            (bring_to_front_script(css), vec![css]),
            (select_by_index_script(css, 1), vec![css]),
            (
                find_visible_text_script(TextMatch::Contains, text),
                vec![text],
            ),
            (console_log_string_script(text), vec![text]),
            (toast_script(text, "#fff", 1), vec![text]),
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
    async fn highlight_outlines_the_element_and_then_puts_its_outline_back() {
        let (browser, page) =
            open_html("<!doctype html><div id=x style='outline:1px dotted blue'>x</div>").await;
        let found = page
            .evaluate(as_call(&highlight_script("#x", 300)))
            .await
            .unwrap();
        assert_eq!(found, Value::Bool(true));
        let during = page
            .evaluate("document.querySelector('#x').style.outline")
            .await
            .unwrap();
        assert!(during.as_str().unwrap().contains("magenta"), "{during}");
        tokio::time::sleep(std::time::Duration::from_millis(900)).await;
        let after = page
            .evaluate("document.querySelector('#x').style.outline")
            .await
            .unwrap();
        assert!(
            after.as_str().unwrap().contains("blue"),
            "the old outline is back: {after}"
        );
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
    async fn a_toast_replaces_the_previous_one_and_goes_away() {
        let (browser, page) = open_html("<!doctype html><p>page</p>").await;
        page.evaluate(as_call(&toast_script("first", "#28a745", 1)))
            .await
            .unwrap();
        page.evaluate(as_call(&toast_script("second", "#dc3545", 1)))
            .await
            .unwrap();
        let count = page
            .evaluate("document.querySelectorAll('#sb-toast-message').length")
            .await
            .unwrap();
        assert_eq!(
            count, 1,
            "repeated calls must not stack elements with one id"
        );
        let text = page
            .evaluate("document.getElementById('sb-toast-message').textContent")
            .await
            .unwrap();
        assert_eq!(text, "second");
        tokio::time::sleep(std::time::Duration::from_millis(1600)).await;
        let gone = page
            .evaluate("document.getElementById('sb-toast-message') === null")
            .await
            .unwrap();
        assert_eq!(
            gone,
            Value::Bool(true),
            "the toast must vanish after its duration"
        );
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

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn find_visible_text_returns_the_deepest_visible_match_and_skips_hidden_text() {
        let (browser, page) = open_html(
            "<!doctype html><div id=hid style='display:none'>hidden-needle</div>\
             <p id=p>hello <b id=deep>needle</b> world</p><span id=e>  </span>\
             <script>var inScript = 'script-needle';</script>",
        )
        .await;
        let id_of = |kind: TextMatch, text: &'static str| {
            let body = format!(
                "var el = (function () {{ {} }})(); return el ? el.id : null;",
                find_visible_text_script(kind, text)
            );
            let page = &page;
            async move { page.evaluate(as_call(&body)).await.unwrap() }
        };
        assert_eq!(id_of(TextMatch::Contains, "needle").await, "deep");
        assert_eq!(id_of(TextMatch::Contains, "hello").await, "p");
        assert_eq!(
            id_of(TextMatch::Contains, "hidden-needle").await,
            Value::Null
        );
        assert_eq!(
            id_of(TextMatch::Contains, "script-needle").await,
            Value::Null
        );
        assert_eq!(id_of(TextMatch::Exact, "needle").await, "deep");
        assert_eq!(
            id_of(TextMatch::Exact, "hello").await,
            Value::Null,
            "only part of the text"
        );
        assert_eq!(id_of(TextMatch::NonEmpty, "").await, "deep");
        browser.close().await.unwrap();
    }
}
