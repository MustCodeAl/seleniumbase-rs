# sbrs install

> Download a browser driver or a browser into `./downloaded_drivers/`. The driver's name is required.
> Also available as `sbrs get`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Download the chromedriver that matches the installed Chrome:

`sbrs install chromedriver`

- Download the driver for a browser (`firefoxdriver` and `msedgedriver` are accepted as aliases):

`sbrs install {{chromedriver|geckodriver|edgedriver}}`

- Download the chromedriver patched for the UC mode:

`sbrs install uc_driver`

- Download the newest chromedriver:

`sbrs install chromedriver latest`

- Download a chromedriver for a major Chrome version:

`sbrs install chromedriver {{major_version}}`

- Download a specific geckodriver version:

`sbrs install geckodriver {{0.36.0}}`

- Download a browser (Chrome for Testing, chrome-headless-shell or a Chromium snapshot):

`sbrs install {{cft|chs|chromium}}`

- Download a driver and also copy it to `$SB_DRIVER_PATH_DIR`, else `/usr/local/bin`:

`sbrs install {{[-p|--path]}} chromedriver`
