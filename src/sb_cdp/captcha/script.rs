//! The page scripts of the CAPTCHA solver.
//!
//! Every script starts with one fixed JavaScript library and the solver's data
//! (selectors, response-field names, challenge markers) as JSON, so the data
//! lives in Rust where a test can see it and the script text never has to be
//! built by string splicing.
//!
//! The library walks the page the way a person looks at it: the document, the
//! open shadow roots in it, and the frames whose content the page may read
//! (a frame from another origin stays a box: its inside cannot be read, but the
//! frame element itself is found and measured). Positions are in viewport
//! coordinates, which is what mouse events use.

use serde_json::{json, Value};

use super::Captcha;

/// The JavaScript every script begins with. `__DATA__` becomes the JSON of
/// [`data`].
const LIBRARY: &str = r"
const DATA = __DATA__;

function* roots(root, depth) {
  yield root;
  if (depth > 6) return;
  for (const el of root.querySelectorAll('*')) {
    if (el.shadowRoot) yield* roots(el.shadowRoot, depth + 1);
    if (el.tagName === 'IFRAME') {
      let doc = null;
      try { doc = el.contentDocument; } catch (e) { doc = null; }
      if (doc && doc.documentElement) yield* roots(doc, depth + 1);
    }
  }
}
const ALL = [...roots(document, 0)];

function shown(el) {
  const view = el.ownerDocument.defaultView;
  if (!view) return false;
  const style = view.getComputedStyle(el);
  const r = el.getBoundingClientRect();
  return style.display !== 'none' && style.visibility !== 'hidden' && r.width > 0 && r.height > 0;
}

// The element's box in the top window's viewport, through the frames it is in.
function viewportRect(el) {
  const b = el.getBoundingClientRect();
  let x = b.left, y = b.top;
  let win = el.ownerDocument.defaultView;
  while (win && win !== win.top && win.frameElement) {
    const f = win.frameElement.getBoundingClientRect();
    x += f.left + win.frameElement.clientLeft;
    y += f.top + win.frameElement.clientTop;
    win = win.parent;
  }
  return { x, y, width: b.width, height: b.height };
}

// The invisible reCAPTCHA badge floats in a corner and has nothing to press.
function badge(el) {
  if (el.closest && el.closest('.grecaptcha-badge')) return true;
  const r = viewportRect(el);
  const w = window.innerWidth, h = window.innerHeight;
  return r.x > 1040 && r.y > 640 && Math.abs(w - r.x) < 110 && Math.abs(h - r.y) < 110;
}

function place(el, kind, scroll) {
  if (scroll) el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
  const r = viewportRect(el);
  const inView = r.x >= 0 && r.y >= 0 &&
    r.x + r.width <= window.innerWidth && r.y + r.height <= window.innerHeight;
  return { kind, x: r.x, y: r.y, width: r.width, height: r.height, inView };
}

// The first widget of the wanted kind (or of any kind when `only` is -1): kinds
// in their order, then the frames before the containers, then the page order.
function find(scroll, only) {
  for (let kind = 0; kind < DATA.kinds.length; kind++) {
    if (only >= 0 && kind !== only) continue;
    for (const tier of DATA.kinds[kind].tiers) {
      const selector = tier.join(',');
      for (const root of ALL) {
        for (const el of root.querySelectorAll(selector)) {
          if (!shown(el)) continue;
          if (kind === DATA.recaptcha && badge(el)) continue;
          return place(el, kind, scroll);
        }
      }
    }
  }
  // A challenge page whose widget has no name to find it by: the box of about
  // 300x65 pixels inside the challenge stage. Only Turnstile draws one.
  if (only <= 0 && challengeMarked()) {
    for (const root of ALL) {
      for (const stage of root.querySelectorAll(DATA.stage.join(','))) {
        for (const el of stage.querySelectorAll('div')) {
          if (!shown(el)) continue;
          const r = el.getBoundingClientRect();
          if (r.width >= 250 && r.width <= 420 && r.height >= 50 && r.height <= 90) {
            return place(el, 0, scroll);
          }
        }
      }
    }
  }
  return null;
}

// Whether the page carries the signs of a Cloudflare challenge.
function challengeMarked() {
  if (document.title.startsWith('Just a moment')) return true;
  const selector = DATA.markers.join(',');
  return ALL.some((root) => root.querySelector(selector));
}

// A challenge that is on its way: the page is marked, or a widget element
// exists and has not been given a size yet (the script is still drawing it).
function pending() {
  if (challengeMarked()) return true;
  for (const kind of DATA.kinds) {
    const selector = kind.tiers.flat().join(',');
    for (const root of ALL) {
      for (const el of root.querySelectorAll(selector)) {
        if (!shown(el) && !(el.closest && el.closest('.grecaptcha-badge'))) return true;
      }
    }
  }
  return false;
}
";

