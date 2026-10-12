// HTML / DOM helpers.

/// A page that `set_content_to_frame` replaced, kept so that
/// `set_content_to_default` and `set_content_to_parent` can bring it back.
#[derive(Clone, Debug)]
struct SwappedPage {
    /// The page's address.
    url: String,
    /// The page's source when it was replaced.
    source: String,
    /// The window or tab the page was in.
    window: String,
}

/// Reads `[document.cframe_swap, document.cframe_tab]` from the page. The first
/// counts the frames the page now shows in place of itself, the second is `1`
/// in a page that opened a frame in another tab or is that tab. Both are page
/// variables, as in Python, so a navigation drops them.
const FRAME_SWAP_STATE_SCRIPT: &str =
    "return [Number(document.cframe_swap) || 0, Number(document.cframe_tab) || 0];";
/// Counts one more frame swapped into the page.
const FRAME_SWAP_UP_SCRIPT: &str =
    "document.cframe_swap = (Number(document.cframe_swap) || 0) + 1;";
/// Counts one frame fewer swapped into the page.
const FRAME_SWAP_DOWN_SCRIPT: &str =
    "document.cframe_swap = Math.max((Number(document.cframe_swap) || 0) - 1, 0);";
/// Forgets that frames were swapped into the page.
const FRAME_SWAP_RESET_SCRIPT: &str = "document.cframe_swap = 0;";
/// Marks the page as one that has to do with a frame opened in a tab.
const FRAME_TAB_SCRIPT: &str = "document.cframe_tab = 1;";

/// The script that makes the page show the document `html`: the head and the
/// body of the page are replaced with those of `html`.
///
/// `html` is parsed as a whole document, so its `<html>`, `<head>` and
/// `<body>` tags do not end up inside the body, and its styles come with it.
/// Scripts in it do not run, as with any `innerHTML`.
fn replace_document_script(html: &str) -> String {
    format!(
        "var next = new DOMParser().parseFromString({html}, 'text/html'); \
         document.head.innerHTML = next.head.innerHTML; \
         document.body.innerHTML = next.body.innerHTML;",
        html = quote(html)
    )
}

/// Whether an iframe with this `src` property is opened in a tab by
/// `set_content_to_frame` instead of being swapped into the page: the
/// property is the resolved address, so this is a web or file address.
fn opens_in_a_tab(src: &str) -> bool {
    ["http:", "https:", "file:"]
        .iter()
        .any(|scheme| src.starts_with(scheme))
}

impl BaseCase {
    /// Returns the current page source.
    pub async fn get_html(&self) -> Result<String, SeleniumBaseError> {
        self.get_page_source().await
    }

    /// Saves the current page source to the logs directory.
    pub async fn save_page_source(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        let source = self.get_page_source().await?;
        let dir = ensure_latest_logs_dir()?;
        let path = crate::artifacts::confined_path(&dir, filename)?;
        fs::write(&path, source)?;
        Ok(path)
    }

    /// Loads a local HTML file into the browser.
    pub async fn load_html_file(&mut self, path: &str) -> Result<(), SeleniumBaseError> {
        let abs = std::fs::canonicalize(path)?;
        let url = url::Url::from_file_path(&abs)
            .map_err(|_| SeleniumBaseError::InvalidConfig("invalid file path".to_owned()))?;
        self.open(url.as_str()).await
    }

    /// Alias for `load_html_file`.
    pub async fn open_html_file(&mut self, path: &str) -> Result<(), SeleniumBaseError> {
        self.load_html_file(path).await
    }

    /// Sets `document.body.innerHTML` to `html`.
    pub async fn set_content(&self, html: &str) -> Result<(), SeleniumBaseError> {
        self.execute_script(&format!("document.body.innerHTML = {};", quote(html)))
            .await?;
        Ok(())
    }

