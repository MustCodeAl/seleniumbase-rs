//! Pure CDP tests: Python's `sb_cdp.Chrome()` and CDP Mode calls as `Page`
//! and `Locator` calls of the pure CDP engine ([`sb_cdp`](crate::sb_cdp)).
//!
//! The calls are the ones the action recorder writes for its pure CDP
//! programs (`src/api/recorder/cdp.rs`) and the ones the examples port
//! (`tests/examples_parity/cdp.rs`) uses, so they are known to compile.

use super::ast::{Arg, Expr};
use super::expr::{bind, compare_operand, Rendered};
use super::table::{by_named, n, omit, s, sd, Fill, ASSERT_TIMEOUT, BY, HTML, K, P, TIMEOUT};
use super::tr::{quote, Skip, TResult, Tr, Ty, Val};

/// A Python method and the pure CDP statement that does the same.
struct CdpCall {
    /// The Python names.
    py: &'static [&'static str],
    /// The Python parameters.
    params: &'static [P],
    /// The Rust expression; `{0}`, `{1}` are the arguments in order.
    code: &'static str,
    /// What it gives back.
    ty: Ty,
    /// The names it needs from `seleniumbase_rs::sb_cdp`.
    imports: &'static [&'static str],
}

/// A call that gives back nothing.
const fn unit(py: &'static [&'static str], params: &'static [P], code: &'static str) -> CdpCall {
    CdpCall {
        py,
        params,
        code,
        ty: Ty::Unit,
        imports: &[],
    }
}

/// A call that needs imports.
const fn importing(
    py: &'static [&'static str],
    params: &'static [P],
    code: &'static str,
    imports: &'static [&'static str],
) -> CdpCall {
    CdpCall {
        py,
        params,
        code,
        ty: Ty::Unit,
        imports,
    }
}

/// A call that gives back a value.
const fn value(
    py: &'static [&'static str],
    params: &'static [P],
    code: &'static str,
    ty: Ty,
) -> CdpCall {
    CdpCall {
        py,
        params,
        code,
        ty,
        imports: &[],
    }
}

/// The `selector, by, timeout` of an action on one element.
const ON: &[P] = &[s("selector"), BY, TIMEOUT];

/// The `selector, by, timeout` of an assertion on one element.
const ASSERT_ON: &[P] = &[s("selector"), BY, ASSERT_TIMEOUT];

/// The `text, selector, by, timeout` of a text assertion.
const TEXT_ON: &[P] = &[s("text"), sd("selector", HTML), BY, ASSERT_TIMEOUT];

/// The `dropdown_selector, option, dropdown_by, timeout` of a select call.
const SELECT: &[P] = &[
    s("dropdown_selector"),
    s("option"),
    by_named("dropdown_by"),
    TIMEOUT,
];

