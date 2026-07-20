# CLI Usage (`sbase`)

The `sbase` binary provides quick commands for common tasks.

## Build the CLI

```bash
cd rust-port
cargo build --bin sbase
```

## Help

```bash
./target/debug/sbase --help
```

## Open a page

```bash
./target/debug/sbase open https://seleniumbase.io
```

## Run a smoke test with UC mode

```bash
./target/debug/sbase --uc smoke https://seleniumbase.io --title-contains SeleniumBase
```

## Execute a raw CDP command

```bash
./target/debug/sbase --cdp cdp --cmd Browser.getVersion
./target/debug/sbase --cdp cdp --cmd Network.setCacheDisabled --params '{"cacheDisabled":true}'
```

## Save artifacts

```bash
./target/debug/sbase screenshot
./target/debug/sbase save-source
```

## Assertions and waits

```bash
./target/debug/sbase open https://seleniumbase.io
./target/debug/sbase assert-element --css "body"
./target/debug/sbase wait-for-text --css "body" --text "SeleniumBase" --timeout 15
```

## Patch chromedriver

```bash
./target/debug/sbase patch-chromedriver --path /path/to/chromedriver
```

## Run a JSON scenario

```bash
./target/debug/sbase run-scenario --file ./scenario.json
```

Example `scenario.json`:

```json
{
  "name": "basic_flow",
  "steps": [
    {"action": "open", "url": "https://seleniumbase.io"},
    {"action": "assert_element", "css": "body"},
    {"action": "wait_for_text", "css": "body", "text": "SeleniumBase", "timeout": 15}
  ]
}
```

## Generate files

```bash
./target/debug/sbase mkfile MyTest.rs
./target/debug/sbase mkdir my_tests
./target/debug/sbase mkpres MyPresentation
./target/debug/sbase mkchart MyChart
```
