pub fn translate(action: &str) -> String {
    match action {
        "open" => "開く",
        "click" => "クリック",
        "type" => "入力",
        "assert" => "検証",
        "wait" => "待機",
        "hover" => "ホバー",
        "scroll" => "スクロール",
        "refresh" => "更新",
        "go_back" => "戻る",
        "go_forward" => "進む",
        "close" => "閉じる",
        "submit" => "送信",
        "clear" => "クリア",
        "select" => "選択",
        _ => action,
    }
    .to_string()
}
