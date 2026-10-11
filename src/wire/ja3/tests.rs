//! Tests of the JA3 and peetprint parsers, the pure MD5 and the published example vectors.

use super::peetprint::{PeetField, Slot, MAX_PEETPRINT_LEN};
use super::{md5_hex, Ja3, Ja3Error, Ja3Field, Peetprint, PeetprintError, MAX_JA3_LEN};
use crate::wire::client_hello::fixtures::HelloBuilder;
use crate::wire::client_hello::ClientHello;

/// The JA3 of the widely published Chrome 85 style hello and its MD5.
const CHROME_JA3: &str = "771,4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,0-23-65281-10-11-35-16-5-13-18-51-45-43-27-21,29-23-24,0";
const CHROME_JA3_HASH: &str = "b32309a26951912be7dba376398abc3b";
/// The example in the JA3 reference documentation and its MD5.
const REFERENCE_JA3: &str = "769,47-53-5-10-49161-49162-49171-49172-50-56-19-4,0-10-11,23-24-25,0";
const REFERENCE_JA3_HASH: &str = "ada70206e40642a3e4461f35503241d5";

fn parse_hello(builder: &HelloBuilder) -> ClientHello {
    ClientHello::parse(&builder.record()).expect("the fixture hello parses")
}

#[test]
fn md5_matches_the_rfc_1321_test_suite() {
    let long_digits = "1234567890".repeat(8);
    let cases: [(&str, &str); 7] = [
        ("", "d41d8cd98f00b204e9800998ecf8427e"),
        ("a", "0cc175b9c0f1b6a831c399e269772661"),
        ("abc", "900150983cd24fb0d6963f7d28e17f72"),
        ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        (
            "abcdefghijklmnopqrstuvwxyz",
            "c3fcd3d76192e4007dfb496cca67e13b",
        ),
        (
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            "d174ab98d277d9f5a5611c2c9f419d9f",
        ),
        (long_digits.as_str(), "57edf4a22be3c955ac49da2e2107b67a"),
    ];
    for (input, want) in cases {
        assert_eq!(md5_hex(input.as_bytes()), want, "md5 of {input:?}");
    }
}

#[test]
fn md5_handles_every_padding_boundary() {
    let cases = [
        (55, "04364420e25c512fd958a70738aa8f72"),
        (56, "668a72d5ba17f08e62dabcafad6db14b"),
        (57, "693037871c4a9d3d8685018905cb530a"),
        (63, "7dc2ca208106a2f703567bdff99d8981"),
        (64, "c1bb4f81d892b2d57947682aeb252456"),
        (65, "1bc932052302d074bdec39795fe00cf6"),
        (119, "ab347a5f68c8a443cfcddc633f12c24f"),
        (120, "fb98667f98096de92620b64f46e1c5b5"),
        (128, "d69cb61a6ee87200676eb0d4b90edbcb"),
    ];
    for (len, want) in cases {
        assert_eq!(md5_hex(&vec![b'x'; len]), want, "{len} bytes");
    }
}

#[test]
fn the_published_examples_hash_to_their_published_md5() {
    for (text, hash) in [
        (CHROME_JA3, CHROME_JA3_HASH),
        (REFERENCE_JA3, REFERENCE_JA3_HASH),
    ] {
        let ja3 = Ja3::parse(text).expect("a published JA3 parses");
        assert_eq!(ja3.to_string(), text);
        assert_eq!(ja3.hash(), hash);
    }
}

#[test]
fn a_parsed_hello_yields_the_published_string_with_and_without_grease() {
    let plain = HelloBuilder::ja3_example();
    let ja3 = Ja3::of(&parse_hello(&plain));
    assert_eq!(ja3.to_string(), CHROME_JA3);
    assert_eq!(ja3.hash(), CHROME_JA3_HASH);
    assert_eq!(Ja3::of(&parse_hello(&plain.with_grease())), ja3);
}

#[test]
fn parsing_drops_grease_ids() {
    let ja3 = Ja3::parse("771,2570-4865,2570-0-10-6682,2570-29-23,0").expect("parses");
    assert_eq!(ja3.ciphers, vec![4865]);
    assert_eq!(ja3.extensions, vec![0, 10]);
    assert_eq!(ja3.groups, vec![29, 23]);
    assert_eq!(ja3.to_string(), "771,4865,0-10,29-23,0");
}