/// The table.
static CDP_TABLE: &[CdpCall] = &[
    unit(&["goto", "open", "visit", "open_url"], &[s("url")], "page.goto({0}).await?"),
    unit(&["go_back"], &[], "page.back().await?"),
    unit(&["go_forward"], &[], "page.forward().await?"),
    unit(&["refresh", "refresh_page", "reload_page"], &[], "page.reload().await?"),
    unit(&["maximize_window"], &[], "page.window().maximize().await?"),
    unit(&["click"], ON, "page.locator({0}).click().await?"),
    unit(
        &["double_click"],
        ON,
        "page.locator({0}).double_click().await?",
    ),
    unit(
        &["right_click", "context_click"],
        ON,
        "page.locator({0}).right_click().await?",
    ),
    unit(
        &["hover", "hover_on_element", "hover_over_element"],
        ON,
        "page.locator({0}).hover().await?",
    ),
    unit(&["clear"], ON, "page.locator({0}).clear().await?"),
    unit(&["submit"], &[s("selector"), BY], "page.locator({0}).submit().await?"),
    unit(&["scroll_to"], ON, "page.locator({0}).scroll_into_view().await?"),
    unit(
        &["highlight", "flash"],
        &[s("selector"), BY, omit("loops"), omit("scroll"), TIMEOUT],
        "page.locator({0}).flash().await?",
    ),
    unit(
        &["type", "update_text", "fill"],
        &[s("selector"), s("text"), BY, TIMEOUT, omit("retry")],
        "page.locator({0}).type_text({1}).await?",
    ),
    unit(
        &["add_text", "send_keys", "press_keys"],
        &[s("selector"), s("text"), BY, TIMEOUT],
        "page.locator({0}).send_keys({1}).await?",
    ),
    unit(
        &["set_value"],
        &[s("selector"), s("text"), BY, TIMEOUT, omit("scroll")],
        "page.locator({0}).set_value({1}).await?",
    ),
    unit(
        &["check_if_unchecked"],
        &[s("selector"), BY],
        "page.locator({0}).check().await?",
    ),
    unit(
        &["uncheck_if_checked"],
        &[s("selector"), BY],
        "page.locator({0}).uncheck().await?",
    ),
    importing(
        &["select_option_by_text"],
        SELECT,
        "page.locator({0}).select_option(SelectBy::Text({1})).await?",
        &["SelectBy"],
    ),
    importing(
        &["select_option_by_value"],
        SELECT,
        "page.locator({0}).select_option(SelectBy::Value({1})).await?",
        &["SelectBy"],
    ),
    importing(
        &["select_option_by_index"],
        &[s("dropdown_selector"), n("option"), by_named("dropdown_by"), TIMEOUT],
        "page.locator({0}).select_option(SelectBy::Index({1})).await?",
        &["SelectBy"],
    ),
    unit(
        &["drag_and_drop"],
        &[
            s("drag_selector"),
            s("drop_selector"),
            by_named("drag_by"),
            by_named("drop_by"),
            TIMEOUT,
            omit("jquery"),
        ],
        "page.locator({0}).drag_to(&page.locator({1})).await?",
    ),
    unit(
        &["hover_and_click"],
        &[
            s("hover_selector"),
            s("click_selector"),
            by_named("hover_by"),
            by_named("click_by"),
            TIMEOUT,
        ],
        "page.locator({0}).hover().await?; page.locator({1}).click().await?",
    ),
    unit(
        &["nested_click"],
        &[s("parent_selector"), s("selector"), BY, TIMEOUT],
        "page.locator({0}).locator({1}).click().await?",
    ),
    unit(
        &["click_visible_elements"],
        &[s("selector"), BY, omit("limit"), TIMEOUT],
        "for element in page.locator({0}).visible().all_waiting().await? { element.click().await?; }",
    ),
    unit(
        &["assert_element", "assert_element_visible"],
        ASSERT_ON,
        "page.locator({0}).expect().to_be_visible().await?",
    ),
    unit(
        &["assert_element_present"],
        ASSERT_ON,
        "page.locator({0}).expect().to_exist().await?",
    ),
    unit(
        &["assert_element_not_visible"],
        ASSERT_ON,
        "page.locator({0}).expect().not().to_be_visible().await?",
    ),
    unit(
        &["assert_text", "assert_text_visible"],
        TEXT_ON,
        "page.locator({1}).expect().to_contain_text({0}).await?",
    ),
    unit(
        &["assert_exact_text"],
        TEXT_ON,
        "page.locator({1}).expect().to_have_text({0}).await?",
    ),
    unit(
        &["assert_text_not_visible"],
        TEXT_ON,
        "page.locator({1}).expect().not().to_contain_text({0}).await?",
    ),
    unit(
        &["assert_exact_text_not_visible"],
        TEXT_ON,
        "page.locator({1}).expect().not().to_have_text({0}).await?",
    ),
    unit(
        &["assert_attribute"],
        &[s("selector"), s("attribute"), s("value"), BY, ASSERT_TIMEOUT],
        "page.locator({0}).expect().to_have_attribute({1}, {2}).await?",
    ),
    unit(
        &["assert_title"],
        &[s("title")],
        "page.expect().to_have_title({0}).await?",
    ),
    unit(
        &["assert_title_contains"],
        &[s("substring")],
        "page.expect().to_contain_title({0}).await?",
    ),
    unit(&["assert_url"], &[s("url")], "page.expect().to_have_url({0}).await?"),
    unit(
        &["assert_url_contains"],
        &[s("substring")],
        "page.expect().to_contain_url({0}).await?",
    ),
    importing(
        &["wait_for_element", "wait_for_element_visible"],
        ON,
        "page.locator({0}).wait_for(State::Visible).await?",
        &["State"],
    ),
    importing(
        &["wait_for_element_present"],
        ON,
        "page.locator({0}).wait_for(State::Present).await?",
        &["State"],
    ),
    importing(
        &["wait_for_element_absent"],
        ON,
        "page.locator({0}).wait_for(State::Absent).await?",
        &["State"],
    ),
    importing(
        &["wait_for_element_not_visible"],
        ON,
        "page.locator({0}).wait_for(State::Hidden).await?",
        &["State"],
    ),
    unit(
        &["wait_for_ready_state_complete"],
        &[omit("timeout")],
        "page.wait_for_load().await?",
    ),
    value(
        &["is_selected", "is_checked"],
        &[s("selector"), BY],
        "page.locator({0}).is_selected().await?",
        Ty::Bool,
    ),
    value(&["get_text"], ON, "page.locator({0}).text().await?", Ty::String),
    unit(&["clear_local_storage"], &[], "page.local_storage().clear().await?"),
    unit(&["delete_all_cookies"], &[], "page.cookies().clear().await?"),
    importing(&["scroll_to_top"], &[], "page.scroll(Scroll::Top).await?", &["Scroll"]),
    importing(
        &["scroll_to_bottom"],
        &[],
        "page.scroll(Scroll::Bottom).await?",
        &["Scroll"],
    ),
    unit(
        &["sleep"],
        &[P {
            name: "seconds",
            kind: K::Float,
            fill: Fill::Required,
        }],
        "tokio::time::sleep(std::time::Duration::from_secs_f64({0})).await",
    ),
];

