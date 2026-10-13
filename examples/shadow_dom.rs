//! Reach into shadow DOM with the `::shadow` piercing combinator.
//!
//! A normal selector stops at a shadow root, so `#submit` finds nothing when
//! the button lives inside `<my-app>`'s shadow tree. Writing `::shadow` between
//! the host and the selector inside it walks into the shadow root, and
//! `shadow_click`, `shadow_type`, `shadow_get_text` and `shadow_get_attribute`
//! accept such selectors, one `::shadow` for each root crossed:
//!
//! ```text
//! my-app ::shadow button#submit                 one shadow root
//! my-app ::shadow user-card ::shadow .badge     two shadow roots
//! ```
//!
//! Only **open** shadow roots can be entered. The example serves a page with
//! one, types into it, clicks inside it, reads text from two levels down, and
//! checks each result.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example shadow_dom
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{ensure, Site};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result};

const PAGE: &str = r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Shadow DOM</title></head>
<body>
  <my-app></my-app>
  <script>
    const app = document.querySelector('my-app').attachShadow({ mode: 'open' });
    app.innerHTML = `
      <label for="name">Name</label>
      <input id="name">
      <button id="submit" type="button">Greet</button>
      <p class="greeting"></p>
      <user-card></user-card>`;
    app.getElementById('submit').addEventListener('click', () => {
      app.querySelector('.greeting').textContent =
        'Hello, ' + app.getElementById('name').value + '!';
    });
    const card = app.querySelector('user-card').attachShadow({ mode: 'open' });
    card.innerHTML = '<span class="badge" data-role="admin">Administrator</span>';
  </script>
</body>
</html>
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve(&[("/", PAGE)])?.url("/");
    run_browser_test(BrowserConfig::default(), |sb| Box::pin(demo(sb, url))).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    // An ordinary selector cannot see inside the shadow root...
    ensure(
        !sb.is_element_present("#submit").await?,
        "#submit is hidden from a normal selector",
    )?;

    // ...but the piercing selector can.
    sb.shadow_type("my-app ::shadow #name", "SeleniumBase")
        .await?;
    sb.shadow_click("my-app ::shadow button#submit").await?;

    let greeting = sb.shadow_get_text("my-app ::shadow .greeting").await?;
    println!("greeting: {greeting}");
    ensure(
        greeting == "Hello, SeleniumBase!",
        "the greeting names the visitor",
    )?;

    // Two roots deep.
    let badge = sb
        .shadow_get_text("my-app ::shadow user-card ::shadow .badge")
        .await?;
    let role = sb
        .shadow_get_attribute("my-app ::shadow user-card ::shadow .badge", "data-role")
        .await?;
    println!("badge: {badge} (role {role:?})");
    ensure(
        badge == "Administrator" && role.as_deref() == Some("admin"),
        "the nested badge is readable",
    )?;
    Ok(())
}
