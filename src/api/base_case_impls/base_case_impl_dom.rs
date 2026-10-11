// HTML / DOM helpers.

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

    /// Alias for `switch_to_default_content`.
    pub async fn set_content_to_default(&mut self) -> Result<(), SeleniumBaseError> {
        self.switch_to_default_content().await
    }

    /// Alias for `switch_to_default_content`.
    pub async fn set_content_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        self.switch_to_default_content().await
    }

    /// Alias for `switch_to_parent_frame`.
    pub async fn set_content_to_parent(&mut self) -> Result<(), SeleniumBaseError> {
        self.switch_to_parent_frame().await
    }

    /// Alias for `switch_to_parent_frame`.
    pub async fn set_content_to_parent_frame(&mut self) -> Result<(), SeleniumBaseError> {
        self.switch_to_parent_frame().await
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
