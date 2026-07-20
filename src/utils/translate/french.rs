pub fn translate(action: &str) -> String {
    match action {
        "open" => "ouvrir",
        "click" => "cliquer",
        "type" => "saisir",
        "assert" => "vérifier",
        "wait" => "attendre",
        "hover" => "survoler",
        "scroll" => "défiler",
        "refresh" => "rafraîchir",
        "go_back" => "retour",
        "go_forward" => "avancer",
        "close" => "fermer",
        "submit" => "soumettre",
        "clear" => "effacer",
        "select" => "sélectionner",
        _ => action,
    }
    .to_string()
}
