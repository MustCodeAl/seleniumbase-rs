# sbrs replay

> Replay a recorded action file against a fresh browser session.
> Exits with status 1 if a step fails. See also: `sbrs record`.
> More information: <https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/recorder_mode>.

- Replay an action file made by `sbrs record`:

`sbrs replay {{path/to/actions.json}}`

- Replay an action file in a headless browser:

`sbrs --headless replay {{path/to/actions.json}}`

- Replay an action file in a browser of a given kind:

`sbrs --browser {{chrome|chromium|edge|firefox|...}} replay {{path/to/actions.json}}`