#[test]
fn empty_lists_are_allowed_where_a_hello_may_lack_them() {
    let ja3: Ja3 = "771,47,,,".parse().expect("parses");
    assert!(ja3.extensions.is_empty() && ja3.groups.is_empty() && ja3.point_formats.is_empty());
    assert_eq!(ja3.to_string(), "771,47,,,");
}

#[test]
fn rejects_malformed_strings_with_the_failing_field() {
    let number = |field, token: &str| {
        Err(Ja3Error::Number {
            field,
            token: token.to_owned(),
        })
    };
    let cases: Vec<(&str, Result<Ja3, Ja3Error>)> = vec![
        ("", Err(Ja3Error::FieldCount(1))),
        ("771,47,0,29", Err(Ja3Error::FieldCount(4))),
        ("771,47,0,29,0,1", Err(Ja3Error::FieldCount(6))),
        (",47,0,29,0", Err(Ja3Error::Empty(Ja3Field::Version))),
        ("771,,0,29,0", Err(Ja3Error::Empty(Ja3Field::Ciphers))),
        ("771,2570,0,29,0", Err(Ja3Error::Empty(Ja3Field::Ciphers))),
        ("x,47,0,29,0", number(Ja3Field::Version, "x")),
        ("65536,47,0,29,0", number(Ja3Field::Version, "65536")),
        ("771,47-,0,29,0", number(Ja3Field::Ciphers, "")),
        ("771,-47,0,29,0", number(Ja3Field::Ciphers, "")),
        ("771,+47,0,29,0", number(Ja3Field::Ciphers, "+47")),
        ("771,47,0-a,29,0", number(Ja3Field::Extensions, "a")),
        ("771,47,0,29-70000,0", number(Ja3Field::Groups, "70000")),
        ("771,47,0,29,256", number(Ja3Field::PointFormats, "256")),
        ("771,47,0,29, 0", number(Ja3Field::PointFormats, " 0")),
        ("771-1,47,0,29,0", number(Ja3Field::Version, "771-1")),
    ];
    for (text, want) in cases {
        assert_eq!(Ja3::parse(text), want, "{text:?}");
    }
    let long = format!("771,{},0,29,0", "47-".repeat(MAX_JA3_LEN));
    assert_eq!(Ja3::parse(&long), Err(Ja3Error::TooLong));
}

#[test]
fn the_error_messages_name_the_field() {
    let text = Ja3::parse("771,47,0,29-x,0")
        .expect_err("a bad group")
        .to_string();
    assert!(text.contains("groups") && text.contains('x'), "{text}");
}

#[test]
fn ja3_round_trips_through_json() {
    let ja3 = Ja3::parse(CHROME_JA3).expect("parses");
    let json = serde_json::to_string(&ja3).expect("serializes");
    assert_eq!(
        serde_json::from_str::<Ja3>(&json).expect("deserializes"),
        ja3
    );
}

/// The example from the public description of the format.
const PEET_EXAMPLE: &str = "GREASE-772-771|2-1.1|GREASE-29-23-24|1027-2052-1025-1283-2053-1281-2054-1537|1|2|GREASE-4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53|GREASE-0-23-65281-10-11-35-16-5-13-18-51-45-43-27-17513-GREASE-21-41";

#[test]
fn the_documented_peetprint_example_round_trips() {
    let parsed = Peetprint::parse(PEET_EXAMPLE).expect("the documented example parses");
    assert_eq!(parsed.to_string(), PEET_EXAMPLE);
    assert_eq!(
        parsed.versions,
        vec![Slot::Grease, Slot::Id(772), Slot::Id(771)]
    );
    assert_eq!(parsed.alpn, vec!["h2".to_owned(), "http/1.1".to_owned()]);
    assert_eq!(parsed.psk_modes, vec![1]);
    assert_eq!(parsed.cert_compression, vec![2]);
    assert_eq!(parsed.extension_ids().len(), 17);
    assert_eq!(parsed.hash(), md5_hex(PEET_EXAMPLE.as_bytes()));
    assert_eq!(parsed.to_string().parse::<Peetprint>(), Ok(parsed));
}

