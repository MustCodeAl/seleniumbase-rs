//! Parses and renders Akamai HTTP/2 fingerprint strings: SETTINGS, `WINDOW_UPDATE`, PRIORITY and pseudo-header order.
//!
//! The format is the one in Akamai's public whitepaper on passive HTTP/2 fingerprinting:
//! four sections separated by `|`.
//!
//! ```text
//! SETTINGS | WINDOW_UPDATE | PRIORITY | pseudo-header order
//! 1:4096;2:0;4:1048576|983041|3:0:0:201,5:1:3:16|m,a,s,p
//! ```
//!
//! - SETTINGS is `id:value` pairs joined by `;`, in the order the frame carried them.
//! - `WINDOW_UPDATE` is the connection-level increment, or `0` when no frame was sent.
//! - PRIORITY is `stream:exclusive:depends_on:weight` entries joined by `,`, or `0` for none.
//!   The weight is the protocol weight, 1 to 256 (the wire byte plus one).
//! - The pseudo-header order is a permutation of `m,a,s,p` (`:method`, `:authority`,
//!   `:scheme`, `:path`).
//!
//! Parsing is strict about what can exist on the wire: canonical decimal numbers only, each
//! SETTINGS id once, weights in 1 to 256, stream ids below 2^31. What a given client stack
//! can emit is a separate question, answered by `H2Spec` in the `h2_spec` module.
//!
//! ```
//! use seleniumbase_rs::wire::akamai::Akamai;
//!
//! let text = "1:4096;2:0;4:1048576|983041|0|m,a,s,p";
//! let parsed = Akamai::parse(text).expect("a well-formed string");
//! assert_eq!(parsed.settings, vec![(1, 4096), (2, 0), (4, 1_048_576)]);
//! assert_eq!(parsed.to_string(), text);
//! ```

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The largest stream id, dependency id or window increment: 2^31 - 1 (the top bit is reserved).
pub const MAX_STREAM_ID: u32 = 0x7fff_ffff;
/// The smallest protocol weight of a stream (the wire byte is one less).
pub const MIN_WEIGHT: u16 = 1;
/// The largest protocol weight of a stream (the wire byte is one less).
pub const MAX_WEIGHT: u16 = 256;
/// The longest Akamai string accepted, in bytes.
pub const MAX_AKAMAI_LEN: usize = 4096;
/// The most SETTINGS entries accepted in one string.
pub const MAX_SETTINGS: usize = 32;
/// The most PRIORITY entries accepted in one string.
pub const MAX_PRIORITY_FRAMES: usize = 64;

/// A request pseudo-header, as named in the pseudo-header order section.
///
/// This is the complete set of pseudo-headers of a plain request, so the enum is not
/// `non_exhaustive`: a pseudo-header order is a permutation of exactly these four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pseudo {
    /// `:method`, written `m`.
    Method,
    /// `:authority`, written `a`.
    Authority,
    /// `:scheme`, written `s`.
    Scheme,
    /// `:path`, written `p`.
    Path,
}

impl Pseudo {
    /// The four pseudo-headers in the order `m,a,s,p`.
    pub const ALL: [Pseudo; 4] = [
        Pseudo::Method,
        Pseudo::Authority,
        Pseudo::Scheme,
        Pseudo::Path,
    ];

    /// The letter used in the Akamai string.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Pseudo::Method => 'm',
            Pseudo::Authority => 'a',
            Pseudo::Scheme => 's',
            Pseudo::Path => 'p',
        }
    }

    /// The header name as it appears in an HTTP/2 header block, such as `:method`.
    #[must_use]
    pub const fn header_name(self) -> &'static str {
        match self {
            Pseudo::Method => ":method",
            Pseudo::Authority => ":authority",
            Pseudo::Scheme => ":scheme",
            Pseudo::Path => ":path",
        }
    }

    /// The pseudo-header written as `letter` in an Akamai string (lowercase only).
    #[must_use]
    pub fn from_letter(letter: char) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.letter() == letter)
    }

    /// The pseudo-header with this exact header name, such as `:path`.
    #[must_use]
    pub fn from_header_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.header_name() == name)
    }
}

