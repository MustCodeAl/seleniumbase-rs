use seleniumbase_rs::recorder::ActionRecorder;

#[test]
fn recorder_generates_rust_script() {
    let mut recorder = ActionRecorder::default();
    recorder.record("open", Some("https://example.com"), None);
    recorder.record("click", Some("#submit"), None);
    recorder.record("type_text", Some("#name"), Some("Alice"));

    let script = recorder.to_rust_script();
    assert!(script.contains("sb.open(\"https://example.com\")"));
    assert!(script.contains("sb.click(\"#submit\")"));
    assert!(script.contains("sb.type_text(\"#name\", \"Alice\")"));
}
