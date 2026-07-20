pub fn get_active_css_js() -> &'static str {
    r#"
    // JS injected for active CSS checking
    function isCssActive(selector) {
        return document.querySelector(selector) !== null;
    }
    "#
}
