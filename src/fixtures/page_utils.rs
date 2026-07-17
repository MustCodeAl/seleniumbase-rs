pub fn is_xpath(selector: &str) -> bool {
    selector.starts_with("/") || selector.starts_with("./")
}
