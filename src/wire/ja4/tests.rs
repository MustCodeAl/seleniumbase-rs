//! Tests of JA4: the published example vectors, every `a` field rule and the parsers.

use super::{
    alpn_code, version_code, version_from_code, Ja4, Ja4AField, Ja4Error, Ja4Facts, Ja4Order,
    Ja4Raw, Ja4Section, MAX_JA4_LEN,
};
use crate::wire::client_hello::fixtures::{
    HelloBuilder, CHROME_CIPHERS, GREASE_CIPHER, JA4_EXAMPLE_EXTENSIONS,
};
use crate::wire::client_hello::{ext_id, ClientHello};

/// The JA4 of the public example hello, from the reference documentation.
const JA4: &str = "t13d1516h2_8daaf6152771_e5627efa2ab1";
/// The same hello with ciphers and extensions in original order (`JA4_o`).
const JA4_O: &str = "t13d1516h2_acb858a92679_18f69afefd3d";
/// `JA4_r`: sorted and unhashed.
const JA4_R: &str = "t13d1516h2_002f,0035,009c,009d,1301,1302,1303,c013,c014,c02b,c02c,c02f,c030,cca8,cca9_0005,000a,000b,000d,0012,0015,0017,001b,0023,002b,002d,0033,4469,ff01_0403,0804,0401,0503,0805,0501,0806,0601";
/// `JA4_ro`: original order and unhashed.
const JA4_RO: &str = "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_001b,0000,0033,0010,4469,0017,002d,000d,0005,0023,0012,002b,ff01,000b,000a,0015_0403,0804,0401,0503,0805,0501,0806,0601";
/// The `c` hash of the example when the signature algorithms are left out.
const C_WITHOUT_SIGALGS: &str = "6d807ffa2a79";

fn parse_hello(builder: &HelloBuilder) -> ClientHello {
    ClientHello::parse(&builder.record()).expect("the fixture hello parses")
}

fn facts() -> Ja4Facts {
    Ja4Facts::of(&parse_hello(&HelloBuilder::chrome_like()))
}

#[test]
fn the_published_example_hashes_to_the_published_fingerprint() {
    let ja4 = Ja4::of(&parse_hello(&HelloBuilder::chrome_like()));
    assert_eq!(ja4.to_string(), JA4);
    assert_eq!(
        (ja4.a.as_str(), ja4.b.as_str()),
        ("t13d1516h2", "8daaf6152771")
    );
    assert_eq!(ja4.c, "e5627efa2ab1");
}

#[test]
fn grease_changes_nothing() {
    let plain = parse_hello(&HelloBuilder::chrome_like());
    let greased = parse_hello(&HelloBuilder::chrome_like().with_grease());
    assert_eq!(Ja4::of(&greased), Ja4::of(&plain));
    assert_eq!(Ja4::raw(&greased), Ja4::raw(&plain));
    assert_eq!(
        Ja4::of_original_order(&greased),
        Ja4::of_original_order(&plain)
    );
}

#[test]
fn the_original_order_and_raw_variants_match_the_published_strings() {
    let hello = parse_hello(&HelloBuilder::chrome_like());
    assert_eq!(Ja4::of_original_order(&hello).to_string(), JA4_O);
    assert_eq!(Ja4::raw(&hello), JA4_R);
    assert_eq!(Ja4::raw_original_order(&hello), JA4_RO);
}

#[test]
fn the_sorted_fingerprint_ignores_order_and_the_original_order_one_does_not() {
    let mut extensions = JA4_EXAMPLE_EXTENSIONS.to_vec();
    extensions.reverse();
    let mut ciphers = CHROME_CIPHERS.to_vec();
    ciphers.reverse();
    let shuffled = parse_hello(&HelloBuilder::new(&ciphers, &extensions));
    let original = parse_hello(&HelloBuilder::chrome_like());
    assert_eq!(Ja4::of(&shuffled), Ja4::of(&original));
    assert_eq!(Ja4::raw(&shuffled), Ja4::raw(&original));
    let (shuffled_o, original_o) = (
        Ja4::of_original_order(&shuffled),
        Ja4::of_original_order(&original),
    );
    assert_eq!(shuffled_o.a, original_o.a);
    assert_ne!(shuffled_o.b, original_o.b);
    assert_ne!(shuffled_o.c, original_o.c);
}

