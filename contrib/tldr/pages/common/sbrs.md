# sbrs

> Automate browsers and run browser tests, a Rust port of SeleniumBase.
> Some subcommands such as `install`, `mkdir`, `mkrec`, etc. have their own usage documentation.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Open a URL in a browser:

`sbrs open {{https://example.com}}`

- Run a smoke test that checks the page title:

`sbrs smoke {{https://example.com}} --title-contains "{{Example}}"`

- Run a smoke test in a headless browser of a given kind:

`sbrs --headless --browser {{chrome|chromium|edge|firefox|...}} smoke {{https://example.com}}`

- Download the chromedriver that matches the installed Chrome:

`sbrs install chromedriver`

- Create a folder with a runnable browser-test suite:

`sbrs mkdir {{path/to/directory}}`

- Record browser interactions on a page into a test file:

`sbrs mkrec {{path/to/test_file.rs}} --url {{https://example.com}}`

- Replay a recorded action file in a fresh browser:

`sbrs replay {{recordings/test_file_rec.json}}`

- Check the environment and configuration for problems:

`sbrs doctor`
