//! Tests for `HeaderSpec`: capture, rendering, overrides, validation and serde.

use super::*;
use crate::wire::h2_frames::{encode_frame, frame_type, read_preface, CLIENT_PREFACE};

/// Synthetic captures in the shape of a browser's: the names are real, the values are made up.
const DOCUMENT: [(&str, &str); 14] = [
    ("Host", "example.test"),
    ("Connection", "keep-alive"),
    ("Upgrade-Insecure-Requests", "1"),
    ("User-Agent", "TestAgent/1.0"),
    ("Accept", "text/html,*/*;q=0.8"),
    ("Sec-Fetch-Site", "none"),
    ("Sec-Fetch-Mode", "navigate"),
    ("Sec-Fetch-Dest", "document"),
    ("sec-ch-ua", "\"Test\";v=\"1\""),
    ("sec-ch-ua-mobile", "?0"),
    ("sec-ch-ua-platform", "\"TestOS\""),
    ("Accept-Encoding", "gzip, deflate"),
    ("Accept-Language", "en-US"),
    ("Cookie", "a=b; c=d"),
];

const FETCH: [(&str, &str); 8] = [
    ("Host", "example.test"),
    ("Connection", "keep-alive"),
    ("Content-Length", "0"),
    ("User-Agent", "TestAgent/1.0"),
    ("Accept", "*/*"),
    ("Sec-Fetch-Mode", "cors"),
    ("Sec-Fetch-Dest", "empty"),
    ("Accept-Language", "en-US"),
];

fn spec() -> HeaderSpec {
    HeaderSpec::from_captured(&DOCUMENT, &FETCH).expect("a valid capture")
}

fn names(list: &[(String, HeaderValue)]) -> Vec<&str> {
    list.iter().map(|(name, _)| name.as_str()).collect()
}

fn owned(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect()
}

fn lit(text: &str) -> HeaderValue {
    HeaderValue::Literal(text.to_owned())
}

#[test]
fn a_capture_becomes_placeholders_and_literals_in_order() {
    let s = spec();
    assert_eq!(
        names(&s.document),
        [
            "Upgrade-Insecure-Requests",
            "User-Agent",
            "Accept",
            "Sec-Fetch-Site",
            "Sec-Fetch-Mode",
            "Sec-Fetch-Dest",
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "sec-ch-ua-platform",
            "Accept-Encoding",
            "Accept-Language",
            "Cookie",
        ],
        "transport headers are dropped, capitalisation and order are kept"
    );
    let value = |name: &str| {
        s.document
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(value("User-Agent"), Some(HeaderValue::UserAgent));
    assert_eq!(value("sec-ch-ua"), Some(HeaderValue::SecChUa));
    assert_eq!(value("sec-ch-ua-mobile"), Some(HeaderValue::SecChUaMobile));
    assert_eq!(
        value("sec-ch-ua-platform"),
        Some(HeaderValue::SecChUaPlatform)
    );
    assert_eq!(value("Accept-Language"), Some(HeaderValue::AcceptLanguage));
    assert_eq!(value("Cookie"), Some(HeaderValue::Cookie));
    assert_eq!(value("Accept"), Some(lit("text/html,*/*;q=0.8")));
    assert_eq!(value("Sec-Fetch-Dest"), Some(lit("document")));
    assert_eq!(
        names(&s.fetch),
        [
            "User-Agent",
            "Accept",
            "Sec-Fetch-Mode",
            "Sec-Fetch-Dest",
            "Accept-Language"
        ]
    );
    assert_eq!(s.priority_by_dest, Vec::<(String, String)>::new());
    assert_eq!(s.validate(), Ok(()));
}

#[test]
fn rendering_the_capturing_identity_reproduces_the_capture() {
    let s = spec();
    let identity = HeaderIdentity::from_captured(&DOCUMENT).expect("has a user agent");
    let want: Vec<(String, String)> = owned(&DOCUMENT)
        .into_iter()
        .filter(|(n, _)| n != "Host" && n != "Connection")
        .collect();
    assert_eq!(s.render(RequestShape::Document, &identity), Ok(want));
    let fetch_identity = HeaderIdentity::from_captured(&FETCH).expect("has a user agent");
    let rendered = s
        .render(RequestShape::Fetch, &fetch_identity)
        .expect("renders");
    assert_eq!(rendered.len(), 5);
    assert_eq!(
        rendered[0],
        ("User-Agent".to_owned(), "TestAgent/1.0".to_owned())
    );
}

#[test]
fn the_identity_read_from_a_capture_has_each_value() {
    let id = HeaderIdentity::from_captured(&DOCUMENT).expect("has a user agent");
    assert_eq!(id.user_agent, "TestAgent/1.0");
    assert_eq!(id.accept_language.as_deref(), Some("en-US"));
    assert_eq!(id.sec_ch_ua.as_deref(), Some("\"Test\";v=\"1\""));
    assert_eq!(id.sec_ch_ua_mobile.as_deref(), Some("?0"));
    assert_eq!(id.sec_ch_ua_platform.as_deref(), Some("\"TestOS\""));
    assert_eq!(id.cookie.as_deref(), Some("a=b; c=d"));
    assert_eq!(id.overrides, Vec::<(String, String)>::new());
    assert_eq!(HeaderIdentity::from_captured(&[("Accept", "*/*")]), None);
}

#[test]
fn a_placeholder_without_a_value_leaves_its_header_out() {
    let s = spec();
    let rendered = s
        .render(RequestShape::Document, &HeaderIdentity::new("Other/9"))
        .expect("renders");
    let kept: Vec<&str> = rendered.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        kept,
        [
            "Upgrade-Insecure-Requests",
            "User-Agent",
            "Accept",
            "Sec-Fetch-Site",
            "Sec-Fetch-Mode",
            "Sec-Fetch-Dest",
            "Accept-Encoding",
        ]
    );
    assert_eq!(rendered[1].1, "Other/9");
}

