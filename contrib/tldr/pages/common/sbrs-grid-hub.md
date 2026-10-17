# sbrs grid-hub

> Start, stop, restart, or check a Selenium Grid hub.
> Needs a Selenium Server jar that you supply, as nothing is downloaded. See also: `sbrs grid-node`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Start a hub with the jar named by the `$SB_SELENIUM_SERVER_JAR` environment variable:

`sbrs grid-hub start`

- Start a hub with a specific Selenium Server jar:

`sbrs grid-hub start --jar {{path/to/selenium-server.jar}}`

- Start a hub that listens on a given address and port:

`sbrs grid-hub start --grid-host {{host}} --grid-port {{port}}`

- Start a hub, waiting longer for it to become ready, and close sessions that have been idle for a while:

`sbrs grid-hub start --wait {{seconds}} --session-timeout {{seconds}}`

- Start a hub and pass more arguments to the Selenium Server:

`sbrs grid-hub start -- {{extra_arguments}}`

- Check whether the hub is running and ready:

`sbrs grid-hub status`

- Stop the hub:

`sbrs grid-hub stop`

- Restart the hub:

`sbrs grid-hub restart`
