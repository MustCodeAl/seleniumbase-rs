pub fn get_recorder_js() -> &'static str {
    r#"
    // JS for action recorder
    window.recordedActions = [];
    document.addEventListener('click', function(e) {
        window.recordedActions.push({ type: 'click', target: e.target.tagName });
    });
    "#
}
