pub fn translate(action: &str) -> String {
    match action {
        "open" => "открыть",
        "click" => "кликнуть",
        "type" => "ввести",
        "assert" => "проверить",
        "wait" => "ждать",
        "hover" => "навести",
        "scroll" => "прокрутить",
        "refresh" => "обновить",
        "go_back" => "назад",
        "go_forward" => "вперед",
        "close" => "закрыть",
        "submit" => "отправить",
        "clear" => "очистить",
        "select" => "выбрать",
        _ => action,
    }
    .to_string()
}