#[test]
fn without_signature_algorithms_the_hash_covers_the_extensions_only() {
    let builder = HelloBuilder::chrome_like().with_body(ext_id::SIGNATURE_ALGORITHMS, vec![0, 0]);
    let hello = parse_hello(&builder);
    let ja4 = Ja4::of(&hello);
    assert_eq!(ja4.c, C_WITHOUT_SIGALGS);
    assert_eq!(ja4.a, "t13d1516h2");
    let raw = Ja4::raw(&hello);
    assert!(raw.ends_with(",ff01"), "no trailing section: {raw}");
    assert_eq!(raw.matches('_').count(), 2);
}

#[test]
fn the_alpn_field_follows_the_reference_rules() {
    let cases: [(&[u8], &str); 14] = [
        (b"h2", "h2"),
        (b"http/1.1", "h1"),
        (b"http/1.0", "h0"),
        (b"a", "aa"),
        (&[], "00"),
        (&[0xAB], "ab"),
        (&[0x20], "20"),
        (&[0xAB, 0xCD], "ad"),
        (&[0x20, 0x61], "21"),
        (&[0x30, 0xAB], "3b"),
        (&[0x61, 0x20], "60"),
        (&[0x30, 0x31, 0xAB, 0xCD], "3d"),
        (&[0x30, 0xAB, 0xCD, 0x31], "01"),
        (b"spdy/3", "s3"),
    ];
    for (value, want) in cases {
        assert_eq!(alpn_code(Some(value)), want, "{value:?}");
    }
    assert_eq!(alpn_code(None), "00");
}

#[test]
fn the_version_field_uses_the_highest_non_grease_supported_version() {
    let mut facts = facts();
    facts.supported_versions = vec![0x4A4A, 0x0303, 0x0304, 0x0302];
    assert!(facts.a().starts_with("t13"));
    facts.supported_versions = vec![0x0A0A];
    assert!(
        facts.a().starts_with("t12"),
        "falls back to the handshake version"
    );
    facts.supported_versions.clear();
    facts.legacy_version = 0x0301;
    assert!(facts.a().starts_with("t10"));
    let table = [
        (0x0304, "13"),
        (0x0303, "12"),
        (0x0302, "11"),
        (0x0301, "10"),
        (0x0300, "s3"),
        (0x0002, "s2"),
        (0xFEFF, "d1"),
        (0xFEFD, "d2"),
        (0xFEFC, "d3"),
        (0x1234, "00"),
    ];
    for (version, code) in table {
        assert_eq!(version_code(version), code);
        if code != "00" {
            assert_eq!(version_from_code(code), Some(version));
        }
    }
    assert_eq!(version_from_code("zz"), None);
}

#[test]
fn the_sni_flag_and_the_counts_follow_the_reference_rules() {
    let mut facts = facts();
    assert_eq!(&facts.a()[3..4], "d");
    facts.extensions.retain(|&id| id != ext_id::SERVER_NAME);
    assert_eq!(&facts.a()[3..4], "i");
    assert_eq!(&facts.a()[6..8], "15", "SNI no longer counted");
    facts.ciphers = (0..120).map(|id| 0x1000 + id).collect();
    facts.ciphers.push(GREASE_CIPHER);
    assert_eq!(&facts.a()[4..6], "99", "capped at 99, GREASE not counted");
    facts.ciphers = vec![GREASE_CIPHER, 0x00FF, 0x5600, 0xFE01];
    assert_eq!(&facts.a()[4..6], "03", "SCSV and reserved values count");
}

