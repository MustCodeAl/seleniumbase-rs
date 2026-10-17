# sbrs

> Automate browsers and run browser tests, a Rust port of SeleniumBase.
> Some subcommands such as `install`, `mkdir`, `record`, etc. have their own usage documentation.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Open a URL in a browser:

`sbrs open {{https://example.com}}`

- Run a smoke test that checks the page title:

`sbrs smoke {{https://example.com}} --title-contains "{{Example}}"`

- Run a smoke test in a headless browser of a given kind:

`sbrs --headless --browser {{chrome|chromium|edge|firefox|...}} smoke {{https://example.com}}`

- Download the chromedriver that matches the installed Chrome:

`sbrs install`

- Create a folder with a runnable browser-test suite:

`sbrs mkdir {{path/to/directory}}`

- Record browser interactions on a page into an action file:

`sbrs record {{https://example.com}} {{[-o|--output]}} {{path/to/actions.json}}`

- Replay a recorded action file in a fresh browser:

`sbrs replay {{path/to/actions.json}}`

- Check the environment and configuration for problems:

`sbrs doctor`
