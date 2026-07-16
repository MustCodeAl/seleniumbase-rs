use seleniumbase_rs::scenario::Scenario;

#[test]
fn scenario_json_parses() {
    let data = r##"
    {
      "name": "basic",
      "steps": [
        {"action":"open","url":"https://example.com"},
        {"action":"click","css":"#go"},
        {"action":"type_text","css":"#q","text":"hello"},
        {"action":"assert_element","css":"body"},
        {"action":"sleep","seconds":0.2}
      ]
    }
    "##;
    let parsed: Scenario = serde_json::from_str(data).expect("scenario should parse");
    assert_eq!(parsed.name, "basic");
    assert_eq!(parsed.steps.len(), 5);
}
