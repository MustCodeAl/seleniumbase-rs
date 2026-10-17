# sbrs prompt

> Print one short line for a shell prompt: the installed Chrome's major version and whether a matching chromedriver is at hand.
> It starts no browser and uses no network. Nothing is printed when no browser is found.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Print the Chrome major version and the driver state, such as `chrome 155 ✓driver`:

`sbrs prompt`

- Mark the driver with `+`, `-`, and `?` instead of `✓`, `✗`, and `?`:

`sbrs prompt --ascii`

- Print a custom line (placeholders: `{browser}`, `{major}`, `{driver}`, `{mark}`, `{driver_major}`):

`sbrs prompt --format "{browser} {major}"`

- Report on a specific browser binary:

`sbrs prompt --browser-path {{path/to/browser}}`

- Run `--version` again instead of using the cached versions:

`sbrs prompt --no-cache`

- Explain on `stderr` what was found and how:

`sbrs prompt --verbose`
