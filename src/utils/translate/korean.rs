pub fn translate(action: &str) -> String {
    match action {
        "open" => "열기",
        "click" => "클릭",
        "type" => "입력",
        "assert" => "검증",
        "wait" => "대기",
        "hover" => "호버",
        "scroll" => "스크롤",
        "refresh" => "새로고침",
        "go_back" => "뒤로",
        "go_forward" => "앞으로",
        "close" => "닫기",
        "submit" => "제출",
        "clear" => "지우기",
        "select" => "선택",
        _ => action,
    }
    .to_string()
}
