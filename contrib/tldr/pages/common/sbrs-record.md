# sbrs record

> Record browser interactions into a replayable action file.
> Recording stops on `<Ctrl c>`, after `--duration`, or when the browser is closed. See also: `sbrs replay`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/recorder_mode>.

- Open a page and record the interactions into `actions.json`:

`sbrs record {{https://example.com}}`

- Record into a given file (`.json` or `.yaml`):

`sbrs record {{https://example.com}} {{[-o|--output]}} {{path/to/actions.yaml}}`

- Stop recording after a number of seconds:

`sbrs record {{https://example.com}} --duration {{seconds}}`

- Record in a browser of a given kind:

`sbrs --browser {{chrome|chromium|edge|firefox|...}} record {{https://example.com}}`
