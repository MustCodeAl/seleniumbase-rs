# sbrs plugin

> List the built-in browser plugins, or show what one declares.
> The built-ins are shield, block-resources, crx, fetcher, extractor, batcher, crawler, browser, imap and tor.
> Turn a plugin on with the global `--plugin` option, the `SB_PLUGINS` variable, or the `[plugins]` table of `sbase_config.toml`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/plugins/selecting>.

- List the plugins of this build, and whether they are on:

`sbrs plugin list`

- Show what a plugin declares: its category, capabilities, requirements, and options:

`sbrs plugin info {{shield}}`

- Run a command with a plugin (a global option goes before the command or after it):

`sbrs --plugin {{shield}} open {{https://example.com}}`

- Turn on several plugins:

`sbrs --plugin {{shield}} --plugin {{block-resources}} open {{https://example.com}}`

- Set an option of a plugin, with a TOML value (quote the whole option so the shell keeps the quotes):

`sbrs --plugin {{shield}} --plugin-opt '{{shield.level}}="{{strict}}"' open {{https://example.com}}`

- Turn on plugins through the environment:

`SB_PLUGINS={{shield,block-resources}} sbrs open {{https://example.com}}`

- Start a browser for DevTools MCP clients with a plugin:

`sbrs --plugin {{shield}} devtools`

- Run the task of a plugin (fetcher, extractor, batcher, crawler or imap) with its input from a JSON file, and write the results as JSON lines:

`sbrs plugin run {{crawler}} --input {{crawl.json}} --out {{pages.jsonl}}`

- Set an option of the plugin whose task runs, with a TOML value:

`sbrs --plugin-opt '{{crawler.max_pages}}={{50}}' plugin run {{crawler}} --input {{crawl.json}}`
