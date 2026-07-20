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
        let path = dir.join(filename);
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
        let escaped = js_escape(html);
        self.execute_script(&format!("document.body.innerHTML = '{}';", escaped))
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
            "document.querySelectorAll('{}').forEach(el => el.remove());",
            js_escape(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Hides all elements matching `css`.
    pub async fn hide_elements(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelectorAll('{}').forEach(el => el.style.display = 'none');",
            js_escape(css)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Shows all elements matching `css`.
    pub async fn show_elements(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.querySelectorAll('{}').forEach(el => el.style.display = '');",
            js_escape(css)
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
            "document.querySelectorAll('{}').forEach(el => el.removeAttribute('{}'));",
            js_escape(css),
            js_escape(attribute)
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
            "document.querySelectorAll('{}').forEach(el => el.setAttribute('{}', '{}'));",
            js_escape(css),
            js_escape(attribute),
            js_escape(value)
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
            serde_json::to_string(href).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Injects a JavaScript script link into `<head>`.
    pub async fn add_js_link(&self, src: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var s=document.createElement('script'); s.src={}; document.head.appendChild(s);",
            serde_json::to_string(src).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Injects raw CSS into `<head>`.
    pub async fn add_css_style(&self, css: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var s=document.createElement('style'); s.textContent={}; document.head.appendChild(s);",
            serde_json::to_string(css).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Injects raw JavaScript into `<head>`.
    pub async fn add_js_code(&self, js: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var s=document.createElement('script'); s.textContent={}; document.head.appendChild(s);",
            serde_json::to_string(js).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Fetches JavaScript from `url` and injects it into the page.
    pub async fn add_js_code_from_link(&self, url: &str) -> Result<(), SeleniumBaseError> {
        self.add_js_link(url).await
    }

    /// Injects a `<meta>` tag into `<head>`.
    pub async fn add_meta_tag(&self, name: &str, content: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "var m=document.createElement('meta'); m.name={}; m.content={}; document.head.appendChild(m);",
            serde_json::to_string(name).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?,
            serde_json::to_string(content).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?
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

    /// Alias for `set_text`.
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
            "return document.querySelector('{}')?.parentElement;",
            js_escape(css)
        );
        let result = self.execute_script(&script).await?;
        self.element_from_script_result(result).await
    }

    /// Returns the origin of the current URL.
    pub async fn get_origin(&mut self) -> Result<String, SeleniumBaseError> {
        let url = self.get_current_url().await?;
        let parsed = reqwest::Url::parse(&url)
            .map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?;
        Ok(format!(
            "{}://{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or("")
        ))
    }

    /// Returns true if `css` is inside an `<iframe>` or `<frame>`.
    pub async fn is_element_in_an_iframe(&self, css: &str) -> Result<bool, SeleniumBaseError> {
        let script = format!(
            "var el = document.querySelector('{}'); return !!(el && el.closest('iframe, frame'));",
            js_escape(css)
        );
        match self.execute_script(&script).await? {
            Value::Bool(v) => Ok(v),
            _ => Ok(false),
        }
    }
}