/// The data the library reads, as JSON text.
fn data() -> String {
    let kinds: Vec<Value> = Captcha::ALL
        .iter()
        .map(|kind| {
            json!({
                "tiers": kind.tiers(),
                "fields": kind.fields(),
                "success": kind.success_selectors(),
            })
        })
        .collect();
    let recaptcha = Captcha::ALL
        .iter()
        .position(|kind| *kind == Captcha::Recaptcha);
    let data = json!({
        "kinds": kinds,
        "recaptcha": recaptcha,
        "markers": MARKERS,
        "stage": STAGE,
    });
    serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_owned())
}

/// What marks a Cloudflare challenge page. The beacon script that most
/// Cloudflare sites carry (`/challenge-platform/.../jsd/`) is not here: only
/// the challenge's own `orchestrate` script and elements are.
pub(super) const MARKERS: [&str; 5] = [
    "#challenge-stage",
    "#challenge-form",
    r#"[id^="cf-chl-widget"]"#,
    r#"iframe[src*="challenges.cloudflare.com"]"#,
    r#"script[src*="/challenge-platform/"][src*="/orchestrate/"]"#,
];

/// Where a Turnstile widget with no recognisable name is looked for.
pub(super) const STAGE: [&str; 3] = [
    "#challenge-stage",
    "#challenge-form",
    r#"[data-testid*="challenge-"]"#,
];

/// `body` wrapped in the library, as an expression that evaluates to its
/// return value.
fn wrap(body: &str) -> String {
    let library = LIBRARY.replace("__DATA__", &data());
    format!("(() => {{ {library}\n{body}\n}})()")
}

/// Scrolls the first visible widget to the middle of the viewport and returns
/// `{ kind, x, y, width, height, inView }` for it, or `null`.
#[cfg(any(feature = "gui", test))]
pub(super) fn locate() -> String {
    wrap("return find(true, -1);")
}

/// Finds the widget (scrolling to it when `scroll`) and says whether a
/// challenge is on its way: `{ widget, pending }`.
pub(super) fn probe(scroll: bool) -> String {
    wrap(&format!(
        "return {{ widget: find({scroll}, -1), pending: pending() }};"
    ))
}

/// Reads the signs of a result for the widget of kind index `kind`: the
/// response fields, a success state the page may read, whether the widget is
/// still shown, and which document this is.
pub(super) fn check(kind: usize) -> String {
    wrap(&format!(
        "const k = DATA.kinds[{kind}];
         let fields = 0, filled = false, success = false;
         for (const root of ALL) {{
           if (k.fields.length) {{
             const names = k.fields.map((n) => '[name=\"' + n + '\"]').join(',');
             for (const f of root.querySelectorAll(names)) {{
               fields += 1;
               if (f.value) filled = true;
             }}
           }}
           for (const s of k.success) {{
             for (const e of root.querySelectorAll(s)) {{ if (shown(e)) success = true; }}
           }}
         }}
         return {{
           fields, filled, success,
           widget: find(false, {kind}) !== null,
           pending: pending(),
           epoch: window.__sbCaptcha || null,
         }};"
    ))
}

/// Marks this document, so a later reading can tell it has been replaced, and
/// returns the mark.
pub(super) fn epoch() -> String {
    "(() => { if (!window.__sbCaptcha) { window.__sbCaptcha = Math.random().toString(36).slice(2); } \
     return window.__sbCaptcha; })()"
        .to_owned()
}

/// The viewport size, as `[width, height]`.
pub(super) const VIEWPORT: &str = "[window.innerWidth, window.innerHeight]";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_carries_every_selector_as_data() {
        let script = locate();
        for kind in Captcha::ALL {
            for tier in kind.tiers() {
                for selector in *tier {
                    let quoted = serde_json::to_string(selector).unwrap();
                    assert!(script.contains(&quoted), "{kind}: {selector} is missing");
                }
            }
        }
        for marker in MARKERS {
            assert!(script.contains(&serde_json::to_string(marker).unwrap()));
        }
    }

    #[test]
    fn a_script_is_one_expression_with_no_leftover_placeholder() {
        for script in [locate(), probe(true), probe(false), check(0), epoch()] {
            assert!(script.starts_with("(() =>"), "{script}");
            assert!(
                script.ends_with("})()") || script.ends_with(")()"),
                "{script}"
            );
            assert!(!script.contains("__DATA__"), "{script}");
        }
    }

    #[test]
    fn the_check_reads_the_response_fields_of_its_widget_only() {
        let script = check(0);
        assert!(script.contains("DATA.kinds[0]"));
        assert!(!script.contains("DATA.kinds[1]"));
        let data = data();
        assert!(data.contains("cf-turnstile-response"));
        assert!(data.contains("g-recaptcha-response"));
    }

    #[test]
    fn the_challenge_beacon_alone_does_not_mark_a_page() {
        // Most Cloudflare sites carry the jsd beacon on every page; waiting for
        // a widget there would slow every call down.
        for marker in MARKERS {
            assert!(!marker.contains("/jsd/"), "{marker}");
        }
        assert!(MARKERS.iter().any(|m| m.contains("/orchestrate/")));
    }
}
