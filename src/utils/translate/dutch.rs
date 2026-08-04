pub fn translate(action: &str) -> String {
    match action {
        "open" => "openen",
        "click" => "klikken",
        "type" => "typen",
        "assert" => "beweren",
        "wait" => "wachten",
        "hover" => "hoveren",
        "scroll" => "scrollen",
        "refresh" => "vernieuwen",
        "go_back" => "terug",
        "go_forward" => "verder",
        "close" => "sluiten",
        "submit" => "indienen",
        "clear" => "wissen",
        "select" => "selecteren",
        _ => action,
    }
    .to_string()
}
