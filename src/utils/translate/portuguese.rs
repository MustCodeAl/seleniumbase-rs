pub fn translate(action: &str) -> String {
    match action {
        "open" => "abrir",
        "click" => "clicar",
        "type" => "digitar",
        "assert" => "verificar",
        "wait" => "esperar",
        "hover" => "pairar",
        "scroll" => "rolar",
        "refresh" => "atualizar",
        "go_back" => "voltar",
        "go_forward" => "avançar",
        "close" => "fechar",
        "submit" => "enviar",
        "clear" => "limpar",
        "select" => "selecionar",
        _ => action,
    }
    .to_string()
}