/// One PRIORITY frame, in the Akamai notation `stream:exclusive:depends_on:weight`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorityFrame {
    /// The stream the frame prioritises.
    pub stream_id: u32,
    /// Whether the dependency is exclusive.
    pub exclusive: bool,
    /// The stream this one depends on.
    pub depends_on: u32,
    /// The protocol weight, 1 to 256 (the wire byte plus one).
    pub weight: u16,
}

impl fmt::Display for PriorityFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}:{}",
            self.stream_id,
            u8::from(self.exclusive),
            self.depends_on,
            self.weight
        )
    }
}

/// A parsed Akamai HTTP/2 fingerprint.
///
/// The fields are public so a caller can build one directly; [`Akamai::parse`] is the
/// validating constructor, and `H2Spec::with_akamai` re-checks whatever it is given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Akamai {
    /// SETTINGS entries `(id, value)` in the order the frame carried them.
    pub settings: Vec<(u16, u32)>,
    /// The connection-level `WINDOW_UPDATE` increment; `0` means no frame.
    pub window_update: u32,
    /// PRIORITY frames in the order they were sent.
    pub priority_frames: Vec<PriorityFrame>,
    /// The order of the four request pseudo-headers.
    pub pseudo_order: [Pseudo; 4],
}

/// Why an Akamai string was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AkamaiError {
    /// The string is longer than [`MAX_AKAMAI_LEN`] bytes.
    #[error("Akamai string is longer than {} bytes", MAX_AKAMAI_LEN)]
    TooLong,
    /// The string does not have exactly four `|`-separated sections.
    #[error("expected 4 `|`-separated sections, found {0}")]
    Sections(usize),
    /// A SETTINGS entry is not a canonical `id:value` pair.
    #[error(
        "SETTINGS entry {index} `{token}` is not `id:value` with a 16-bit id and a 32-bit value"
    )]
    Setting {
        /// Zero-based position of the entry.
        index: usize,
        /// The offending text.
        token: String,
    },
    /// A SETTINGS id appears more than once.
    #[error("SETTINGS id {0} appears more than once")]
    DuplicateSetting(u16),
    /// The SETTINGS section has more than [`MAX_SETTINGS`] entries.
    #[error("more than {} SETTINGS entries", MAX_SETTINGS)]
    TooManySettings,
    /// The `WINDOW_UPDATE` section is not a number from 0 to 2^31 - 1.
    #[error("WINDOW_UPDATE `{0}` is not a number from 0 to 2147483647")]
    WindowUpdate(String),
    /// A PRIORITY entry is not `stream:exclusive:depends_on:weight` with a valid range.
    #[error("PRIORITY entry {index} `{token}` is not `stream:exclusive:depends_on:weight`")]
    PriorityFrame {
        /// Zero-based position of the entry.
        index: usize,
        /// The offending text.
        token: String,
    },
    /// The PRIORITY section has more than [`MAX_PRIORITY_FRAMES`] entries.
    #[error("more than {} PRIORITY entries", MAX_PRIORITY_FRAMES)]
    TooManyPriorityFrames,
    /// The pseudo-header order is not a permutation of `m,a,s,p`.
    #[error("pseudo-header order `{0}` is not a permutation of m,a,s,p")]
    PseudoOrder(String),
}

impl Akamai {
    /// Parses an Akamai fingerprint string.
    ///
    /// Leading and trailing whitespace is ignored; nothing inside the string is.
    ///
    /// # Errors
    ///
    /// Returns an [`AkamaiError`] naming the section and entry that is malformed.
    pub fn parse(text: &str) -> Result<Self, AkamaiError> {
        let text = text.trim();
        if text.len() > MAX_AKAMAI_LEN {
            return Err(AkamaiError::TooLong);
        }
        let sections: Vec<&str> = text.split('|').collect();
        let [settings, window, priority, pseudo] = sections.as_slice() else {
            return Err(AkamaiError::Sections(sections.len()));
        };
        Ok(Self {
            settings: parse_settings(settings)?,
            window_update: parse_window_update(window)?,
            priority_frames: parse_priority(priority)?,
            pseudo_order: parse_pseudo_order(pseudo)?,
        })
    }
}

/// Canonical decimal: ASCII digits only, no sign, no leading zeros, and it fits `T`.
fn decimal<T: FromStr>(token: &str) -> Option<T> {
    let digits_only = !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit());
    let canonical = token == "0" || !token.starts_with('0');
    if digits_only && canonical {
        token.parse().ok()
    } else {
        None
    }
}

