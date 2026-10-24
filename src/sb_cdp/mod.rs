//! Pure CDP Mode: drive Chrome over the DevTools Protocol, with no WebDriver.
//!
//! A [`Browser`] launches Chrome directly, so there is no chromedriver and none
//! of the markers WebDriver leaves behind. The pieces nest the way a browser
//! does:
//!
//! * [`Browser`] owns the process and its tabs, and holds browser-wide settings
//!   such as permissions and the download directory.
//! * [`Page`] is one tab. It navigates, evaluates scripts, captures the screen,
//!   and hands out the handles below.
//! * [`Locator`] finds elements lazily and waits for them before it acts, so it
//!   keeps working when the page re-renders. Narrow it with
//!   [`nth`](Locator::nth), [`visible`](Locator::visible) and
//!   [`locator`](Locator::locator).
//! * [`Mouse`] and [`Keyboard`] deliver trusted input events (`isTrusted`).
//! * [`Cookies`], [`Storage`], [`Window`] and [`Emulation`] cover the state and
//!   environment attached to a tab.
//! * [`LocatorExpect`] and [`PageExpect`] are assertions that retry until they
//!   hold.
//! * [`HarRecorder`] records a tab's network traffic as a HAR file
//!   ([`Page::start_har`]).
//!
//! Selectors are plain strings classified the way SeleniumBase classifies
//! them: `"#id"` is CSS, `"//div"` is XPath, `"link=Home"` is a link. Use
//! [`SelectorBuf`](crate::utils::selectors::SelectorBuf) to state the kind
//! explicitly.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
//! let page = browser.default_page().await?;
//!
//! page.goto("https://seleniumbase.io/simple/login").await?;
//! page.locator("#username").fill("demo_user").await?;
//! page.locator("#password").fill("secret_pass").await?;
//! page.locator("button").click().await?;
//! page.locator("h1").expect().to_contain_text("Welcome").await?;
//!
//! browser.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Testing without a browser
//!
//! With the `test-util` feature, [`Browser::new_mocked`] returns a browser
//! backed by a scripted [`MockCtrl`], so code that drives pages can be tested
//! without launching Chrome.
//!
//! # Coming from SeleniumBase
//!
//! The Python `sb.cdp` API is a flat set of methods that each take a selector.
//! Here the selector lives in a [`Locator`], so `sb.cdp.click(sel)` is
//! `page.locator(sel).click()`, and `click_if_visible`, `click_nth_element` and
//! the like are compositions such as `page.locator(sel).visible().first()`.
//! The full mapping is in the parity table in the documentation.

mod breakpoint;
mod browser;
mod captcha;
mod capture;
mod client;
mod context;
mod download;
mod endpoint;
mod expect;
#[cfg(feature = "gui")]
pub mod gui_captcha;
mod har;
mod human;
mod identity;
mod inject;
mod input;
mod intercept;
mod launch;
mod locator;
pub(crate) mod messenger;
#[cfg(any(test, feature = "test-util"))]
mod mock;
mod page;
#[cfg(feature = "plugins")]
pub(crate) mod plugin_hooks;
#[cfg(feature = "plugins")]
mod plugin_network;
mod pool;
mod proxy_auth;
mod session_store;
mod state;
pub(crate) mod sync;
mod types;
mod webrtc;

#[doc(inline)]
pub use browser::Browser;
#[doc(inline)]
pub use captcha::Captcha;
#[doc(inline)]
pub use client::{CdpEvent, Events};
#[doc(inline)]
pub use context::{BrowserContext, ContextOptions, ContextRoute};
#[doc(inline)]
pub use endpoint::{DevToolsEndpoint, CHROME_DEVTOOLS_MCP_VERSION, PLAYWRIGHT_MCP_VERSION};
#[doc(inline)]
pub use expect::{LocatorExpect, PageExpect};
#[doc(inline)]
pub use har::{
    Har, HarCache, HarContent, HarCookie, HarCreator, HarEntry, HarLog, HarNameValue, HarOptions,
    HarPage, HarPageTimings, HarPostData, HarRecorder, HarRequest, HarResponse, HarTimings,
    DEFAULT_MAX_CONTENT_BYTES, MAX_CONTENT_BYTES,
};
#[doc(inline)]
pub use human::Human;
#[doc(inline)]
pub use inject::{InitScriptId, TagSource};
#[doc(inline)]
pub use input::{Button, Key, Keyboard, Mouse};
#[doc(inline)]
pub use intercept::{Interception, Outcome, Request, ResourceType, Response, Rule, Seen};
#[doc(inline)]
pub use launch::{LaunchOptions, LaunchOptionsBuilder, Proxy};
#[doc(inline)]
pub use locator::Locator;
#[doc(inline)]
pub use messenger::{
    MessageOptions, MessageStyle, MessengerLocation, MessengerSettings, MessengerTheme,
};
#[cfg(any(test, feature = "test-util"))]
#[doc(inline)]
pub use mock::{Call, MockCtrl};
pub(crate) use page::without_final_return;
#[doc(inline)]
pub use page::Page;
#[doc(inline)]
pub use pool::{BrowserPool, Lease, PoolOptions, PoolOptionsBuilder, PoolStats};
#[doc(inline)]
pub use session_store::{Session, SessionStore};
#[doc(inline)]
pub use state::{
    ColorScheme, Cookie, Cookies, Emulation, SameSite, Storage, StorageArea, Window, WindowBounds,
    WindowState,
};
#[doc(inline)]
pub use types::{
    ElementInfo, PageInfo, Permission, Point, Rect, ScreenRect, Scroll, SelectBy, State,
};
/// The Chrome switch for a WebRTC policy, for the WebDriver engine's plugin
/// hooks, which pass it as a launch argument.
#[cfg(feature = "plugins")]
pub(crate) use webrtc::chrome_flag as webrtc_chrome_flag;
#[doc(inline)]
pub use webrtc::{AddressKind, CandidateKind, IceCandidate, WebRtcReport};
