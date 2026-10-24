# sbrs-mcp

> Serve a browser to Model Context Protocol clients over stdio or streamable HTTP.
> Part of `sbrs`. Logs go to `stderr`, and `stdout` carries only the protocol.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/mcp_server>.

- Serve the default toolset (`sb`, WebDriver with the broader `SB()` tools):

`sbrs-mcp`

- Serve the Pure CDP toolset (no WebDriver), the WebDriver `Driver()` toolset, or the development tools (no browser):

`sbrs-mcp --server {{cdp|driver|dev}}`

- Write screenshots, saved pages, PDFs, and downloads to a directory (default: `./mcp_output`):

`SB_MCP_OUTPUT_DIR={{path/to/directory}} sbrs-mcp --server {{cdp|driver|sb}}`

- Give every browser the server starts a built-in plugin, with an option as a TOML value (the operator chooses, the model cannot add one):

`sbrs-mcp --plugin {{shield}} --plugin-opt '{{shield.level}}="{{strict}}"'`

- Serve over streamable HTTP on loopback instead of stdio (port `0` picks a free port; the address is printed to `stderr`; each client session gets its own browser):

`sbrs-mcp --server {{cdp|driver|sb|dev}} --listen {{127.0.0.1:8931}}`

- Serve over HTTP on an address other machines can reach: a bearer token is required, read from the environment or from a file (never from the command line), and the name clients use must be allowed:

`SB_MCP_TOKEN={{secret}} sbrs-mcp --listen {{0.0.0.0:8931}} --allow-host {{mcp.example.com}}`

- Read the HTTP token from a file, mount the endpoint at another path, let a browser page from one origin in, and hold at most two sessions (browsers):

`sbrs-mcp --listen {{127.0.0.1:8931}} --token-file {{path/to/token}} --path {{/sbrs}} --allow-origin {{https://app.example.com}} --max-sessions {{2}}`


- Register the browser server (`seleniumbase`) and the development server (`seleniumbase-dev`) with MCP clients, by this program's absolute path:

`sbrs-mcp install {{claude-code|claude-desktop|codex|cursor|gemini-cli|opencode|vscode|zed}}`

- Show what `install` would write or run, and change nothing:

`sbrs-mcp install --print {{client1 client2 ...}}`

- Remove the registrations of the browser and development servers from MCP clients:

`sbrs-mcp uninstall {{client1 client2 ...}}`

- List the clients, where each keeps its config, whether it is on this machine, and what is registered:

`sbrs-mcp clients`