#[test]
fn empty_lists_hash_to_zeros() {
    let mut facts = facts();
    facts.ciphers.clear();
    assert_eq!(facts.ja4(Ja4Order::Sorted).b, "000000000000");
    facts.extensions = vec![ext_id::SERVER_NAME, ext_id::ALPN];
    assert_eq!(
        facts.ja4(Ja4Order::Sorted).c,
        "000000000000",
        "nothing left after exclusions"
    );
    assert_eq!(facts.ja4(Ja4Order::Original).c.len(), 12);
    assert_ne!(facts.ja4(Ja4Order::Original).c, "000000000000");
    facts.extensions.clear();
    assert_eq!(facts.ja4(Ja4Order::Original).c, "000000000000");
    assert_eq!(&facts.a()[6..8], "00");
}

#[test]
fn raw_strings_parse_and_print_unchanged() {
    for text in [JA4_R, JA4_RO] {
        let raw = Ja4Raw::parse(text).expect("a published raw fingerprint parses");
        assert_eq!(raw.to_string(), text);
    }
    let raw = Ja4Raw::parse(JA4_R).expect("parses");
    assert_eq!(raw.hashed().to_string(), JA4);
    assert_eq!(raw.ciphers.len(), 15);
    assert_eq!(raw.signature_algorithms.len(), 8);
    let ordered = Ja4Raw::parse(JA4_RO).expect("parses");
    assert_eq!(ordered.hashed().to_string(), JA4_O);
    assert_eq!(facts().raw(Ja4Order::Sorted), raw);
    assert_eq!(facts().raw(Ja4Order::Original), ordered);
}

#[test]
fn raw_strings_without_signature_algorithms_have_three_sections() {
    let raw = Ja4Raw::parse("t13d0101h2_1301_000a,000b").expect("parses");
    assert!(raw.signature_algorithms.is_empty());
    assert_eq!(raw.to_string(), "t13d0101h2_1301_000a,000b");
    let gap = Ja4Raw::parse("t13d0101h2_1301__0403").expect("an empty extension section parses");
    assert!(gap.extensions.is_empty());
    assert_eq!(gap.to_string(), "t13d0101h2_1301__0403");
}

#[test]
fn rejects_malformed_raw_strings_with_the_failing_section() {
    let hex = |section, token: &str| {
        Err(Ja4Error::Hex {
            section,
            token: token.to_owned(),
        })
    };
    let cases: Vec<(&str, Result<Ja4Raw, Ja4Error>)> = vec![
        ("", Err(Ja4Error::Sections(1))),
        ("t13d1516h2_1301", Err(Ja4Error::Sections(2))),
        ("t13d1516h2_1_2_3_4", Err(Ja4Error::Sections(5))),
        (
            "x13d1516h2_1301_000a",
            Err(Ja4Error::PartA("x13d1516h2".to_owned())),
        ),
        (
            "t13d1516_1301_000a",
            Err(Ja4Error::PartA("t13d1516".to_owned())),
        ),
        (
            "t13x1516h2_1301_000a",
            Err(Ja4Error::PartA("t13x1516h2".to_owned())),
        ),
        (
            "t13d15a6h2_1301_000a",
            Err(Ja4Error::PartA("t13d15a6h2".to_owned())),
        ),
        ("t13d1516h2_130_000a", hex(Ja4Section::Ciphers, "130")),
        (
            "t13d1516h2_1301,zzzz_000a",
            hex(Ja4Section::Ciphers, "zzzz"),
        ),
        ("t13d1516h2_1301_000a,", hex(Ja4Section::Extensions, "")),
        (
            "t13d1516h2_1301_000a_0403,12345",
            hex(Ja4Section::SignatureAlgorithms, "12345"),
        ),
        (
            "t13d1516h2_1301_000a_+403",
            hex(Ja4Section::SignatureAlgorithms, "+403"),
        ),
    ];
    for (text, want) in cases {
        assert_eq!(Ja4Raw::parse(text), want, "{text:?}");
    }
    let long = format!("t13d1516h2_{}_000a", "1301,".repeat(MAX_JA4_LEN));
    assert_eq!(Ja4Raw::parse(&long), Err(Ja4Error::TooLong));
}

