// JavaScript library injection helpers.

impl BaseCase {
    /// Injects jQuery from a CDN into the current page.
    pub async fn activate_jquery(&self) -> Result<(), SeleniumBaseError> {
        self.add_js_link("https://code.jquery.com/jquery-3.7.1.min.js").await
    }

    /// Injects the jquery-confirm CSS and JS from a CDN.
    pub async fn activate_jquery_confirm(&self) -> Result<(), SeleniumBaseError> {
        self.add_css_link("https://cdnjs.cloudflare.com/ajax/libs/jquery-confirm/3.3.4/jquery-confirm.min.css").await?;
        self.add_js_link("https://cdnjs.cloudflare.com/ajax/libs/jquery-confirm/3.3.4/jquery-confirm.min.js").await
    }

    /// Injects the HubSpot Messenger CSS and JS from a CDN.
    pub async fn activate_messenger(&self) -> Result<(), SeleniumBaseError> {
        self.add_css_link("https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/css/messenger.min.css").await?;
        self.add_js_link("https://cdnjs.cloudflare.com/ajax/libs/messenger/1.5.0/js/messenger.min.js").await
    }

    /// Sets the jquery-confirm theme by adding a CSS class to `<body>`.
    pub async fn set_jqc_theme(&self, theme: &str) -> Result<(), SeleniumBaseError> {
        let script = format!(
            "document.body.classList.add('jqc-theme-{}');",
            js_escape(theme)
        );
        self.execute_script(&script).await?;
        Ok(())
    }

    /// Removes all jquery-confirm theme classes from `<body>`.
    pub async fn reset_jqc_theme(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(
            "document.body.classList.forEach(c => { if (c.startsWith('jqc-theme-')) document.body.classList.remove(c); });",
        )
        .await?;
        Ok(())
    }

    /// Injects a small CSS snippet to customize IntroJS highlight colors.
    pub async fn set_introjs_colors(&self, background: &str, border: &str) -> Result<(), SeleniumBaseError> {
        let css = format!(
            ".introjs-helperLayer {{ background: {} !important; border-color: {} !important; }}",
            background, border
        );
        self.add_css_style(&css).await
    }

    /// Injects a small CSS snippet to customize Messenger theme colors.
    pub async fn set_messenger_theme(&self, color: &str) -> Result<(), SeleniumBaseError> {
        let css = format!(
            ".messenger-message {{ background: {} !important; }}",
            color
        );
        self.add_css_style(&css).await
    }

    /// Disables the `beforeunload` confirmation dialog.
    pub async fn disable_beforeunload(&self) -> Result<(), SeleniumBaseError> {
        self.execute_script(
            "window.onbeforeunload = null; window.addEventListener('beforeunload', function(e) { e.preventDefault(); });",
        )
        .await?;
        Ok(())
    }

    /// Removes common advertising elements from the page.
    pub async fn ad_block(&self) -> Result<(), SeleniumBaseError> {
        let selectors = [
            "[id*='google_ads']",
            "[id*='ad-container']",
            ".ad",
            ".ads",
            ".advertisement",
            "iframe[src*='ads']",
            "iframe[src*='doubleclick']",
        ];
        let script = format!(
            "var selectors = {}; selectors.forEach(s => {{ document.querySelectorAll(s).forEach(el => el.remove()); }});",
            serde_json::to_string(&selectors).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))?
        );
        self.execute_script(&script).await?;
        Ok(())
    }
}
