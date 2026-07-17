# Syntax Formats

SeleniumBase Rust supports both direct API calls and JSON-based scenarios.

## Direct API
`sb.click("#my-button").await?;`

## JSON Scenario
`{"action": "click", "target": "#my-button"}`