fn parse_settings(section: &str) -> Result<Vec<(u16, u32)>, AkamaiError> {
    let mut settings: Vec<(u16, u32)> = Vec::new();
    if section.is_empty() {
        return Ok(settings);
    }
    for (index, token) in section.split(';').enumerate() {
        if index == MAX_SETTINGS {
            return Err(AkamaiError::TooManySettings);
        }
        let malformed = || AkamaiError::Setting {
            index,
            token: token.to_owned(),
        };
        let (id, value) = token.split_once(':').ok_or_else(malformed)?;
        let id: u16 = decimal(id).ok_or_else(malformed)?;
        let value: u32 = decimal(value).ok_or_else(malformed)?;
        if settings.iter().any(|(seen, _)| *seen == id) {
            return Err(AkamaiError::DuplicateSetting(id));
        }
        settings.push((id, value));
    }
    Ok(settings)
}

fn parse_window_update(section: &str) -> Result<u32, AkamaiError> {
    decimal::<u32>(section)
        .filter(|value| *value <= MAX_STREAM_ID)
        .ok_or_else(|| AkamaiError::WindowUpdate(section.to_owned()))
}

fn parse_priority(section: &str) -> Result<Vec<PriorityFrame>, AkamaiError> {
    if section == "0" {
        return Ok(Vec::new());
    }
    let mut frames = Vec::new();
    for (index, token) in section.split(',').enumerate() {
        if index == MAX_PRIORITY_FRAMES {
            return Err(AkamaiError::TooManyPriorityFrames);
        }
        let frame = parse_priority_frame(token).ok_or_else(|| AkamaiError::PriorityFrame {
            index,
            token: token.to_owned(),
        })?;
        frames.push(frame);
    }
    Ok(frames)
}

fn parse_priority_frame(token: &str) -> Option<PriorityFrame> {
    let mut parts = token.split(':');
    let stream_id: u32 = decimal(parts.next()?)?;
    let exclusive = match parts.next()? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    let depends_on: u32 = decimal(parts.next()?)?;
    let weight: u16 = decimal(parts.next()?)?;
    let in_range = stream_id <= MAX_STREAM_ID
        && depends_on <= MAX_STREAM_ID
        && (MIN_WEIGHT..=MAX_WEIGHT).contains(&weight);
    if parts.next().is_some() || !in_range {
        return None;
    }
    Some(PriorityFrame {
        stream_id,
        exclusive,
        depends_on,
        weight,
    })
}

fn parse_pseudo_order(section: &str) -> Result<[Pseudo; 4], AkamaiError> {
    let malformed = || AkamaiError::PseudoOrder(section.to_owned());
    let mut order: Vec<Pseudo> = Vec::with_capacity(Pseudo::ALL.len());
    for token in section.split(',') {
        let mut chars = token.chars();
        let (Some(letter), None) = (chars.next(), chars.next()) else {
            return Err(malformed());
        };
        let pseudo = Pseudo::from_letter(letter).ok_or_else(malformed)?;
        if order.contains(&pseudo) {
            return Err(malformed());
        }
        order.push(pseudo);
    }
    <[Pseudo; 4]>::try_from(order).map_err(|_incomplete| malformed())
}

