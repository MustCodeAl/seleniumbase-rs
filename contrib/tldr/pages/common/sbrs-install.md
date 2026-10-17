# sbrs install

> Download a browser driver into `./downloaded_drivers/`.
> Also available as `sbrs get`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Download the chromedriver that matches the installed Chrome:

`sbrs install`

- Download the driver for a browser (`firefoxdriver` and `msedgedriver` are accepted as aliases):

`sbrs install {{chromedriver|geckodriver|edgedriver}}`

- Download the newest chromedriver:

`sbrs install chromedriver latest`

- Download a chromedriver for a major Chrome version:

`sbrs install chromedriver {{major_version}}`

- Download a specific geckodriver version:

`sbrs install geckodriver {{0.36.0}}`