#[test]
fn a_parsed_hello_yields_a_sorted_peetprint() {
    let hello = parse_hello(&HelloBuilder::chrome_like().with_grease());
    let want = "GREASE-772-771|2-1.1|GREASE-29-23-24|1027-2052-1025-1283-2053-1281-2054-1537|1|2|GREASE-4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53|0-10-11-13-16-17513-18-21-23-27-35-43-45-5-51-65281-GREASE-GREASE";
    assert_eq!(Peetprint::of(&hello).to_string(), want);
    let shuffled = Peetprint::parse(PEET_EXAMPLE).expect("parses").normalized();
    assert!(shuffled.to_string().ends_with("-GREASE-GREASE"));
}

#[test]
fn grease_written_as_a_number_collapses_to_the_word() {
    let parsed = Peetprint::parse("2570-772|2|||||4865|0").expect("parses");
    assert_eq!(parsed.versions, vec![Slot::Grease, Slot::Id(772)]);
    assert!(parsed.groups.is_empty() && parsed.alpn == ["h2"]);
    assert_eq!(parsed.to_string(), "GREASE-772|2|||||4865|0");
}

#[test]
fn unknown_alpn_names_survive_as_hex() {
    let parsed = Peetprint::parse("772|x6833|29|1027|1|2|4865|0").expect("parses");
    assert_eq!(parsed.alpn, vec!["h3".to_owned()]);
    assert_eq!(parsed.to_string(), "772|x6833|29|1027|1|2|4865|0");
}

#[test]
fn rejects_malformed_peetprints_with_the_failing_field() {
    let token = |field, text: &str| {
        Err(PeetprintError::Token {
            field,
            token: text.to_owned(),
        })
    };
    let cases: Vec<(&str, Result<Peetprint, PeetprintError>)> = vec![
        ("", Err(PeetprintError::FieldCount(1))),
        ("1|2|3|4|5|6|7", Err(PeetprintError::FieldCount(7))),
        ("1|2|3|4|5|6|7|8|9", Err(PeetprintError::FieldCount(9))),
        (
            "x|2|29|1027|1|2|4865|0",
            token(super::peetprint::PeetField::Versions, "x"),
        ),
        (
            "772|3|29|1027|1|2|4865|0",
            token(super::peetprint::PeetField::Alpn, "3"),
        ),
        (
            "772|xZZ|29|1027|1|2|4865|0",
            token(super::peetprint::PeetField::Alpn, "xZZ"),
        ),
        (
            "772|x|29|1027|1|2|4865|0",
            token(super::peetprint::PeetField::Alpn, "x"),
        ),
        (
            "772|2|29|1027|256|2|4865|0",
            token(super::peetprint::PeetField::PskModes, "256"),
        ),
        (
            "772|2|29|1027|1|70000|4865|0",
            token(super::peetprint::PeetField::CertCompression, "70000"),
        ),
        (
            "772|2|29|1027|1|2|4865|0-",
            token(super::peetprint::PeetField::Extensions, ""),
        ),
    ];
    for (text, want) in cases {
        assert_eq!(Peetprint::parse(text), want, "{text:?}");
    }
    let long = "1".repeat(MAX_PEETPRINT_LEN + 1);
    assert_eq!(Peetprint::parse(&long), Err(PeetprintError::TooLong));
}

#[test]
fn rejects_malformed_alpn_hex_forms() {
    for text in ["x683", "xFF", "x+f", "xzz"] {
        let peet = format!("772|{text}|29|1027|1|2|4865|0");
        assert_eq!(
            Peetprint::parse(&peet),
            Err(PeetprintError::Token {
                field: PeetField::Alpn,
                token: text.to_owned()
            }),
            "{text}"
        );
    }
}

#[test]
fn field_names_read_naturally_in_messages() {
    let ja3_names = [
        (Ja3Field::Version, "version"),
        (Ja3Field::Ciphers, "ciphers"),
        (Ja3Field::Extensions, "extensions"),
        (Ja3Field::Groups, "groups"),
        (Ja3Field::PointFormats, "point formats"),
    ];
    for (field, name) in ja3_names {
        assert_eq!(field.to_string(), name);
    }
    let peet_names = [
        (PeetField::Versions, "versions"),
        (PeetField::Alpn, "alpn"),
        (PeetField::Groups, "groups"),
        (PeetField::SignatureAlgorithms, "signature algorithms"),
        (PeetField::PskModes, "psk modes"),
        (PeetField::CertCompression, "certificate compression"),
        (PeetField::Ciphers, "ciphers"),
        (PeetField::Extensions, "extensions"),
    ];
    for (field, name) in peet_names {
        assert_eq!(field.to_string(), name);
    }
}
