pub fn get_ad_block_list() -> Vec<&'static str> {
    vec![
        "*://*.doubleclick.net/*",
        "*://*.googleadservices.com/*",
        "*://*.googlesyndication.com/*",
    ]
}
