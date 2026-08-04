pub fn translate(action: &str) -> String {
    match action {
        "open" => "aprire",
        "click" => "cliccare",
        "type" => "digitare",
        "assert" => "verificare",
        "wait" => "attendere",
        "hover" => "passare",
        "scroll" => "scorrere",
        "refresh" => "aggiornare",
        "go_back" => "indietro",
        "go_forward" => "avanti",
        "close" => "chiudere",
        "submit" => "inviare",
        "clear" => "cancellare",
        "select" => "selezionare",
        _ => action,
    }
    .to_string()
}
