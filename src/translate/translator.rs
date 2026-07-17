pub fn translate_action(action: &str, lang: &str) -> String {
    format!("{}_{}", action, lang)
}
