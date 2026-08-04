pub fn translate(action: &str) -> String {
    match action {
        "open" => "abrir",
        "click" => "clic",
        "type" => "escribir",
        "assert" => "verificar",
        "wait" => "esperar",
        "hover" => "hover",
        "scroll" => "desplazar",
        "refresh" => "refrescar",
        "go_back" => "atrás",
        "go_forward" => "adelante",
        "close" => "cerrar",
        "submit" => "enviar",
        "clear" => "limpiar",
        "select" => "seleccionar",
        _ => action,
    }
    .to_string()
}
