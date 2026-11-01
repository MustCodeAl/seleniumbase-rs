---
title: "Upstream sync: SeleniumBase 4.55.2 to 4.56.5"
---

# Upstream sync: SeleniumBase 4.55.2 to 4.56.5

This crate tracks Python SeleniumBase 4.55.2 (`parity/upstream.toml`). The newest
release on GitHub is **4.56.5** (29 October 2026). This note is the delta from
tag `v4.55.2` (7 October 2026) through tag `v4.56.5`.

Source: the local checkout of [seleniumbase/SeleniumBase](https://github.com/seleniumbase/SeleniumBase),
`git diff v4.55.2 v4.56.5` (19 commits, 60 files, +542/−319). The release notes
are [v4.56.5](https://github.com/seleniumbase/SeleniumBase/releases/tag/v4.56.5),
[v4.56.4](https://github.com/seleniumbase/SeleniumBase/releases/tag/v4.56.4) and
[v4.56.3](https://github.com/seleniumbase/SeleniumBase/releases/tag/v4.56.3).
The compare view is
[v4.55.2...v4.56.5](https://github.com/seleniumbase/SeleniumBase/compare/v4.55.2...v4.56.5).

No BaseCase method was added or removed. No CDP-mode method was added or
removed. No CLI command was added or removed. No pytest, `SB()` or `Driver()`
option was added or removed. No MCP tool was added or removed (`mcp_servers/`
only bumps its package version). What changed is behaviour, one new method on
the `Driver` class, and dependency pins.

## BaseCase

| Change | What it does | Porting note |
| --- | --- | --- |
| `get_link_status_code(..., verify=True)` | The default of `verify` flipped from `False` to `True`. A `HEAD` of a link now checks the certificate unless the caller passes `verify=False`. The same default moved in `page_utils._get_link_status_code`. | `BaseCase::get_link_status_code` (`src/api/base_case_impls/base_case_impl_links.rs`) already verifies TLS: `link_check_client` builds a normal reqwest client. Rust has no `verify` argument. Already matches the new default. Effort **S** only if a caller must be able to skip certificate checks the way `verify=False` still can. |

## CDP mode

| Change | What it does | Porting note |
| --- | --- | --- |
| New tab is not appended to `driver.targets` | `CDPMethods` still opens the tab and appends it to `driver.tabs`, but no longer also appends it to `driver.targets`. That double record broke listing tabs ([#4533](https://github.com/seleniumbase/SeleniumBase/issues/4533), 4.56.4). | Check the pure CDP tab list in `src/sb_cdp` and the WebDriver CDP wrapper. A target recorded twice would show the same tab twice. Effort **S**. |
| `Element` highlight swallows `DOM.enable` errors | `highlight` now calls `cdp.dom.enable()` inside `suppress(Exception)` before the overlay. A thrown enable used to abort the highlight ([#4537](https://github.com/seleniumbase/SeleniumBase/issues/4537), 4.56.5). | If a Rust highlight sends `DOM.enable` and treats a failure as fatal, ignore that one error and continue with the overlay. `src/sb_cdp`. Effort **S**. |
| CDP fallback receives `timeout` | `page_actions` calls that run on a CDP driver (`click`, `click_if_visible`, `send_keys`, `press_keys`, `type`, `assert_element`, `assert_element_present`, `assert_text`, `assert_exact_text`, `select`, `find_element`, `get_text`, `wait_for_element_absent`, `wait_for_element_not_visible`) now pass the caller's `timeout`. They used to call the CDP method with no timeout. | Rust methods take the timeout on the call itself, so there is no second hop that was dropping it. Confirm any WebDriver path that delegates to `sb_cdp` forwards the same duration. `src/api`. Effort **S**. |

## `Driver()` / `SB()`

| Change | What it does | Porting note |
| --- | --- | --- |
| `DriverMethods.assert_element_absent` | New method. It calls `page_actions.assert_element_absent`, which waits until the selector is absent (`settings.SMALL_TIMEOUT` unless the caller passes `timeout`). `extend_driver` also sets `page.assert_element_absent` and `driver.assert_element_absent`. | `BaseCase::assert_element_absent` already exists (`src/api/base_case.rs`). The new Python surface is the `Driver`/`SB()` object, which this crate does not ship as a separate class. Nothing to add on `BaseCase`. Effort **S** to record the parity row as already implemented, not a new method. |

`page_actions.assert_element_absent` is new as a function. It recalculates the selector and calls `wait_for_element_absent`. It is the body of the `Driver` method above, not a new BaseCase method.

## CLI, pytest, MCP

No command, flag, pytest option, `SB()` option or `Driver()` option changed between the two tags. `setup.py` only moves dependency pins (Selenium 4.50.0 to 4.51.0, and the usual libraries). The MCP server's `pyproject.toml` version changes; its tool list does not.

## Other behaviour, not an API

- Missing or outdated chromedriver, uc_driver, geckodriver, msedgedriver and Chrome for Testing messages now start with `INFO:` instead of `Warning:`. Wording only. `src/browser` if we want the same tone. Effort **S**, optional.
- `decorators.print_runtime` with no description labels the block as `file:line` of the caller instead of `"Code Block"`. We have no `print_runtime`. Not applicable.
- 4.56.0 is "full Python 3.15 support" plus driver-method wiring. The language support does not port.

## Recommended port order

1. **Tab list (S).** Confirm a new CDP tab is recorded once. This is the bug 4.56.4 fixed.
2. **Highlight `DOM.enable` (S).** Ignore a failed enable and still highlight. This is the bug 4.56.5 fixed.
3. **CDP timeout forwarding (S).** Only where a WebDriver call delegates to CDP.
4. **Parity row for `Driver.assert_element_absent` (S).** Point it at `BaseCase::assert_element_absent`. Do not add a second method.
5. **`verify` on link checks (S, optional).** Add an argument only if a caller must skip certificate checks. The new Python default already matches Rust.

Do not port the dependency pins, the `INFO:` wording, or `print_runtime`.
