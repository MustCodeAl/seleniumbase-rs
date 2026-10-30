//! The table of Python `BaseCase` methods and the Rust calls that do the same.
//!
//! Every entry names the Python method (and its aliases), the Rust method, the
//! Python parameters in their order, and how each becomes a Rust argument. The
//! table is the one place to adjust when a `BaseCase` signature changes; the
//! tests check it against the real signatures (`src/api`) and against the
//! rows of `parity/api.toml`.

/// The kind of a Python parameter, and so of the Rust argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum K {
    /// A string (`&str`).
    Str,
    /// A whole number.
    Int,
    /// A number written as a float (`f64`).
    Float,
    /// A `bool` expression.
    Bool,
    /// A slice of strings (`&[&str]`).
    Strs,
    /// A frame: a string or a number (`impl Into<FrameRef>`).
    Frame,
    /// A window: a string or a number (`impl Into<WindowRef>`).
    Window,
    /// Python's `by=`: only the CSS selector strategy has an equal.
    By,
}

/// What happens to a Python parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Fill {
    /// Must be given; becomes a Rust argument.
    Required,
    /// Becomes a Rust argument; the literal is used when Python leaves it out.
    Default(&'static str),
    /// The remaining positional arguments, as one slice argument.
    Rest,
    /// Accepted and dropped with a warning: Rust has no such argument.
    Drop,
    /// Rust has no such argument: giving it is an error.
    Omit,
    /// A method of the call builder the Rust call returns: the template, with
    /// `{}` for the argument, is chained after the call
    /// (`sb.wait_for_element("#a").timeout(..).await?`).
    Chain(&'static str),
}

/// One Python parameter.
#[derive(Clone, Copy, Debug)]
pub(super) struct P {
    /// Its name in Python.
    pub(super) name: &'static str,
    /// Its kind.
    pub(super) kind: K,
    /// What happens to it.
    pub(super) fill: Fill,
}

/// How a Rust call is written.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Flow {
    /// `sb.method(args).await?`.
    Await,
    /// `sb.method(args).await`: it cannot fail.
    AwaitInfallible,
    /// `sb.method(args)?`: not async, it can fail.
    Try,
}

/// What a call gives back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Ret {
    /// Nothing.
    Unit,
    /// A `String`.
    Str,
    /// An `Option<String>`; the call is written with `.unwrap_or_default()`.
    OptStr,
    /// A `bool`.
    Bool,
    /// A `Vec<String>`.
    ListStr,
    /// The alert `switch_to_alert` returns.
    Alert,
    /// Something that is not used further.
    Other,
}

/// A Python method and its Rust equal.
#[derive(Clone, Copy, Debug)]
pub(super) struct Call {
    /// The Python names: the method and its aliases.
    pub(super) py: &'static [&'static str],
    /// The `BaseCase` method.
    pub(super) rust: &'static str,
    /// The Python parameters, in order.
    pub(super) params: &'static [P],
    /// How the call is written.
    pub(super) flow: Flow,
    /// What it gives back.
    pub(super) ret: Ret,
}

/// A required string.
pub(super) const fn s(name: &'static str) -> P {
    P {
        name,
        kind: K::Str,
        fill: Fill::Required,
    }
}

/// A string with a default.
pub(super) const fn sd(name: &'static str, default: &'static str) -> P {
    P {
        name,
        kind: K::Str,
        fill: Fill::Default(default),
    }
}

/// A required whole number.
pub(super) const fn n(name: &'static str) -> P {
    P {
        name,
        kind: K::Int,
        fill: Fill::Required,
    }
}

/// A required float.
pub(super) const fn f(name: &'static str) -> P {
    P {
        name,
        kind: K::Float,
        fill: Fill::Required,
    }
}

/// A required `bool` expression.
pub(super) const fn b(name: &'static str) -> P {
    P {
        name,
        kind: K::Bool,
        fill: Fill::Required,
    }
}

/// Python's `by=`.
pub(super) const BY: P = P {
    name: "by",
    kind: K::By,
    fill: Fill::Drop,
};

/// A named `by` (`hover_by`, `click_by`).
pub(super) const fn by_named(name: &'static str) -> P {
    P {
        name,
        kind: K::By,
        fill: Fill::Drop,
    }
}

