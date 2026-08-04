pub fn translate(action: &str) -> String {
    match action {
        "open" => "open",
        "click" => "click",
        "type" => "type",
        "assert" => "assert",
        "wait" => "wait",
        "hover" => "hover",
        "scroll" => "scroll",
        "refresh" => "refresh",
        "go_back" => "go_back",
        "go_forward" => "go_forward",
        "close" => "close",
        "submit" => "submit",
        "clear" => "clear",
        "select" => "select",
        _ => action,
    }
    .to_string()
}
