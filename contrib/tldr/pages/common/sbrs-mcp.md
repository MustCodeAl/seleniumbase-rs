# sbrs-mcp

> Serve a browser to Model Context Protocol clients over stdio.
> Part of `sbrs`. Logs go to `stderr`, and `stdout` carries only the protocol.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/mcp_server>.

- Serve the default toolset (`sb`, WebDriver with the broader `SB()` tools):

`sbrs-mcp`

- Serve the Pure CDP toolset (Chrome or Chromium, no WebDriver):

`sbrs-mcp --server cdp`

- Serve the WebDriver `Driver()` toolset:

`sbrs-mcp --server driver`

- Write screenshots, saved pages, PDFs, and downloads to a directory (default: `./mcp_output`):

`SB_MCP_OUTPUT_DIR={{path/to/directory}} sbrs-mcp --server {{cdp|driver|sb}}`

- Wait a number of seconds for the browser to close on shutdown (default: 30):

`SB_SHUTDOWN_TIMEOUT_SECS={{seconds}} sbrs-mcp`

- Give every browser the server starts a built-in plugin (the operator chooses, the model cannot add one):

`sbrs-mcp --server {{cdp|driver|sb}} --plugin {{shield}}`

- Set an option of a plugin, with a TOML value:

`sbrs-mcp --plugin {{shield}} --plugin-opt '{{shield.level}}="{{strict}}"'`

- Display help:

`sbrs-mcp --help`

- Display version:

`sbrs-mcp --version`