    /// Makes the page show what the iframe `frame` shows, in place of the
    /// page's own content. Corresponds to Python's `set_content_to_frame`.
    ///
    /// `frame` selects the iframe, or is its `name`. The iframe's document
    /// replaces the head and the body of the page, so the page's commands
    /// (`click`, `get_text`, ...) work on it without entering the frame; the
    /// address does not change. If the iframe's `src` is a web or file
    /// address, that address is opened in a new tab instead, which becomes the
    /// current one.
    ///
    /// [`set_content_to_default`](Self::set_content_to_default) brings the page
    /// back, and [`set_content_to_parent`](Self::set_content_to_parent) undoes
    /// one swap of several. Scripts of the frame's document do not run once it
    /// is swapped in.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the iframe does not appear
    /// within the default wait, and the driver's error if it cannot be entered.
    /// The page is left as it was in both cases.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// sb.set_content_to_frame("iframe#payment").await?;
    /// sb.click("button#pay").await?;
    /// sb.set_content_to_default().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_content_to_frame(&mut self, frame: &str) -> Result<(), SeleniumBaseError> {
        let selector = self.frame_selector(frame).await?;
        let by = Selector::auto(&selector).to_by()?;
        let url = self.get_current_url().await?;
        let window = self.get_current_window_handle().await?;
        let source = self.get_page_source().await?;
        self.session.switch_to_frame(by.clone()).await?;
        let frame_source = self.get_page_source().await;
        // Leave the frame whether or not its source could be read.
        self.session.switch_to_default_content().await?;
        let frame_source = frame_source?;
        self.wait_for_ready_state_complete().await?;
        let src = self
            .session
            .get_property(by, "src")
            .await?
            .unwrap_or_default();
        if opens_in_a_tab(&src) {
            self.execute_script(FRAME_TAB_SCRIPT).await?;
            self.open_new_window().await?;
            self.open(&src).await?;
            self.execute_script(FRAME_TAB_SCRIPT).await?;
        } else {
            self.execute_script(&replace_document_script(&frame_source))
                .await?;
            self.execute_script(FRAME_SWAP_UP_SCRIPT).await?;
        }
        self.swapped_pages
            .push(SwappedPage { url, source, window });
        Ok(())
    }

    /// The selector of the iframe `frame` names: `frame` itself if it selects
    /// an element, else the iframe whose `name` it is. Waits for `frame`
    /// otherwise.
    async fn frame_selector(&self, frame: &str) -> Result<String, SeleniumBaseError> {
        if self.is_element_present(frame).await? {
            return Ok(frame.to_owned());
        }
        if !frame.contains(' ') {
            let by_name = format!(
                "iframe[name=\"{}\"]",
                frame.replace('\\', "\\\\").replace('"', "\\\"")
            );
            if self.is_element_present(&by_name).await? {
                return Ok(by_name);
            }
        }
        self.await_ready(frame, Ready::Present).await?;
        Ok(frame.to_owned())
    }

    /// How many frames the page shows in place of itself, and whether it is
    /// tied to a frame opened in a tab: the page's `document.cframe_swap` and
    /// `document.cframe_tab`.
    async fn frame_swap_state(&self) -> Result<(u64, u64), SeleniumBaseError> {
        let state = self.execute_script(FRAME_SWAP_STATE_SCRIPT).await?;
        let count = |index: usize| state.get(index).and_then(Value::as_u64).unwrap_or(0);
        Ok((count(0), count(1)))
    }

    /// Brings `past` back: its tab if it is still open, then its content, or a
    /// refresh if the tab has been somewhere else since.
    async fn restore_swapped_page(&mut self, past: &SwappedPage) -> Result<(), SeleniumBaseError> {
        if self.get_current_window_handle().await? != past.window
            && self.get_all_window_handles().await?.contains(&past.window)
        {
            self.session.switch_to_window(&past.window).await?;
        }
        if self.get_current_url().await? == past.url {
            self.execute_script(&replace_document_script(&past.source))
                .await?;
            Ok(())
        } else {
            self.refresh_page().await
        }
    }

    /// Brings back the page that [`set_content_to_frame`](Self::set_content_to_frame)
    /// replaced, however many frames were swapped in. Corresponds to Python's
    /// `set_content_to_default`.
    ///
    /// If a swap was made but the page has since been navigated away from (its
    /// counters are gone), the page is refreshed instead, as Python does.
    /// If no frame was swapped in, this only leaves the frame the driver is in
    /// (see [`switch_to_default_content`](Self::switch_to_default_content));
    /// Python refreshes the page in that case.
    ///
    /// # Errors
    ///
    /// Returns the driver's error if the page or its tab cannot be reached.
    pub async fn set_content_to_default(&mut self) -> Result<(), SeleniumBaseError> {
        if self.swapped_pages.is_empty() {
            return self.switch_to_default_content().await;
        }
        let (swaps, tabs) = self.frame_swap_state().await?;
        match self.swapped_pages.first().cloned() {
            Some(past) if swaps > 0 || tabs > 0 => self.restore_swapped_page(&past).await?,
            _ => self.refresh_page().await?,
        }
        self.execute_script(FRAME_SWAP_RESET_SCRIPT).await?;
        self.swapped_pages.clear();
        Ok(())
    }

    /// Same as [`set_content_to_default`](Self::set_content_to_default).
    pub async fn set_content_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        self.set_content_to_default().await
    }

    /// Undoes the last [`set_content_to_frame`](Self::set_content_to_frame), so
    /// the page shows what it showed before that swap. Corresponds to Python's
    /// `set_content_to_parent`.
    ///
    /// Like [`set_content_to_default`](Self::set_content_to_default), it
    /// refreshes the page when the swap was forgotten, and only leaves the
    /// current frame (see
    /// [`switch_to_parent_frame`](Self::switch_to_parent_frame)) when no frame
    /// was swapped in.
    ///
    /// # Errors
    ///
    /// Returns the driver's error if the page or its tab cannot be reached.
    pub async fn set_content_to_parent(&mut self) -> Result<(), SeleniumBaseError> {
        if self.swapped_pages.is_empty() {
            return self.switch_to_parent_frame().await;
        }
        let (swaps, tabs) = self.frame_swap_state().await?;
        let mut refresh = false;
        if swaps > 0 {
            self.execute_script(FRAME_SWAP_DOWN_SCRIPT).await?;
            let past = self.swapped_pages.pop();
            match past {
                Some(past) if self.get_current_url().await? == past.url => {
                    self.execute_script(&replace_document_script(&past.source))
                        .await?;
                }
                _ => refresh = true,
            }
        } else if tabs > 0 {
            match self.swapped_pages.pop() {
                Some(past) if self.get_all_window_handles().await?.contains(&past.window) => {
                    self.session.switch_to_window(&past.window).await?;
                }
                _ => refresh = true,
            }
        } else {
            refresh = true;
        }
        if refresh {
            self.refresh_page().await?;
            self.execute_script(FRAME_SWAP_RESET_SCRIPT).await?;
            self.swapped_pages.clear();
        }
        Ok(())
    }

    /// Same as [`set_content_to_parent`](Self::set_content_to_parent).
    pub async fn set_content_to_parent_frame(&mut self) -> Result<(), SeleniumBaseError> {
        self.set_content_to_parent().await
    }

    /// Removes all elements matching `css` from the DOM.
    pub async fn remove_elements(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelectorAll({}).forEach(el => el.remove());",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Hides all elements matching `css`.
    pub async fn hide_elements(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelectorAll({}).forEach(el => el.style.display = 'none');",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Shows all elements matching `css`.
    pub async fn show_elements(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelectorAll({}).forEach(el => el.style.display = '');",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Removes `attribute` from all elements matching `css`.
    pub async fn remove_attributes(
        &self,
        css: &str,
        attribute: &str,
    ) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelectorAll({}).forEach(el => el.removeAttribute({}));",
            quote(css),
            quote(attribute)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Sets `attribute=value` on all elements matching `css`.
    pub async fn set_attributes(
        &self,
        css: &str,
        attribute: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelectorAll({}).forEach(el => el.setAttribute({}, {}));",
            quote(css),
            quote(attribute),
            quote(value)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Alias for `set_attributes`.
    pub async fn set_attribute_all(
        &self,
        css: &str,
        attribute: &str,
        value: &str,
    ) -> Result<(), SeleniumBaseError> {
        self.set_attributes(css, attribute, value).await
    }

    /// Injects a CSS stylesheet link into `<head>`.
    pub async fn add_css_link(&self, href: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var l=document.createElement('link'); l.rel='stylesheet'; l.href={}; document.head.appendChild(l);",
            quote(href)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Injects a JavaScript script link into `<head>`.
    pub async fn add_js_link(&self, src: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var s=document.createElement('script'); s.src={}; document.head.appendChild(s);",
            quote(src)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Injects raw CSS into `<head>`.
    pub async fn add_css_style(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var s=document.createElement('style'); s.textContent={}; document.head.appendChild(s);",
            quote(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Injects raw JavaScript into `<head>`.
    pub async fn add_js_code(&self, js: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var s=document.createElement('script'); s.textContent={}; document.head.appendChild(s);",
            quote(js)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Downloads the JavaScript at `url` and injects it into the page as inline
    /// code. The page itself never requests `url`, so a content security policy
    /// that blocks the script's host does not block it, and the code has run by
    /// the time this returns (unlike [`add_js_link`](Self::add_js_link), whose
    /// script loads asynchronously). Corresponds to Python's
    /// `add_js_code_from_link`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Network`] if the script cannot be
    /// downloaded or the server answers with an error status.
    pub async fn add_js_code_from_link(&self, url: &str) -> Result<(), SeleniumBaseError> {
        let code = fetch_text(url).await?;
        self.add_js_code(&code).await
    }

    /// Injects a `<meta>` tag into `<head>`.
    pub async fn add_meta_tag(&self, name: &str, content: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var m=document.createElement('meta'); m.name={}; m.content={}; document.head.appendChild(m);",
            quote(name),
            quote(content)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Saves the current page source to `filename`.
    pub async fn save_as_html(&self, filename: &str) -> Result<PathBuf, SeleniumBaseError> {
        self.save_page_source(filename).await
    }

    /// Loads raw HTML into the browser via a data URL.
    pub async fn load_html_string(&mut self, html: &str) -> Result<(), SeleniumBaseError> {
        let encoded = BASE64_STANDARD.encode(html);
        let url = format!("data:text/html;base64,{}", encoded);
        self.open(&url).await
    }

    /// Sets the text of `css`: the `value` of an `<input>` or `<textarea>`,
    /// the `textContent` of any other element. The same as
    /// [`set_text`](Self::set_text), as it is in Python.
    pub async fn set_text_content(&mut self, css: &str, text: &str) -> Result<(), SeleniumBaseError> {
        self.set_text(css, text).await
    }

    /// Returns the element at the given viewport coordinates.
    pub async fn get_element_at_x_y(
        &mut self,
        x: i64,
        y: i64,
    ) -> Result<Option<WebElement>, SeleniumBaseError> {
        let script = format!("return document.elementFromPoint({}, {});", x, y);
        let result = self.execute_script(&script).await?;
        self.element_from_script_result(result).await
    }

    /// Returns the parent element of `css`.
    pub async fn get_parent(&mut self, css: &str) -> Result<Option<WebElement>, SeleniumBaseError> {
        let script = format!(
            "return document.querySelector({})?.parentElement;",
            quote(css)
        );
        let result = self.execute_script(&script).await?;
        self.element_from_script_result(result).await
    }

    /// Returns the origin of the current URL, as `window.location.origin`
    /// reports it: the scheme, the host and a port that is not the scheme's
    /// default (`http://localhost:8080`), or `null` for a page with no origin
    /// such as `about:blank`.
    ///
    /// # Errors
    ///
    /// Returns an error if the current URL cannot be read or parsed.
    pub async fn get_origin(&mut self) -> Result<String, SeleniumBaseError> {
        let url = self.get_current_url().await?;
        origin_of_url(&url)
    }

    /// Returns true if `css` is not in this document but is inside an
    /// `<iframe>` or `<frame>` of it, at any depth.
    ///
    /// The elements of the current document are not "in an iframe", so this is
    /// false for them, as in Python SeleniumBase. Only frames from the same
    /// origin can be searched; the contents of a cross-origin frame are hidden
    /// from the page's script and count as not found.
    ///
    /// # Errors
    ///
    /// Returns an error if `css` is not a valid CSS selector.
    pub async fn is_element_in_an_iframe(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let script = element_in_iframe_script(css);
        match self.execute_script(&script).await? {
            Value::Bool(v) => Ok(v),
            _ => Ok(false),
        }
    }
}

/// Time allowed to download one script for `add_js_code_from_link`.
const FETCH_TEXT_TIMEOUT: Duration = Duration::from_secs(30);

/// The body of the page at `url`, as text; redirects are followed.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Network`] if the request fails or the answer
/// is not a success status.
async fn fetch_text(url: &str) -> Result<String, SeleniumBaseError> {
    let failed = |e: reqwest::Error| SeleniumBaseError::network(url, 0, e.to_string());
    let client = reqwest::Client::builder()
        .timeout(FETCH_TEXT_TIMEOUT)
        .build()
        .map_err(failed)?;
    let response = client.get(url).send().await.map_err(failed)?;
    let status = response.status();
    if !status.is_success() {
        return Err(SeleniumBaseError::network(url, status.as_u16(), status.to_string()));
    }
    response.text().await.map_err(failed)
}

/// The origin of `url`, serialized the way `window.location.origin` does.
fn origin_of_url(url: &str) -> Result<String, SeleniumBaseError> {
    let parsed = url::Url::parse(url)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("invalid URL '{url}': {e}")))?;
    // Chrome reports a local file's origin as `file://`; the URL standard calls it opaque.
    if parsed.scheme() == "file" {
        return Ok("file://".to_owned());
    }
    Ok(parsed.origin().ascii_serialization())
}

/// A script that tells whether `css` matches an element inside a frame of the
/// current document but not in the document itself.
fn element_in_iframe_script(css: &str) -> String {
    format!(
        "var selector = {}; \
         if (document.querySelector(selector) !== null) {{ return false; }} \
         function searchFrames(doc, depth) {{ \
             var frames = doc.querySelectorAll('iframe, frame'); \
             for (var i = 0; i < frames.length; i++) {{ \
                 var inner = null; \
                 try {{ inner = frames[i].contentDocument; }} catch (e) {{ inner = null; }} \
                 if (!inner) {{ continue; }} \
                 if (inner.querySelector(selector) !== null) {{ return true; }} \
                 if (depth < 8 && searchFrames(inner, depth + 1)) {{ return true; }} \
             }} \
             return false; \
         }} \
         return searchFrames(document, 0);",
        quote(css)
    )
}

#[cfg(test)]
mod tests_dom {
    use super::chrome_support::open_html;
    use super::*;

    #[tokio::test]
    async fn fetch_text_returns_the_body_and_refuses_an_error_status() {
        let base = chrome_support::serve(vec![
            ("/lib.js", 200, String::new(), "window.libLoaded = true;".to_owned()),
            ("/gone.js", 404, String::new(), "not here".to_owned()),
        ]);
        assert_eq!(
            fetch_text(&format!("{base}/lib.js")).await.unwrap(),
            "window.libLoaded = true;"
        );
        let error = fetch_text(&format!("{base}/gone.js")).await.unwrap_err();
        assert!(matches!(error, SeleniumBaseError::Network { .. }), "{error:?}");
        let unreachable = fetch_text("http://127.0.0.1:1/x.js").await.unwrap_err();
        assert!(matches!(unreachable, SeleniumBaseError::Network { .. }), "{unreachable:?}");
    }

    #[test]
    fn origin_keeps_a_port_that_is_not_the_default_for_its_scheme() {
        let cases = [
            ("http://localhost:8080/a/b?q=1#h", "http://localhost:8080"),
            ("https://example.com/x", "https://example.com"),
            ("https://example.com:443/x", "https://example.com"),
            ("http://example.com:80/", "http://example.com"),
            ("https://user:pw@example.com:8443/x", "https://example.com:8443"),
            ("HTTP://EXAMPLE.com/", "http://example.com"),
            ("http://[::1]:3000/", "http://[::1]:3000"),
        ];
        for (url, expected) in cases {
            assert_eq!(origin_of_url(url).unwrap(), expected, "{url}");
        }
    }

    #[test]
    fn a_page_with_no_origin_has_the_origin_null_not_an_empty_host() {
        for url in ["about:blank", "data:text/html,hi", "javascript:void(0)"] {
            assert_eq!(origin_of_url(url).unwrap(), "null", "{url}");
        }
        assert_eq!(origin_of_url("file:///tmp/page.html").unwrap(), "file://");
    }

    #[test]
    fn origin_of_something_that_is_not_a_url_is_an_error() {
        assert!(matches!(origin_of_url("not a url"), Err(SeleniumBaseError::InvalidConfig(_))));
    }

    #[test]
    fn the_iframe_script_embeds_the_selector_as_a_string_literal() {
        let script = element_in_iframe_script("a[title='x\"y']\n");
        assert!(script.starts_with(r#"var selector = "a[title='x\"y']\n";"#), "{script}");
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn origin_agrees_with_the_browser() {
        let (browser, page) = open_html("<!doctype html><title>x</title>").await;
        for url in [
            "http://localhost:8080/a",
            "https://example.com:8443/b?c#d",
            "https://example.com:443/",
            "http://user:pw@example.com/",
            "about:blank",
            "data:text/html,hi",
            "file:///tmp/x.html",
            "blob:https://example.com/uuid",
        ] {
            let expected: String = page
                .evaluate_as(format!("new URL({}).origin", quote(url)))
                .await
                .unwrap();
            assert_eq!(origin_of_url(url).unwrap(), expected, "{url}");
        }
        // And for the page the browser is actually showing.
        let shown: String = page.evaluate_as("location.href").await.unwrap();
        let real: String = page.evaluate_as("window.location.origin").await.unwrap();
        assert_eq!(origin_of_url(&shown).unwrap(), real);
        assert!(real.contains(':'), "the fixture runs on a port: {real}");
        browser.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "needs Chrome"]
    async fn an_element_is_in_an_iframe_only_when_a_frame_holds_it() {
        let (browser, page) = open_html(
            "<!doctype html><p id=here>top</p>\
             <iframe id=one srcdoc=\"<p id='inner'>x</p><iframe id='two' srcdoc='<p id=deep>y</p>'></iframe>\"></iframe>",
        )
        .await;
        page.wait_for_load().await.unwrap();
        // The nested frame loads asynchronously.
        page.wait_for_function("document.getElementById('one').contentDocument.getElementById('two')?.contentDocument?.getElementById('deep')")
            .await
            .unwrap();
        let ask = |css: &'static str| {
            let page = page.clone();
            async move { page.evaluate(format!("(function () {{ {} }})()", element_in_iframe_script(css))).await.unwrap() }
        };
        assert_eq!(ask("#here").await, false, "an element of this document is not in a frame");
        assert_eq!(ask("#inner").await, true);
        assert_eq!(ask("#deep").await, true, "frames are searched at any depth");
        assert_eq!(ask("#nowhere").await, false);
        browser.close().await.unwrap();
    }
}
