pub fn translate(action: &str) -> String {
    match action {
        "open" => "打开",
        "click" => "点击",
        "type" => "输入",
        "assert" => "断言",
        "wait" => "等待",
        "hover" => "悬停",
        "scroll" => "滚动",
        "refresh" => "刷新",
        "go_back" => "返回",
        "go_forward" => "前进",
        "close" => "关闭",
        "submit" => "提交",
        "clear" => "清空",
        "select" => "选择",
        _ => action,
    }
    .to_string()
}