/// `template` with each `{N}` replaced by the Nth argument.
fn fill_template(template: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut digits = String::new();
            while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                digits.push(*d);
                chars.next();
            }
            if !digits.is_empty() && chars.peek() == Some(&'}') {
                chars.next();
                if let Some(arg) = digits.parse::<usize>().ok().and_then(|i| args.get(i)) {
                    out.push_str(arg);
                    continue;
                }
            }
            out.push('{');
            out.push_str(&digits);
            continue;
        }
        out.push(c);
    }
    out
}

impl Tr<'_> {
    /// A call of a CDP Mode method (`sb.cdp.click(..)` or a method of
    /// `sb_cdp.Chrome()`).
    pub(super) fn cdp_call(&mut self, method: &str, args: &[Arg]) -> TResult<Rendered> {
        match method {
            // The fixture closes the browser.
            "quit" | "disconnect" | "stop" | "close_browser" => {
                return Ok(Rendered::plain("", Ty::Unit));
            }
            "activate_cdp_mode" => {
                // `activate_cdp_mode(url)` opens the page; without one the
                // fixture's page is the CDP page already.
                return match args.first() {
                    Some(Arg::Pos(url)) => {
                        let url = self.str_arg(url)?;
                        Ok(Rendered::awaiting(
                            format!("page.goto({url}).await?"),
                            Ty::Unit,
                        ))
                    }
                    _ => Ok(Rendered::plain("", Ty::Unit)),
                };
            }
            "assert_true" | "assert_false" | "assert_equal" | "assert_not_equal" | "assert_in"
            | "assert_not_in" => return self.cdp_assert(method, args),

            "click_link" | "click_link_text" => return self.cdp_link("link=", args),
            "click_partial_link" | "click_partial_link_text" => {
                return self.cdp_link("p_link=", args);
            }
            "find_element" | "select" | "query_selector" => {
                let selector = self.one_selector(method, args)?;
                return Ok(Rendered::plain(
                    format!("page.locator({selector})"),
                    Ty::Locator,
                ));
            }
            "find_elements" | "select_all" | "query_selector_all" => {
                let selector = self.one_selector(method, args)?;
                return Ok(Rendered::awaiting(
                    format!("page.locator({selector}).all_waiting().await?"),
                    Ty::List(Box::new(Ty::Locator)),
                ));
            }
            "open_new_tab" => {
                return Ok(Rendered::awaiting(
                    "let page = page.browser().new_page().await?",
                    Ty::Unit,
                ));
            }
            "switch_to_tab" => {
                let index = match args.first() {
                    Some(Arg::Pos(index)) => self.argument(K::Int, index)?,
                    _ => return Err(Skip::error("the 'tab' argument is missing")),
                };
                return Ok(Rendered::awaiting(
                    format!("let page = page.browser().pages().await?.remove({index})"),
                    Ty::Unit,
                ));
            }
            _ => {}
        }
        let Some(entry) = CDP_TABLE.iter().find(|entry| entry.py.contains(&method)) else {
            return Err(Skip::warn(format!(
                "SeleniumBase CDP method '{method}' needs manual migration"
            )));
        };
        let bound = bind(entry.params, args)?;
        let rust_args = self.rust_arguments(entry.params, &bound)?;
        self.imports.extend(entry.imports.iter().copied());
        let code = fill_template(entry.code, &rust_args);
        let awaits = code.contains(".await");
        Ok(Rendered {
            code,
            ty: entry.ty.clone(),
            awaits,
        })
    }

    /// `assert_true`, `assert_equal` and `assert_in` as Rust assertions: the
    /// pure CDP engine has no `sb` to call them on.
    fn cdp_assert(&mut self, method: &str, args: &[Arg]) -> TResult<Rendered> {
        let code = match method {
            "assert_true" | "assert_false" => {
                let [Arg::Pos(test)] = args else {
                    return Err(Skip::error(format!("{method} takes one value")));
                };
                let condition = self.condition(test)?;
                if method == "assert_true" {
                    format!("assert!({})", condition.code)
                } else {
                    format!("assert!(!{})", condition.operand())
                }
            }
            "assert_equal" | "assert_not_equal" => {
                let (first, second) = Self::two_values(args, method)?;
                let a = compare_operand(&self.expr(first)?)?;
                let b = compare_operand(&self.expr(second)?)?;
                let name = if method == "assert_equal" {
                    "assert_eq"
                } else {
                    "assert_ne"
                };
                format!("{name}!({a}, {b})")
            }
            "assert_in" | "assert_not_in" => {
                let (first, second) = Self::two_values(args, method)?;
                let found = self.membership(first, second, method == "assert_not_in")?;
                format!("assert!({})", found.code)
            }
            _ => return Err(Skip::warn(format!("{method} is not translated"))),
        };
        Ok(Rendered::plain(code, Ty::Unit))
    }

    /// The selector of `find_element(selector)`.
    fn one_selector(&mut self, method: &str, args: &[Arg]) -> TResult<String> {
        match args {
            [Arg::Pos(selector)] | [Arg::Pos(selector), Arg::Kw(..)] => self.str_arg(selector),
            _ => Err(Skip::error(format!("{method} takes one selector"))),
        }
    }

    /// A click on the link whose text is the argument; `prefix` is `link=` or
    /// `p_link=`.
    fn cdp_link(&mut self, prefix: &str, args: &[Arg]) -> TResult<Rendered> {
        let Some(Arg::Pos(text)) = args.first() else {
            return Err(Skip::error("the 'link_text' argument is missing"));
        };
        let selector = match text {
            Expr::Str(text) => quote(&format!("{prefix}{text}")),
            other => format!("&format!(\"{prefix}{{}}\", {})", self.str_arg(other)?),
        };
        Ok(Rendered::awaiting(
            format!("page.locator({selector}).click().await?"),
            Ty::Unit,
        ))
    }

    /// A method of a `Locator` held in a variable.
    pub(super) fn locator_method(
        &mut self,
        locator: &Val,
        method: &str,
        args: &[&Expr],
    ) -> TResult<Rendered> {
        let base = locator.operand();
        let none = |code: String| Ok(Rendered::awaiting(code, Ty::Unit));
        match (method, args) {
            ("click", []) => none(format!("{base}.click().await?")),
            ("hover", []) => none(format!("{base}.hover().await?")),
            ("double_click", []) => none(format!("{base}.double_click().await?")),
            ("clear", []) => none(format!("{base}.clear().await?")),
            ("flash" | "highlight", []) => none(format!("{base}.flash().await?")),
            ("type" | "type_text" | "fill", [text]) => {
                let text = self.str_arg(text)?;
                none(format!("{base}.type_text({text}).await?"))
            }
            ("send_keys" | "add_text", [text]) => {
                let text = self.str_arg(text)?;
                none(format!("{base}.send_keys({text}).await?"))
            }
            ("get_text", []) => Ok(Rendered::awaiting(
                format!("{base}.text().await?"),
                Ty::String,
            )),
            ("find_element" | "query_selector" | "select", [selector]) => {
                let selector = self.str_arg(selector)?;
                Ok(Rendered::plain(
                    format!("{base}.locator({selector})"),
                    Ty::Locator,
                ))
            }
            ("find_elements" | "query_selector_all" | "select_all", [selector]) => {
                let selector = self.str_arg(selector)?;
                Ok(Rendered::awaiting(
                    format!("{base}.locator({selector}).all_waiting().await?"),
                    Ty::List(Box::new(Ty::Locator)),
                ))
            }
            _ => Err(Skip::warn(format!(
                "the element method `.{method}()` is not translated"
            ))),
        }
    }
}
