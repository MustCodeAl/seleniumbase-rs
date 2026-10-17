# sbrs mkdir

> Create a folder with a runnable browser-test suite.
> Nothing is overwritten unless `--force` is given.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Create a folder with a test suite and example tests:

`sbrs mkdir {{path/to/directory}}`

- Create only the scaffolding, with no example tests:

`sbrs mkdir {{[-b|--basic]}} {{path/to/directory}}`

- Replace generated files that already exist:

`sbrs mkdir --force {{path/to/directory}}`
