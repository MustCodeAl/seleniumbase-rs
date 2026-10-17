# sbrs devtools

> Launch Chrome with an open DevTools endpoint and print MCP client configs.
> The browser keeps running until `<Ctrl c>` closes it. See also: `sbrs-mcp`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/devtools_mcp>.

- Launch Chrome with a throwaway profile on port 9222, and print the endpoint and the configs:

`sbrs devtools`

- Launch the browser without a window:

`sbrs devtools --headless`

- Print the endpoint and the configs as one line of JSON:

`sbrs devtools --json`

- Launch the browser on a given port (`0` lets Chrome pick a free one):

`sbrs devtools --port {{port}}`

- Open a page once the browser runs:

`sbrs devtools --url {{https://example.com}}`

- Keep the browser profile in a directory:

`sbrs devtools --user-data-dir {{path/to/directory}}`

- Launch another Chromium browser, such as Microsoft Edge:

`sbrs devtools --browser-path {{path/to/browser}}`

- Launch the browser behind a proxy:

`sbrs devtools --proxy {{server:port}}`