/// A parameter Rust has no argument for: dropped with a warning.
pub(super) const fn drop(name: &'static str) -> P {
    P {
        name,
        kind: K::Int,
        fill: Fill::Drop,
    }
}

/// A parameter Rust has no argument for: giving it is an error.
pub(super) const fn omit(name: &'static str) -> P {
    P {
        name,
        kind: K::Int,
        fill: Fill::Omit,
    }
}

/// The rest of the positional arguments as a slice of strings.
pub(super) const fn rest(name: &'static str) -> P {
    P {
        name,
        kind: K::Strs,
        fill: Fill::Rest,
    }
}

/// A frame or window argument.
pub(super) const fn typed(name: &'static str, kind: K) -> P {
    P {
        name,
        kind,
        fill: Fill::Required,
    }
}

/// A parameter that is a method of the call builder.
pub(super) const fn chain(name: &'static str, kind: K, template: &'static str) -> P {
    P {
        name,
        kind,
        fill: Fill::Chain(template),
    }
}

/// The timeout of a call builder, in whole seconds.
pub(super) const CHAIN_TIMEOUT: P = chain(
    "timeout",
    K::Int,
    ".timeout(std::time::Duration::from_secs({}))",
);

/// The timeout of a call, dropped with a warning.
pub(super) const TIMEOUT: P = drop("timeout");

/// The timeout of an assertion: Rust waits the default timeout, so a
/// different one cannot be carried over.
pub(super) const ASSERT_TIMEOUT: P = omit("timeout");

/// The timeout of a wait, in seconds: Rust takes it as an argument.
pub(super) const WAIT_TIMEOUT: P = P {
    name: "timeout",
    kind: K::Int,
    fill: Fill::Default("10"),
};

pub(super) const fn call(
    py: &'static [&'static str],
    rust: &'static str,
    params: &'static [P],
    flow: Flow,
    ret: Ret,
) -> Call {
    Call {
        py,
        rust,
        params,
        flow,
        ret,
    }
}

/// An async call that can fail and gives back nothing.
const fn act(py: &'static [&'static str], rust: &'static str, params: &'static [P]) -> Call {
    call(py, rust, params, Flow::Await, Ret::Unit)
}

/// An async call that can fail and gives back a value.
const fn get(
    py: &'static [&'static str],
    rust: &'static str,
    params: &'static [P],
    ret: Ret,
) -> Call {
    call(py, rust, params, Flow::Await, ret)
}

/// The Python method `name` (or an alias of it) and its Rust call.
pub(super) fn lookup(name: &str) -> Option<&'static Call> {
    TABLE
        .iter()
        .chain(super::generated::GENERATED.iter())
        .find(|entry| entry.py.contains(&name))
}

/// Python's text selector: `html` when left out.
pub(super) const HTML: &str = "\"html\"";