impl FromStr for Akamai {
    type Err = AkamaiError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl fmt::Display for Akamai {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let settings: Vec<String> = self
            .settings
            .iter()
            .map(|(id, value)| format!("{id}:{value}"))
            .collect();
        let priority = if self.priority_frames.is_empty() {
            "0".to_owned()
        } else {
            let frames: Vec<String> = self
                .priority_frames
                .iter()
                .map(ToString::to_string)
                .collect();
            frames.join(",")
        };
        let pseudo: Vec<String> = self
            .pseudo_order
            .iter()
            .map(|p| p.letter().to_string())
            .collect();
        write!(
            f,
            "{}|{}|{}|{}",
            settings.join(";"),
            self.window_update,
            priority,
            pseudo.join(",")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic values in the whitepaper's shape; they are not measured from any browser.
    const SAMPLE: &str = "1:4096;2:0;3:100;4:1048576|983041|3:0:0:201,5:1:3:16|m,a,s,p";

    /// The shape of the example in Akamai's whitepaper, to prove the parser reads a
    /// string with many PRIORITY frames and a different pseudo-header order.
    const WHITEPAPER_SHAPE: &str = "1:65536;4:131072;5:16384|12517377|3:0:0:201,5:0:0:101,7:0:0:1,9:0:7:1,11:0:3:1,13:0:0:241|m,p,a,s";

    fn sample() -> Akamai {
        Akamai::parse(SAMPLE).expect("sample parses")
    }

    #[test]
    fn parses_every_section_of_the_sample() {
        let a = sample();
        assert_eq!(
            a.settings,
            vec![(1, 4096), (2, 0), (3, 100), (4, 1_048_576)]
        );
        assert_eq!(a.window_update, 983_041);
        assert_eq!(
            a.priority_frames,
            vec![
                PriorityFrame {
                    stream_id: 3,
                    exclusive: false,
                    depends_on: 0,
                    weight: 201
                },
                PriorityFrame {
                    stream_id: 5,
                    exclusive: true,
                    depends_on: 3,
                    weight: 16
                },
            ]
        );
        assert_eq!(a.pseudo_order, Pseudo::ALL);
    }

    #[test]
    fn parses_the_whitepaper_shape() {
        let a = Akamai::parse(WHITEPAPER_SHAPE).expect("parses");
        assert_eq!(a.settings.len(), 3);
        assert_eq!(a.priority_frames.len(), 6);
        assert_eq!(a.priority_frames[3].depends_on, 7);
        assert_eq!(
            a.pseudo_order,
            [
                Pseudo::Method,
                Pseudo::Path,
                Pseudo::Authority,
                Pseudo::Scheme
            ]
        );
    }

    #[test]
    fn display_round_trips_canonical_strings() {
        for text in [
            SAMPLE,
            WHITEPAPER_SHAPE,
            "|0|0|m,a,s,p",
            "4:2097152;3:100|10485760|0|m,s,p,a",
            "1:0|2147483647|2147483647:1:2147483647:256|p,s,a,m",
        ] {
            let parsed = Akamai::parse(text).expect(text);
            assert_eq!(parsed.to_string(), text);
            assert_eq!(text.parse::<Akamai>().expect("FromStr"), parsed);
        }
    }

    #[test]
    fn surrounding_whitespace_is_ignored_but_inner_whitespace_is_not() {
        assert_eq!(
            Akamai::parse(&format!("  {SAMPLE}\n")).expect("trimmed"),
            sample()
        );
        assert!(matches!(
            Akamai::parse("1:4096; 2:0|0|0|m,a,s,p"),
            Err(AkamaiError::Setting { index: 1, .. })
        ));
        assert!(matches!(
            Akamai::parse("1:4096|0|0|m, a,s,p"),
            Err(AkamaiError::PseudoOrder(_))
        ));
    }

    #[test]
    fn wrong_section_counts_are_named() {
        for (text, count) in [("", 1), ("a", 1), ("1:1|0|0", 3), ("1:1|0|0|m,a,s,p|x", 5)] {
            assert_eq!(
                Akamai::parse(text),
                Err(AkamaiError::Sections(count)),
                "{text}"
            );
        }
    }

    #[test]
    fn malformed_settings_name_the_entry() {
        let cases = [
            ("1", 0),
            ("1:", 0),
            (":1", 0),
            ("1:2:3", 0),
            ("a:1", 0),
            ("1:b", 0),
            ("65536:1", 0),
            ("1:4294967296", 0),
            ("+1:1", 0),
            ("1:-1", 0),
            ("01:1", 0),
            ("1:01", 0),
            ("1:1;", 1),
            ("1:1;;2:2", 1),
            ("1:1;2", 1),
            (";1:1", 0),
        ];
        for (section, at) in cases {
            let text = format!("{section}|0|0|m,a,s,p");
            match Akamai::parse(&text) {
                Err(AkamaiError::Setting { index, .. }) => assert_eq!(index, at, "{section}"),
                other => panic!("{section}: expected a Setting error, got {other:?}"),
            }
        }
    }

    #[test]
    fn duplicate_settings_are_refused_not_dropped() {
        assert_eq!(
            Akamai::parse("1:1;2:0;1:2|0|0|m,a,s,p"),
            Err(AkamaiError::DuplicateSetting(1))
        );
    }

    #[test]
    fn setting_ids_and_values_accept_their_full_range() {
        let a = Akamai::parse("0:0;65535:4294967295|0|0|m,a,s,p").expect("full range");
        assert_eq!(a.settings, vec![(0, 0), (65535, u32::MAX)]);
    }

    #[test]
    fn too_many_settings_are_refused() {
        let many: Vec<String> = (1..=33u16).map(|id| format!("{id}:1")).collect();
        let text = format!("{}|0|0|m,a,s,p", many.join(";"));
        assert_eq!(Akamai::parse(&text), Err(AkamaiError::TooManySettings));
        let ok: Vec<String> = (1..=32u16).map(|id| format!("{id}:1")).collect();
        assert!(Akamai::parse(&format!("{}|0|0|m,a,s,p", ok.join(";"))).is_ok());
    }

    #[test]
    fn window_update_bounds() {
        for ok in ["0", "1", "15663105", "2147483647"] {
            assert!(
                Akamai::parse(&format!("1:1|{ok}|0|m,a,s,p")).is_ok(),
                "{ok}"
            );
        }
        for bad in [
            "",
            "-1",
            "+1",
            "01",
            "x",
            "2147483648",
            "4294967296",
            "1 ",
            "1.5",
        ] {
            let text = format!("1:1|{bad}|0|m,a,s,p");
            match Akamai::parse(&text) {
                Err(AkamaiError::WindowUpdate(token)) => assert_eq!(token, bad),
                other => panic!("{bad:?}: expected WindowUpdate, got {other:?}"),
            }
        }
    }

    #[test]
    fn priority_section_zero_means_none_and_empty_is_malformed() {
        assert!(sample_with_priority("0").priority_frames.is_empty());
        assert!(matches!(
            Akamai::parse("1:1|0||m,a,s,p"),
            Err(AkamaiError::PriorityFrame { index: 0, .. })
        ));
    }

    fn sample_with_priority(section: &str) -> Akamai {
        Akamai::parse(&format!("1:1|0|{section}|m,a,s,p")).expect("priority section parses")
    }

    #[test]
    fn priority_frame_bounds() {
        for ok in [
            "1:0:0:1",
            "1:1:0:256",
            "2147483647:0:2147483647:128",
            "1:0:0:201",
        ] {
            assert_eq!(sample_with_priority(ok).priority_frames.len(), 1, "{ok}");
        }
        for bad in [
            "1:0:0:0",
            "1:0:0:257",
            "1:2:0:16",
            "1:0:0",
            "1:0:0:1:1",
            "2147483648:0:0:1",
            "1:0:2147483648:1",
            "a:0:0:1",
            "1:0:0:x",
            "1:0:0:-1",
            "1:0:0:+1",
            "1:00:0:1",
            "1:0:0:016",
            "",
        ] {
            let text = format!("1:1|0|{bad}|m,a,s,p");
            assert!(
                matches!(
                    Akamai::parse(&text),
                    Err(AkamaiError::PriorityFrame { index: 0, .. })
                ),
                "{bad:?} -> {:?}",
                Akamai::parse(&text)
            );
        }
    }

    #[test]
    fn priority_errors_report_the_position_of_the_bad_entry() {
        assert_eq!(
            Akamai::parse("1:1|0|3:0:0:201,5:0:0:0,7:0:0:1|m,a,s,p"),
            Err(AkamaiError::PriorityFrame {
                index: 1,
                token: "5:0:0:0".to_owned()
            })
        );
        assert!(matches!(
            Akamai::parse("1:1|0|3:0:0:201,|m,a,s,p"),
            Err(AkamaiError::PriorityFrame { index: 1, .. })
        ));
    }

    #[test]
    fn too_many_priority_frames_are_refused() {
        let frame = |i: usize| format!("{}:0:0:16", 2 * i + 1);
        let at_cap: Vec<String> = (0..64).map(frame).collect();
        assert!(Akamai::parse(&format!("1:1|0|{}|m,a,s,p", at_cap.join(","))).is_ok());
        let over: Vec<String> = (0..65).map(frame).collect();
        assert_eq!(
            Akamai::parse(&format!("1:1|0|{}|m,a,s,p", over.join(","))),
            Err(AkamaiError::TooManyPriorityFrames)
        );
    }

    #[test]
    fn pseudo_order_must_be_a_full_permutation() {
        for bad in [
            "",
            "m",
            "m,a,s",
            "m,a,s,p,m",
            "m,m,s,p",
            "masp",
            "M,A,S,P",
            "m,a,s,x",
            "m,a,s,pp",
            "m;a;s;p",
            "m,a,s,",
            ",m,a,s",
            "m, a,s,p",
            ":m,a,s,p",
        ] {
            let text = format!("1:1|0|0|{bad}");
            assert_eq!(
                Akamai::parse(&text),
                Err(AkamaiError::PseudoOrder(bad.to_owned())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn all_twenty_four_pseudo_orders_round_trip() {
        let mut seen = 0;
        let letters = ['m', 'a', 's', 'p'];
        for a in 0..4 {
            for b in (0..4).filter(|b| *b != a) {
                for c in (0..4).filter(|c| *c != a && *c != b) {
                    let d = 6 - a - b - c;
                    let order = format!(
                        "{},{},{},{}",
                        letters[a], letters[b], letters[c], letters[d]
                    );
                    let text = format!("1:1|0|0|{order}");
                    assert_eq!(Akamai::parse(&text).expect(&order).to_string(), text);
                    seen += 1;
                }
            }
        }
        assert_eq!(seen, 24);
    }

    #[test]
    fn only_a_genuinely_long_string_is_refused_before_parsing() {
        // Surrounding whitespace is trimmed first, so it does not count toward the cap.
        let long = format!("1:1|0|0|m,a,s,p{}", " ".repeat(MAX_AKAMAI_LEN));
        assert!(Akamai::parse(&long).is_ok());
        let padded = format!("1:1|0|0|m,a,s,p{}", "x".repeat(MAX_AKAMAI_LEN));
        assert_eq!(Akamai::parse(&padded), Err(AkamaiError::TooLong));
    }

    #[test]
    fn pseudo_helpers_agree_with_each_other() {
        for pseudo in Pseudo::ALL {
            assert_eq!(Pseudo::from_letter(pseudo.letter()), Some(pseudo));
            assert_eq!(Pseudo::from_header_name(pseudo.header_name()), Some(pseudo));
            assert!(pseudo.header_name().starts_with(':'));
        }
        assert_eq!(Pseudo::from_letter('x'), None);
        assert_eq!(Pseudo::from_letter('M'), None);
        assert_eq!(Pseudo::from_header_name(":status"), None);
        assert_eq!(Pseudo::from_header_name("method"), None);
        assert_eq!(Pseudo::from_header_name(":Method"), None);
    }

    #[test]
    fn json_round_trip_keeps_every_field() {
        let a = Akamai::parse(WHITEPAPER_SHAPE).expect("parses");
        let json = serde_json::to_string(&a).expect("serialises");
        let back: Akamai = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, a);
        assert!(
            json.contains("\"path\""),
            "pseudo names are readable: {json}"
        );
    }

    #[test]
    fn truncated_input_never_panics_and_accepted_prefixes_round_trip() {
        for text in [SAMPLE, WHITEPAPER_SHAPE] {
            for end in 0..=text.len() {
                if let Ok(parsed) = Akamai::parse(&text[..end]) {
                    assert_eq!(Akamai::parse(&parsed.to_string()), Ok(parsed));
                }
            }
        }
    }

    /// A tiny deterministic generator for the mutation loop; no clock, no dependency.
    struct XorShift(u64);

    impl XorShift {
        fn step(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn mutated_strings_never_panic() {
        const ALPHABET: &[u8] = b"0123456789:;,|msap-+ x\xff";
        let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
        for _ in 0..4000 {
            let mut bytes = WHITEPAPER_SHAPE.as_bytes().to_vec();
            for _ in 0..=(rng.step() % 4) {
                let at = usize::try_from(rng.step()).unwrap_or(0) % bytes.len();
                let pick = usize::try_from(rng.step()).unwrap_or(0) % ALPHABET.len();
                match rng.step() % 3 {
                    0 => bytes[at] = ALPHABET[pick],
                    1 => bytes.insert(at, ALPHABET[pick]),
                    _ => {
                        bytes.remove(at);
                    }
                }
            }
            let text = String::from_utf8_lossy(&bytes);
            if let Ok(parsed) = Akamai::parse(&text) {
                assert_eq!(Akamai::parse(&parsed.to_string()), Ok(parsed));
            }
        }
    }
}
