pub fn translate_action(action: &str, lang: &str) -> String {
    match lang.to_lowercase().as_str() {
        "zh" | "chinese" => crate::utils::translate::chinese::translate(action),
        "nl" | "dutch" => crate::utils::translate::dutch::translate(action),
        "fr" | "french" => crate::utils::translate::french::translate(action),
        "it" | "italian" => crate::utils::translate::italian::translate(action),
        "ja" | "japanese" => crate::utils::translate::japanese::translate(action),
        "ko" | "korean" => crate::utils::translate::korean::translate(action),
        "pt" | "portuguese" => crate::utils::translate::portuguese::translate(action),
        "ru" | "russian" => crate::utils::translate::russian::translate(action),
        "es" | "spanish" => crate::utils::translate::spanish::translate(action),
        "en" | "english" => crate::utils::translate::english::translate(action),
        _ => action.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_in_chinese() {
        assert_eq!(translate_action("click", "zh"), "点击");
    }

    #[test]
    fn type_in_spanish() {
        assert_eq!(translate_action("type", "es"), "escribir");
    }

    #[test]
    fn unknown_action_passes_through() {
        assert_eq!(translate_action("custom", "en"), "custom");
    }

    #[test]
    fn default_language_is_english() {
        assert_eq!(translate_action("click", "xx"), "click");
    }
}
