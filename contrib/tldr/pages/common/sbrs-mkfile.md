# sbrs mkfile

> Create a browser-test file.
> Also available as `sbrs new`. The `.rs` extension is added when missing, and an existing file is never overwritten unless `--force` is given.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/cli_usage>.

- Create a test file that opens `https://example.com`:

`sbrs mkfile {{path/to/test_file.rs}}`

- Create a test file that opens a given page:

`sbrs mkfile {{path/to/test_file.rs}} --url {{https://example.com/login}}`

- Create a test file with an empty body that only opens the page:

`sbrs mkfile {{[-b|--basic]}} {{path/to/test_file.rs}}`

- Replace the file if it already exists:

`sbrs mkfile --force {{path/to/test_file.rs}}`
