# sbrs grid-node

> Start, stop, restart, or check a Selenium Grid node.
> A node registers with the hub named by `--hub`, or with the hub of this machine. See also: `sbrs grid-hub`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Start a node with the jar named by the `$SB_SELENIUM_SERVER_JAR` environment variable:

`sbrs grid-node start`

- Start a node and register it with a hub, given as a host or a URL:

`sbrs grid-node start --hub {{hub_host_or_url}}`

- Start a node with a specific Selenium Server jar:

`sbrs grid-node start --jar {{path/to/selenium-server.jar}}`

- Start a node that listens on a given port:

`sbrs grid-node start --grid-port {{port}}`

- Check whether the node is running and ready:

`sbrs grid-node status`

- Stop the node:

`sbrs grid-node stop`

- Restart the node:

`sbrs grid-node restart`