#[test]
fn every_identity_value_can_be_supplied_by_the_caller() {
    let s = spec();
    let identity = HeaderIdentity::new("Mine/3")
        .with_accept_language("de-DE,de;q=0.9")
        .with_client_hints("\"Mine\";v=\"3\"", "?1", "\"MineOS\"")
        .with_cookie("k=v");
    let rendered = s
        .render(RequestShape::Document, &identity)
        .expect("renders");
    let get = |name: &str| {
        rendered
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(get("User-Agent"), Some("Mine/3"));
    assert_eq!(get("Accept-Language"), Some("de-DE,de;q=0.9"));
    assert_eq!(get("sec-ch-ua"), Some("\"Mine\";v=\"3\""));
    assert_eq!(get("sec-ch-ua-mobile"), Some("?1"));
    assert_eq!(get("sec-ch-ua-platform"), Some("\"MineOS\""));
    assert_eq!(get("Cookie"), Some("k=v"));
    assert_eq!(get("Accept"), Some("text/html,*/*;q=0.8"), "literals stay");
}

#[test]
fn overrides_replace_a_value_by_name_without_regard_to_case() {
    let s = spec();
    let identity = HeaderIdentity::new("A/1")
        .with_override("accept", "application/json")
        .with_override("X-Not-In-The-Spec", "ignored")
        .with_override("user-agent", "Override/2");
    let rendered = s.render(RequestShape::Fetch, &identity).expect("renders");
    assert_eq!(
        rendered[0],
        ("User-Agent".to_owned(), "Override/2".to_owned())
    );
    assert_eq!(
        rendered[1],
        ("Accept".to_owned(), "application/json".to_owned())
    );
    assert!(rendered.iter().all(|(n, _)| n != "X-Not-In-The-Spec"));
}

#[test]
fn a_resolved_value_that_is_not_a_header_value_is_refused_without_echoing_it() {
    let s = spec();
    for bad in [
        "a\r\nX-Injected: 1",
        "a\nb",
        "nul\0",
        " leading",
        "trailing ",
        "del\x7f",
    ] {
        let err = s
            .render(RequestShape::Fetch, &HeaderIdentity::new(bad))
            .expect_err("refused");
        assert_eq!(
            err,
            HeaderSpecError::InvalidValue {
                name: "User-Agent".to_owned()
            }
        );
        assert!(!format!("{err}{err:?}").contains("Injected"));
    }
    let huge = "x".repeat(MAX_VALUE_LEN + 1);
    assert!(s
        .render(RequestShape::Fetch, &HeaderIdentity::new(huge))
        .is_err());
    let ok = "x".repeat(MAX_VALUE_LEN);
    assert!(s
        .render(RequestShape::Fetch, &HeaderIdentity::new(ok))
        .is_ok());
}

#[test]
fn the_debug_of_an_identity_hides_the_cookie_and_the_override_values() {
    let identity = HeaderIdentity::new("UA/1")
        .with_cookie("session=SECRET-COOKIE")
        .with_override("Authorization", "Bearer SECRET-TOKEN");
    let shown = format!("{identity:?}");
    assert!(!shown.contains("SECRET"), "{shown}");
    assert!(
        shown.contains("<redacted>") && shown.contains("Authorization") && shown.contains("UA/1")
    );
}

#[test]
fn an_http2_capture_moves_priority_into_the_table_and_skips_pseudo_headers() {
    let document = [
        (":method", "GET"),
        (":path", "/"),
        ("user-agent", "TestAgent/1.0"),
        ("sec-fetch-dest", "document"),
        ("priority", "u=0, i"),
    ];
    let fetch = [
        (":method", "GET"),
        ("user-agent", "TestAgent/1.0"),
        ("priority", "u=1, i"),
        ("sec-fetch-dest", "empty"),
    ];
    let s = HeaderSpec::from_captured(&document, &fetch).expect("valid");
    assert_eq!(names(&s.document), ["user-agent", "sec-fetch-dest"]);
    assert_eq!(names(&s.fetch), ["user-agent", "sec-fetch-dest"]);
    assert_eq!(s.priority_for("document"), Some("u=0, i"));
    assert_eq!(s.priority_for("EMPTY"), Some("u=1, i"));
    assert_eq!(s.priority_for("image"), None);
    assert_eq!(s.validate(), Ok(()));
}

#[test]
fn a_priority_header_needs_a_destination_and_must_agree_with_the_other_list() {
    let no_dest = [("user-agent", "A"), ("priority", "u=0")];
    assert_eq!(
        HeaderSpec::from_captured(&no_dest, &no_dest),
        Err(HeaderSpecError::PriorityWithoutDestination)
    );
    let a = [("sec-fetch-dest", "document"), ("priority", "u=0, i")];
    let b = [("sec-fetch-dest", "document"), ("priority", "u=1")];
    assert_eq!(
        HeaderSpec::from_captured(&a, &b),
        Err(HeaderSpecError::DuplicateDestination("document".to_owned()))
    );
    let same = HeaderSpec::from_captured(&a, &a).expect("equal values are one entry");
    assert_eq!(same.priority_by_dest, owned(&[("document", "u=0, i")]));
}

#[test]
fn bad_captures_are_refused_by_name() {
    let dup = [("Accept", "a"), ("accept", "b")];
    assert_eq!(
        HeaderSpec::from_captured(&dup, &DOCUMENT),
        Err(HeaderSpecError::DuplicateName("accept".to_owned()))
    );
    let bad_name = [("Bad Name", "a")];
    assert_eq!(
        HeaderSpec::from_captured(&DOCUMENT, &bad_name),
        Err(HeaderSpecError::InvalidName("Bad Name".to_owned()))
    );
    let bad_value = [("X-A", "line\r\nbreak")];
    assert_eq!(
        HeaderSpec::from_captured(&bad_value, &DOCUMENT),
        Err(HeaderSpecError::InvalidValue {
            name: "X-A".to_owned()
        })
    );
    let many: Vec<(String, String)> = (0..=MAX_HEADERS)
        .map(|i| (format!("x-{i}"), "1".to_owned()))
        .collect();
    assert_eq!(
        HeaderSpec::from_captured(&many, &many),
        Err(HeaderSpecError::TooManyHeaders)
    );
}

fn with_document(entries: Vec<(&str, HeaderValue)>) -> HeaderSpec {
    HeaderSpec {
        document: entries
            .into_iter()
            .map(|(n, v)| (n.to_owned(), v))
            .collect(),
        ..HeaderSpec::default()
    }
}

#[test]
fn validation_names_each_kind_of_problem() {
    let err = |entries| with_document(entries).validate();
    assert_eq!(
        err(vec![("", lit("a"))]),
        Err(HeaderSpecError::InvalidName(String::new()))
    );
    for bad in ["a b", "a:b", "a\tb", "naïve", "a(b)", "a/b", "a@b", "a\"b"] {
        assert_eq!(
            err(vec![(bad, lit("a"))]),
            Err(HeaderSpecError::InvalidName(bad.to_owned())),
            "{bad:?}"
        );
    }
    let long = "n".repeat(MAX_NAME_LEN + 1);
    assert!(err(vec![(long.as_str(), lit("a"))]).is_err());
    assert_eq!(
        err(vec![(":path", lit("/"))]),
        Err(HeaderSpecError::PseudoHeader(":path".to_owned()))
    );
    for reserved in [
        "Host",
        "CONNECTION",
        "keep-alive",
        "proxy-connection",
        "Proxy-Authorization",
        "transfer-encoding",
        "upgrade",
        "content-length",
        "TE",
        "priority",
    ] {
        assert_eq!(
            err(vec![(reserved, lit("a"))]),
            Err(HeaderSpecError::ReservedHeader(
                reserved.to_ascii_lowercase()
            )),
            "{reserved}"
        );
    }
    assert_eq!(
        err(vec![("X-A", lit("1")), ("x-a", HeaderValue::Cookie)]),
        Err(HeaderSpecError::DuplicateName("x-a".to_owned()))
    );
    for bad in ["a\r\nb", "a\0", "\u{7f}", " a", "a ", "\ta"] {
        assert_eq!(
            err(vec![("X-A", lit(bad))]),
            Err(HeaderSpecError::InvalidValue {
                name: "X-A".to_owned()
            }),
            "{bad:?}"
        );
    }
    assert_eq!(
        err(vec![("X-A", lit(""))]),
        Ok(()),
        "an empty value is legal"
    );
    assert_eq!(
        err(vec![("X-A", lit("a\tb"))]),
        Ok(()),
        "inner tabs are legal"
    );
    assert_eq!(
        err(vec![("!#$%&'*+-.^_`|~09azAZ", lit("a"))]),
        Ok(()),
        "every token character"
    );
}

#[test]
fn the_priority_table_is_validated_too() {
    let table = |entries: &[(&str, &str)]| HeaderSpec {
        priority_by_dest: owned(entries),
        ..HeaderSpec::default()
    };
    assert_eq!(
        table(&[("document", "u=0, i"), ("empty", "u=1")]).validate(),
        Ok(())
    );
    assert_eq!(
        table(&[("document", "u=0"), ("Document", "u=1")]).validate(),
        Err(HeaderSpecError::DuplicateDestination("Document".to_owned()))
    );
    assert_eq!(
        table(&[("bad dest", "u=0")]).validate(),
        Err(HeaderSpecError::InvalidName("bad dest".to_owned()))
    );
    assert_eq!(
        table(&[("document", "u=0\r\n")]).validate(),
        Err(HeaderSpecError::InvalidValue {
            name: "priority".to_owned()
        })
    );
    let many: Vec<(String, String)> = (0..=MAX_PRIORITY_DESTINATIONS)
        .map(|i| (format!("d{i}"), "u=0".to_owned()))
        .collect();
    let too_many = HeaderSpec {
        priority_by_dest: many,
        ..HeaderSpec::default()
    };
    assert_eq!(
        too_many.validate(),
        Err(HeaderSpecError::TooManyDestinations)
    );
}

#[test]
fn with_header_replaces_in_place_or_appends() {
    let s = spec();
    let changed = s
        .with_header(RequestShape::Fetch, "accept", lit("application/json"))
        .expect("valid");
    assert_eq!(
        names(&changed.fetch),
        names(&s.fetch),
        "same names, same order"
    );
    assert_eq!(
        changed.fetch[1],
        ("Accept".to_owned(), lit("application/json"))
    );
    assert_eq!(changed.document, s.document, "the other shape is untouched");
    let added = s
        .with_header(RequestShape::Fetch, "X-New", lit("1"))
        .expect("valid");
    assert_eq!(added.fetch.last(), Some(&("X-New".to_owned(), lit("1"))));
    let swapped = s
        .with_header(
            RequestShape::Document,
            "Accept-Language",
            HeaderValue::UserAgent,
        )
        .expect("any placeholder can go anywhere");
    assert_eq!(swapped.document[10].1, HeaderValue::UserAgent);
    assert_eq!(s, spec(), "the original is not changed");
}

#[test]
fn with_header_refuses_what_validate_refuses() {
    let s = spec();
    for (name, value) in [
        ("Host", lit("x")),
        ("bad name", lit("x")),
        (":path", lit("/")),
        ("X-A", lit("a\nb")),
    ] {
        assert!(
            s.with_header(RequestShape::Fetch, name, value).is_err(),
            "{name}"
        );
    }
}

#[test]
fn without_header_removes_by_name_in_one_shape() {
    let s = spec();
    let fewer = s.without_header(RequestShape::Document, "COOKIE");
    assert_eq!(fewer.document.len(), s.document.len() - 1);
    assert!(fewer.document.iter().all(|(n, _)| n != "Cookie"));
    assert_eq!(fewer.fetch, s.fetch);
    assert_eq!(s.without_header(RequestShape::Fetch, "missing"), s);
}

#[test]
fn with_priority_sets_or_replaces_a_destination() {
    let s = spec().with_priority("document", "u=0, i").expect("valid");
    assert_eq!(s.priority_for("document"), Some("u=0, i"));
    let again = s.with_priority("Document", "u=1").expect("valid");
    assert_eq!(again.priority_by_dest.len(), 1);
    assert_eq!(again.priority_for("document"), Some("u=1"));
    assert!(s.with_priority("bad dest", "u=0").is_err());
    assert!(s.with_priority("image", "u=0\n").is_err());
}

#[test]
fn json_has_readable_shapes_and_round_trips() {
    let s = spec().with_priority("document", "u=0, i").expect("valid");
    let json = serde_json::to_string(&s).expect("serialises");
    assert!(json.contains("\"user_agent\""), "{json}");
    assert!(json.contains("{\"literal\":\"1\"}"), "{json}");
    assert_eq!(
        serde_json::from_str::<HeaderSpec>(&json).expect("deserialises"),
        s
    );
    let shape =
        serde_json::to_string(&[RequestShape::Document, RequestShape::Fetch]).expect("serialises");
    assert_eq!(shape, "[\"document\",\"fetch\"]");
    let sparse: HeaderSpec =
        serde_json::from_str(r#"{"fetch":[["Accept",{"literal":"*/*"}]]}"#).expect("defaults");
    assert_eq!(sparse.document, Vec::new());
    assert_eq!(sparse.fetch.len(), 1);
    assert!(
        serde_json::from_str::<HeaderSpec>(r#"{"fetches":[]}"#).is_err(),
        "typos are errors"
    );
    assert!(serde_json::from_str::<HeaderSpec>(r#"{"fetch":[["A","nothing"]]}"#).is_err());
}

/// A client start with the given regular headers, as a capture tool would receive it.
fn client_start(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut bytes = CLIENT_PREFACE.to_vec();
    bytes.extend(encode_frame(frame_type::SETTINGS, 0, 0, &[0, 4, 0, 0x10, 0, 0]).expect("frame"));
    let all = [
        (":method", "GET"),
        (":authority", "example.test"),
        (":scheme", "https"),
        (":path", "/"),
    ]
    .into_iter()
    .chain(fields.iter().copied());
    let mut block = Vec::new();
    for (name, value) in all {
        block.push(0x00);
        for text in [name, value] {
            block.push(u8::try_from(text.len()).expect("short test text"));
            block.extend(text.bytes());
        }
    }
    bytes.extend(encode_frame(frame_type::HEADERS, 0x04, 1, &block).expect("frame"));
    bytes
}

#[test]
fn a_spec_built_from_a_decoded_http2_capture_renders_it_back() {
    let sent = [
        ("user-agent", "TestAgent/1.0"),
        ("accept", "text/html"),
        ("sec-fetch-dest", "document"),
        ("accept-language", "en-US"),
        ("priority", "u=0, i"),
    ];
    let preface = read_preface(&client_start(&sent))
        .expect("valid")
        .expect("complete");
    let regular: Vec<(String, String)> = preface
        .regular_headers()
        .map(|f| (f.name.clone(), f.value.clone()))
        .collect();
    let spec = HeaderSpec::from_captured(&regular, &regular).expect("valid");
    assert_eq!(spec.priority_for("document"), Some("u=0, i"));
    let identity = HeaderIdentity::from_captured(&regular).expect("has a user agent");
    let rendered = spec
        .render(RequestShape::Document, &identity)
        .expect("renders");
    let without_priority: Vec<(String, String)> = regular
        .into_iter()
        .filter(|(n, _)| n != "priority")
        .collect();
    assert_eq!(rendered, without_priority);
}

/// A tiny deterministic generator; no clock, no dependency.
struct XorShift(u64);

impl XorShift {
    fn step(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.step() % u64::try_from(bound).expect("small bound")).expect("fits")
    }
}

#[test]
fn random_header_lists_are_either_refused_or_valid_and_render_cleanly() {
    const NAMES: [&str; 12] = [
        "user-agent",
        "Accept",
        "cookie",
        "SEC-CH-UA",
        "priority",
        "sec-fetch-dest",
        "host",
        "x-a",
        ":path",
        "bad name",
        "",
        "X-B",
    ];
    const VALUES: [&str; 8] = [
        "a", "", "document", " a", "a\r\nb", "u=0, i", "x\u{7f}", "ok value",
    ];
    let mut rng = XorShift(0x2545_f491_4f6c_dd1d);
    let identity = HeaderIdentity::new("UA/1").with_cookie("c=1");
    for _ in 0..3000 {
        let len = rng.below(8);
        let list: Vec<(&str, &str)> = (0..len)
            .map(|_| {
                (
                    NAMES[rng.below(NAMES.len())],
                    VALUES[rng.below(VALUES.len())],
                )
            })
            .collect();
        if let Ok(built) = HeaderSpec::from_captured(&list, &list) {
            assert_eq!(built.validate(), Ok(()), "{list:?}");
            let rendered = built
                .render(RequestShape::Document, &identity)
                .expect("a valid spec renders");
            assert!(rendered.len() <= list.len());
        }
    }
}