#[test]
fn hashed_fingerprints_parse_and_print_unchanged() {
    let ja4 = Ja4::parse(JA4).expect("the published fingerprint parses");
    assert_eq!(ja4.to_string(), JA4);
    assert_eq!(ja4.a, "t13d1516h2");
    assert_eq!(JA4.parse::<Ja4>(), Ok(ja4));
}

#[test]
fn rejects_malformed_hashed_fingerprints() {
    let cases: Vec<(&str, Ja4Error)> = vec![
        ("t13d1516h2_8daaf6152771", Ja4Error::Sections(2)),
        (
            "t13d1516h2_8daaf6152771_e5627efa2ab1_x",
            Ja4Error::Sections(4),
        ),
        (
            "t13d1516h2x_8daaf6152771_e5627efa2ab1",
            Ja4Error::PartA("t13d1516h2x".to_owned()),
        ),
        (
            "t13d1516h2_8daaf615277_e5627efa2ab1",
            Ja4Error::Hash(Ja4Section::Ciphers),
        ),
        (
            "t13d1516h2_8DAAF6152771_e5627efa2ab1",
            Ja4Error::Hash(Ja4Section::Ciphers),
        ),
        (
            "t13d1516h2_8daaf6152771_e5627efa2abg",
            Ja4Error::Hash(Ja4Section::Extensions),
        ),
    ];
    for (text, want) in cases {
        assert_eq!(Ja4::parse(text), Err(want), "{text:?}");
    }
}

#[test]
fn diff_names_the_parts_that_differ() {
    let base = Ja4::parse(JA4).expect("parses");
    assert!(base.diff(&base).is_empty());
    let other_c = Ja4 {
        c: C_WITHOUT_SIGALGS.to_owned(),
        ..base.clone()
    };
    let diff = base.diff(&other_c);
    assert!(diff.c && !diff.b && diff.a_fields.is_empty());
    let other_a = Ja4::parse("q12i1617h3_8daaf6152771_e5627efa2ab1").expect("parses");
    let diff = base.diff(&other_a);
    assert_eq!(
        diff.a_fields,
        vec![
            Ja4AField::Protocol,
            Ja4AField::Version,
            Ja4AField::Sni,
            Ja4AField::CipherCount,
            Ja4AField::ExtensionCount,
            Ja4AField::Alpn
        ]
    );
    assert!(!diff.is_empty() && !diff.b && !diff.c);
    let other_b = Ja4 {
        b: "000000000000".to_owned(),
        ..base.clone()
    };
    assert!(base.diff(&other_b).b);
}

#[test]
fn fingerprints_round_trip_through_json() {
    let ja4 = Ja4::parse(JA4).expect("parses");
    let json = serde_json::to_string(&ja4).expect("serializes");
    assert_eq!(
        serde_json::from_str::<Ja4>(&json).expect("deserializes"),
        ja4
    );
    let raw = Ja4Raw::parse(JA4_R).expect("parses");
    let json = serde_json::to_string(&raw).expect("serializes");
    assert_eq!(
        serde_json::from_str::<Ja4Raw>(&json).expect("deserializes"),
        raw
    );
}

#[test]
fn section_names_read_naturally_in_messages() {
    let names = [
        (Ja4Section::A, "a"),
        (Ja4Section::Ciphers, "ciphers"),
        (Ja4Section::Extensions, "extensions"),
        (Ja4Section::SignatureAlgorithms, "signature algorithms"),
    ];
    for (section, name) in names {
        assert_eq!(section.to_string(), name);
    }
    let text = Ja4Error::Hash(Ja4Section::Extensions).to_string();
    assert!(text.contains("extensions"), "{text}");
}
