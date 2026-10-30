---
title: "Tours, charts, recorder and utilities"
sidebarTitle: "Tours and utilities"
description: "Tours, presentations, charts, the recorder and other helpers."
---

<Note>

Methods are listed as `name(arguments)`, called on a `BaseCase` as `sb.name(...).await?`. The argument names, the selector syntax and the return types are explained in [Reading the API reference](/tutorials/api_reference#reading-this-page).

</Note>

## Tours, presentations, charts

| Method | Description |
|--------|-------------|
| `create_tour(name)` | Creates a tour with the default theme. |
| `create_tour_with_theme(name, theme)` | Creates a tour with a specific `TourTheme`. |
| `create_shepherd_tour(name)` / `create_introjs_tour(name)` / `create_driverjs_tour(name)` / `create_bootstrap_tour(name)` / `create_hopscotch_tour(name)` | Convenience themed constructors. |
| `add_tour_step(message, target)` | Adds a tour step; `target` is an optional CSS selector. |
| `play_tour()` / `start_tour()` | Plays the tour in the current page. |
| `export_tour(path)` | Exports tour HTML. |
| `create_presentation(name)` | Creates HTML presentation. |
| `add_presentation_slide(html)` | Adds a slide. |
| `save_presentation(path)` | Saves presentation. |
| `create_pie_chart(name)` / `create_bar_chart(name)` / ... | Creates a chart. |
| `add_data_point(label, value)` | Adds data to the current chart. |
| `save_chart(path)` | Saves chart HTML. |

See the [Tours](/tutorials/tours) and [Charts](/tutorials/charts) pages for details.

## Recorder

| Method | Description |
|--------|-------------|
| `activate_recorder()` | Injects the browser-side action recorder. |
| `recorded_actions()` | Returns actions captured so far. |
| `export_recording_as_rust()` | Returns recorded actions as Rust source (pure CDP code when `rec_sb_cdp()` is on). |
| `export_recording_as_cdp_rust()` | Returns recorded actions as a pure CDP program (`Page` and `Locator` calls). |
| `save_recording_to_logs()` | Saves the recording as JSON and Rust source. |
| `save_recording_json(filename)` | Saves recorded actions to a JSON file in the logs directory. |
| `recorder_breakpoint(message)` | Pauses the test so a person can do the manual part in the browser (Recorder Mode). |
| `wait_for_recorder_end(limit)` | Waits for SHIFT+ESC in the browser, for every window to close, or for `limit`. |
| `set_recording_target(stem, feature, scenario)` | Names the files Recorder Mode writes under `recordings/`. |
| `finish_recording()` | Writes the Recorder Mode files now; `quit` does it otherwise. |
| `save_recorded_actions()` | Python's: keeps what the page recorded in this tab, for a test that goes to another origin in the same tab. |

See [Recorder Mode](/tutorials/recorder_mode) for `sbrs mkrec`, the output formats and replay.

## Utility / misc helpers

| Method | Description |
|--------|-------------|
| `get_beautiful_soup()` | Returns a BeautifulSoup-style parser for the page. |
| `get_beautiful_soup_object()` | Alias for `get_beautiful_soup`. |
| `convert_css_to_xpath(css)` | Converts a CSS selector to an XPath expression. |
| `convert_xpath_to_css(xpath)` | Converts a simple XPath expression (a tag with attribute tests) to a CSS selector; any other XPath is an `InvalidSelector` error. |
| `ad_block()` | Removes the ad elements of upstream's `AD_BLOCK_LIST` from the current page. |
| `disable_beforeunload()` | Disables `beforeunload` prompts. |
| `print_unique_links_with_status_codes()` | Prints all unique links and their HTTP status codes. |
