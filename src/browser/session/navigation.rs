//! Moving around: pages and history, frames, and windows and tabs.

use crate::utils::selectors::Selector;

use crate::error::SeleniumBaseError;

use super::BrowserSession;

impl BrowserSession {
    /// Navigates to `url`.
    ///
    /// On Firefox and Safari, the scripts browser plugins added for every
    /// document run again in the new one (feature `plugins`).
    pub async fn goto(&mut self, url: &str) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            self.leave_frames();

            return page.goto(url).await;
        }
        self.webdriver()?.goto(url).await?;
        #[cfg(feature = "plugins")]
        if let Some(plugins) = &self.plugins {
            plugins.after_navigation(self.webdriver()?).await?;
        }
        Ok(())
    }

    /// Navigates back in browser history.
    pub async fn back(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            self.leave_frames();
            return page.back().await;
        }
        self.webdriver()?.back().await?;
        Ok(())
    }

    /// Navigates forward in browser history.
    pub async fn forward(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            self.leave_frames();
            return page.forward().await;
        }
        self.webdriver()?.forward().await?;
        Ok(())
    }

    /// Reloads the current page.
    pub async fn refresh(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            self.leave_frames();
            return page.reload().await;
        }
        self.webdriver()?.refresh().await?;
        Ok(())
    }

    /// Returns the current page title.
    pub async fn current_title(&self) -> Result<String, SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page.title().await;
        }
        let title = self.webdriver()?.title().await?;
        Ok(title)
    }

    /// Returns the current page URL as a string.
    pub async fn current_url(&self) -> Result<String, SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page.url().await;
        }
        let url = self.webdriver()?.current_url().await?;
        Ok(url.as_str().to_owned())
    }

    /// Returns the full HTML source of the current page.
    pub async fn page_source(&self) -> Result<String, SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page.content().await;
        }
        Ok(self.webdriver()?.source().await?)
    }

    /// Switches context into the frame `selector` finds.
    pub async fn switch_to_frame(
        &mut self,
        selector: impl Into<Selector>,
    ) -> Result<(), SeleniumBaseError> {
        let element = self.find(selector).await?;
        self.enter_frame_element(&element).await
    }

    /// Switches context back to the top-level document.
    pub async fn switch_to_default_content(&mut self) -> Result<(), SeleniumBaseError> {
        if self.external_page.is_some() {
            self.leave_frames();
            return Ok(());
        }
        self.webdriver()?.enter_default_frame().await?;
        Ok(())
    }

    /// WebDriver interaction: `maximize_window`.
    pub async fn maximize_window(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            return page.window().maximize().await;
        }
        self.webdriver()?
            .maximize_window()
            .await
            .map_err(SeleniumBaseError::from)
    }

    /// WebDriver interaction: `set_window_size`.
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<(), SeleniumBaseError> {
        self.set_window_rect(0, 0, width, height).await
    }

    /// Returns the current window size as `(width, height)`.
    pub async fn window_size(&self) -> Result<(u32, u32), SeleniumBaseError> {
        let (_, _, width, height) = self.window_rect().await?;
        Ok((width as u32, height as u32))
    }

    /// Switches to the window/tab identified by `handle`.
    ///
    /// A session that blocks ads makes the window refuse the ad hosts too; see
    /// [`apply_ad_block`](Self::apply_ad_block).
    pub async fn switch_to_window(&self, handle: &str) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            let tab = page.browser().page(handle).await?;
            return self.switch_to_tab(tab).await;
        }
        let h = thirtyfour::common::types::WindowHandle::from(handle.to_string());
        self.webdriver()?.switch_to_window(h).await?;
        self.apply_mobile_emulation().await?;
        self.apply_ad_block().await
    }

    /// Opens and switches to a new browser window/tab.
    ///
    /// A session that blocks ads makes the new window refuse the ad hosts too;
    /// see [`apply_ad_block`](Self::apply_ad_block). Browser plugins (feature
    /// `plugins`) give it their packages and `on_page_created`.
    pub async fn switch_to_new_window(&self) -> Result<(), SeleniumBaseError> {
        if let Some(page) = self.external_page.get() {
            let tab = page.browser().new_page().await?;
            return self.switch_to_tab(tab).await;
        }

        let handle = self
            .webdriver()?
            .new_window()
            .await
            .map_err(SeleniumBaseError::from)?;
        self.webdriver()?
            .switch_to_window(handle)
            .await
            .map_err(SeleniumBaseError::from)?;
        self.apply_ad_block().await?;
        self.apply_mobile_emulation().await?;
        #[cfg(feature = "plugins")]
        self.apply_plugins_to_window().await?;
        Ok(())
    }
}