/// The table.
pub(super) static TABLE: &[Call] = &[
    // Navigation and windows.
    act(&["open", "visit", "open_url"], "open", &[s("url")]),
    act(&["goto"], "goto", &[s("url")]),
    act(&["goto_if_not_url"], "goto_if_not_url", &[s("url")]),
    act(&["go_back"], "go_back", &[]),
    act(&["go_forward"], "go_forward", &[]),
    act(&["refresh", "refresh_page", "reload_page"], "refresh", &[]),
    act(&["maximize_window"], "maximize_window", &[]),
    get(&["get_title", "get_page_title"], "get_title", &[], Ret::Str),
    get(&["get_current_url"], "get_current_url", &[], Ret::Str),
    act(
        &["open_new_window"],
        "open_new_window",
        &[omit("switch_to")],
    ),
    act(
        &["switch_to_window"],
        "switch_to_window",
        &[typed("window", K::Window), TIMEOUT],
    ),
    act(&["switch_to_newest_window"], "switch_to_newest_window", &[]),
    act(
        &["switch_to_default_window"],
        "switch_to_default_window",
        &[],
    ),
    act(&["get_new_driver"], "get_new_driver", &[]),
    act(&["open_new_tab"], "open_new_tab", &[omit("switch_to")]),
    // Frames.
    act(
        &["switch_to_frame"],
        "switch_to_frame",
        &[typed("frame", K::Frame), TIMEOUT],
    ),
    act(
        &["switch_to_default_content"],
        "switch_to_default_content",
        &[],
    ),
    act(&["switch_to_parent_frame"], "switch_to_parent_frame", &[]),
    act(
        &["switch_to_frame_of_element"],
        "switch_to_frame_of_element",
        &[s("selector")],
    ),
    act(
        &["set_content_to_frame"],
        "set_content_to_frame",
        &[s("selector"), TIMEOUT],
    ),
    act(&["set_content_to_default"], "set_content_to_default", &[]),
    act(&["set_content_to_parent"], "set_content_to_parent", &[]),
    // Alerts.
    act(&["accept_alert"], "accept_alert", &[TIMEOUT]),
    act(&["dismiss_alert"], "dismiss_alert", &[TIMEOUT]),
    get(&["is_alert_present"], "is_alert_present", &[], Ret::Bool),
    get(
        &["switch_to_alert"],
        "switch_to_alert",
        &[TIMEOUT],
        Ret::Alert,
    ),
    // Actions on elements.
    act(
        &["click"],
        "click",
        &[s("selector"), BY, TIMEOUT, omit("delay"), omit("scroll")],
    ),
    act(
        &["double_click"],
        "double_click",
        &[s("selector"), BY, TIMEOUT],
    ),
    act(
        &["right_click", "context_click"],
        "context_click",
        &[s("selector"), BY, TIMEOUT],
    ),
    act(
        &["js_click"],
        "js_click",
        &[s("selector"), BY, omit("all_matches"), omit("scroll")],
    ),
    act(
        &["hover", "hover_on_element", "hover_over_element"],
        "hover",
        &[s("selector"), BY, TIMEOUT],
    ),
    act(
        &["hover_and_click"],
        "hover_and_click",
        &[
            s("hover_selector"),
            s("click_selector"),
            by_named("hover_by"),
            by_named("click_by"),
            TIMEOUT,
        ],
    ),
    act(
        &["hover_and_js_click"],
        "hover_and_js_click",
        &[
            s("hover_selector"),
            s("click_selector"),
            by_named("hover_by"),
            by_named("click_by"),
            TIMEOUT,
        ],
    ),
    act(
        &["drag_and_drop"],
        "drag_and_drop",
        &[
            s("drag_selector"),
            s("drop_selector"),
            by_named("drag_by"),
            by_named("drop_by"),
            TIMEOUT,
            omit("jquery"),
        ],
    ),
    // Python's `send_keys` is `add_text`: it does not clear the field.
    act(
        &["type", "update_text", "fill"],
        "type_text",
        &[s("selector"), s("text"), BY, TIMEOUT, omit("retry")],
    ),
    act(
        &["add_text", "send_keys"],
        "add_text",
        &[s("selector"), s("text"), BY, TIMEOUT],
    ),
    act(
        &["enter_mfa_code"],
        "enter_mfa_code",
        &[s("selector"), s("totp_key"), BY, TIMEOUT],
    ),
    act(&["clear"], "clear", &[s("selector"), BY, TIMEOUT]),
    act(&["submit"], "submit", &[s("selector"), BY]),
    act(
        &["set_value"],
        "set_value",
        &[s("selector"), s("text"), BY, TIMEOUT, omit("scroll")],
    ),
    act(
        &["press_keys"],
        "press_keys",
        &[s("selector"), s("text"), BY, TIMEOUT],
    ),
    act(
        &["select_option_by_text"],
        "select_option_by_text",
        &[
            s("dropdown_selector"),
            s("option"),
            by_named("dropdown_by"),
            TIMEOUT,
        ],
    ),
    act(
        &["select_option_by_value"],
        "select_option_by_value",
        &[
            s("dropdown_selector"),
            s("option"),
            by_named("dropdown_by"),
            TIMEOUT,
        ],
    ),
    act(
        &["select_option_by_index"],
        "select_option_by_index",
        &[
            s("dropdown_selector"),
            n("option"),
            by_named("dropdown_by"),
            TIMEOUT,
        ],
    ),
    act(
        &["check_if_unchecked"],
        "check_if_unchecked",
        &[s("selector"), BY],
    ),
    act(
        &["uncheck_if_checked"],
        "uncheck_if_checked",
        &[s("selector"), BY],
    ),
    act(
        &["click_visible_elements"],
        "click_visible_elements",
        &[s("selector"), BY, omit("limit"), TIMEOUT],
    ),
    act(
        &["click_link", "click_link_text"],
        "click_link",
        &[s("link_text"), TIMEOUT],
    ),
    act(
        &["click_partial_link", "click_partial_link_text"],
        "click_partial_link_text",
        &[s("link_text"), TIMEOUT],
    ),
    act(
        &["highlight"],
        "highlight",
        &[s("selector"), BY, omit("loops"), omit("scroll"), TIMEOUT],
    ),
    act(&["scroll_to"], "scroll_to", &[s("selector"), BY, TIMEOUT]),
    act(&["scroll_to_top"], "scroll_to_top", &[]),
    act(&["scroll_to_bottom"], "scroll_to_bottom", &[]),
    act(
        &["post_message"],
        "post_message",
        &[s("message"), omit("duration"), omit("pause"), omit("style")],
    ),
    act(&["activate_demo_mode"], "activate_demo_mode", &[]),
    act(&["deactivate_demo_mode"], "deactivate_demo_mode", &[]),
    act(&["clear_local_storage"], "clear_local_storage", &[]),
    act(&["clear_session_storage"], "clear_session_storage", &[]),
    act(&["delete_all_cookies"], "delete_all_cookies", &[]),
    act(
        &["wait_for_ready_state_complete"],
        "wait_for_ready_state_complete",
        &[omit("timeout")],
    ),
    // Assertions that wait.
    act(
        &["assert_element", "assert_element_visible"],
        "assert_element",
        &[s("selector"), BY, ASSERT_TIMEOUT],
    ),
    act(
        &["assert_element_present"],
        "assert_element_present",
        &[s("selector"), BY, ASSERT_TIMEOUT],
    ),
    act(
        &["assert_element_not_visible"],
        "assert_element_not_visible",
        &[s("selector"), BY, ASSERT_TIMEOUT],
    ),
    act(
        &["assert_text", "assert_text_visible"],
        "assert_text",
        &[s("text"), sd("selector", HTML), BY, ASSERT_TIMEOUT],
    ),
    act(
        &["assert_exact_text"],
        "assert_exact_text",
        &[s("text"), sd("selector", HTML), BY, ASSERT_TIMEOUT],
    ),
    act(
        &["assert_text_not_visible"],
        "assert_text_not_visible",
        &[s("text"), sd("selector", HTML), BY, ASSERT_TIMEOUT],
    ),
    act(
        &["assert_exact_text_not_visible"],
        "assert_exact_text_not_visible",
        &[s("text"), sd("selector", HTML), BY, ASSERT_TIMEOUT],
    ),
    act(&["assert_title"], "assert_title", &[s("title")]),
    act(
        &["assert_title_contains"],
        "assert_title_contains",
        &[s("substring")],
    ),
    act(&["assert_url"], "assert_url", &[s("url")]),
    act(
        &["assert_url_contains"],
        "assert_url_contains",
        &[s("substring")],
    ),
    act(
        &["assert_link_text"],
        "assert_link_text",
        &[s("link_text"), ASSERT_TIMEOUT],
    ),
    act(
        &["assert_attribute"],
        "assert_attribute",
        &[
            s("selector"),
            s("attribute"),
            s("value"),
            BY,
            ASSERT_TIMEOUT,
        ],
    ),
    act(
        &["assert_elements", "assert_elements_visible"],
        "assert_elements",
        &[rest("selectors"), ASSERT_TIMEOUT],
    ),
    act(
        &["assert_elements_present"],
        "assert_elements_present",
        &[rest("selectors"), ASSERT_TIMEOUT],
    ),
    // Deferred assertions.
    act(
        &["deferred_assert_element", "delayed_assert_element"],
        "deferred_assert_element",
        &[s("selector"), BY, ASSERT_TIMEOUT, omit("fs")],
    ),
    act(
        &["deferred_assert_text", "delayed_assert_text"],
        "deferred_assert_text",
        &[
            s("text"),
            sd("selector", HTML),
            BY,
            ASSERT_TIMEOUT,
            omit("fs"),
        ],
    ),
    act(
        &["deferred_assert_exact_text", "delayed_assert_exact_text"],
        "deferred_assert_exact_text",
        &[
            s("text"),
            sd("selector", HTML),
            BY,
            ASSERT_TIMEOUT,
            omit("fs"),
        ],
    ),
    act(
        &["process_deferred_asserts", "process_delayed_asserts"],
        "process_deferred_asserts",
        &[omit("print_only")],
    ),
    // Waits: Rust takes the timeout in seconds.
    act(
        &["wait_for_element"],
        "wait_for_element",
        &[s("selector"), BY, CHAIN_TIMEOUT],
    ),
    act(
        &["wait_for_element_visible"],
        "wait_for_element_visible",
        &[s("selector"), BY, WAIT_TIMEOUT],
    ),
    act(
        &["wait_for_element_present"],
        "wait_for_element_present",
        &[s("selector"), BY, WAIT_TIMEOUT],
    ),
    act(
        &["wait_for_element_absent"],
        "wait_for_element_absent",
        &[s("selector"), BY, WAIT_TIMEOUT],
    ),
    act(
        &["wait_for_element_not_visible"],
        "wait_for_element_not_visible",
        &[s("selector"), BY, WAIT_TIMEOUT],
    ),
    act(
        &["wait_for_text", "wait_for_text_visible"],
        "wait_for_text",
        &[s("text"), sd("selector", HTML), BY, WAIT_TIMEOUT],
    ),
    act(
        &["wait_for_exact_text", "wait_for_exact_text_visible"],
        "wait_for_exact_text",
        &[s("text"), sd("selector", HTML), BY, WAIT_TIMEOUT],
    ),
    // Reads.
    get(
        &["get_text"],
        "get_text",
        &[s("selector"), BY, TIMEOUT],
        Ret::Str,
    ),
    get(
        &["get_attribute"],
        "get_attribute",
        &[
            s("selector"),
            s("attribute"),
            BY,
            TIMEOUT,
            omit("hard_fail"),
        ],
        Ret::OptStr,
    ),
    get(
        &["get_select_options"],
        "get_select_options",
        &[
            s("dropdown_selector"),
            chain("attribute", K::Str, ".attribute({})"),
            by_named("dropdown_by"),
            CHAIN_TIMEOUT,
        ],
        Ret::ListStr,
    ),
    get(
        &["is_element_visible"],
        "is_element_visible",
        &[s("selector"), BY],
        Ret::Bool,
    ),
    get(
        &["is_element_present"],
        "is_element_present",
        &[s("selector"), BY],
        Ret::Bool,
    ),
    get(
        &["is_text_visible"],
        "is_text_visible",
        &[s("text"), sd("selector", HTML), BY],
        Ret::Bool,
    ),
    get(
        &["is_selected"],
        "is_selected",
        &[s("selector"), BY],
        Ret::Bool,
    ),
    get(
        &["is_checked"],
        "is_checked",
        &[s("selector"), BY],
        Ret::Bool,
    ),
    get(
        &["execute_script"],
        "execute_script",
        &[s("script")],
        Ret::Other,
    ),
    // Booleans and the end of a test.
    call(
        &["assert_true"],
        "assert_true",
        &[b("expr"), omit("msg")],
        Flow::Try,
        Ret::Unit,
    ),
    call(
        &["assert_false"],
        "assert_false",
        &[b("expr"), omit("msg")],
        Flow::Try,
        Ret::Unit,
    ),
    call(
        &["sleep"],
        "sleep",
        &[f("seconds")],
        Flow::AwaitInfallible,
        Ret::Unit,
    ),
    call(
        &["skip"],
        "skip_test",
        &[sd("reason", "\"\"")],
        Flow::Try,
        Ret::Unit,
    ),
    act(
        &["save_screenshot_to_logs"],
        "save_screenshot_to_logs",
        &[omit("name")],
    ),
    act(
        &["save_page_source_to_logs"],
        "save_page_source_to_logs",
        &[omit("name")],
    ),
    act(
        &["save_teardown_screenshot"],
        "save_teardown_screenshot",
        &[],
    ),
];
