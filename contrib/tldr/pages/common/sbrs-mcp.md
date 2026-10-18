# sbrs-mcp

> Serve a browser to Model Context Protocol clients over stdio.
> Part of `sbrs`. Logs go to `stderr`, and `stdout` carries only the protocol.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/mcp_server>.

- Serve the default toolset (`sb`, WebDriver with the broader `SB()` tools):

`sbrs-mcp`

- Serve the Pure CDP toolset (Chrome or Chromium, no WebDriver) or the WebDriver `Driver()` toolset:

`sbrs-mcp --server {{cdp|driver}}`

- Write screenshots, saved pages, PDFs, and downloads to a directory (default: `./mcp_output`):

`SB_MCP_OUTPUT_DIR={{path/to/directory}} sbrs-mcp --server {{cdp|driver|sb}}`

- Give every browser the server starts a built-in plugin, with an option as a TOML value (the operator chooses, the model cannot add one):

`sbrs-mcp --plugin {{shield}} --plugin-opt '{{shield.level}}="{{strict}}"'`

- Register the browser server (`seleniumbase`) and the development server (`seleniumbase-dev`) with MCP clients, by this program's absolute path:

`sbrs-mcp install {{claude-code|claude-desktop|codex|cursor|gemini-cli|opencode|vscode|zed}}`

- Show what `install` would write or run, and change nothing:

`sbrs-mcp install --print {{client1 client2 ...}}`

- Remove the registrations of the browser and development servers from MCP clients:

`sbrs-mcp uninstall {{client1 client2 ...}}`

- List the clients, where each keeps its config, whether it is on this machine, and what is registered:

`sbrs-mcp clients`
