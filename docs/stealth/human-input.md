---
title: "Human-like input"
sidebarTitle: "Human-like input"
---

Detection scripts watch how input arrives as well as what the browser claims:
events that are not trusted, a pointer that jumps straight to its target, keys
typed at a constant rate. The pure CDP engine sends trusted input through the
DevTools Protocol, and UC mode has `uc_click` and `uc_type` with human timing
(see [UC mode](/tutorials/uc_mode)). This page is the model behind the pacing.


Detectors look for input that is too regular. The
`evasion::behavior` module plans input that is not: curved
pointer paths with a bell-shaped speed profile, Fitts's-law timing, tremor and
overshoot, and Gaussian typing with word and sentence pauses. It is pure and
seedable:

```rust
use seleniumbase_rs_stealth::sb_cdp::Point;
use seleniumbase_rs_stealth::evasion::behavior::{Behavior, Humanizer};

fn main() -> Result<(), seleniumbase_rs_stealth::SeleniumBaseError> {
    let mut person = Humanizer::new(Behavior::builder().typing_wpm(70.0).seed(7).build()?);
    let path = person.mouse_path(Point::new(0.0, 0.0), Point::new(200.0, 90.0), 60.0);
    let keys = person.typing_plan("hello");
    assert_eq!(path.last().map(|step| step.at), Some(Point::new(200.0, 90.0)));
    assert_eq!(keys.len(), 5);
    Ok(())
}
```

To play a plan out on a page, use `page.human(behavior)` from the Pure CDP
engine (see [Pure CDP](/tutorials/pure_cdp#human-paced-input)). `BaseCase::human_type`
uses the same typing model, taking its pace from `Fingerprint::humanize()` when the
configuration has a fingerprint.

`Behavior::from_config(fingerprint.humanize())` turns a fingerprint's
`HumanizeConfig` (its minimum and maximum keystroke delay) into the matching
behaviour.

<Tip>

The older `evasion::humanize` helpers (`bezier_mouse_path`,
`keystroke_delays`) remain, but they sample a Bézier curve at evenly spaced
parameter values and draw delays uniformly between two bounds; prefer
`behavior`.

</Tip>

