//! Tests of the overlays: custom JA3, JA4 and peetprint strings layered over a base spec.

use super::overlay::versions_with_max;
use super::tests::{
    chrome, duplicate, hello, ja3, refused, sigalg, CHROME_JA3, CHROME_JA3_HASH, CIPHERS,
};
use super::{KnownGap, SpecError, TlsSpec};
use crate::wire::client_hello::ext_id;
use crate::wire::client_hello::fixtures::{HelloBuilder, CHROME_CIPHERS, JA4_EXAMPLE_EXTENSIONS};
use crate::wire::ja3::peetprint::{PeetField, Slot};
use crate::wire::ja3::{Ja3, Peetprint};
use crate::wire::ja4::{Ja4Order, Ja4Section};

#[test]
fn a_custom_ja3_overlays_the_base_and_reproduces_exactly() {
    let base = chrome();
    let wanted = Ja3::parse(CHROME_JA3).expect("parses");
    let spec = base
        .with_ja3(&wanted)
        .expect("the published JA3 is reproducible");
    assert_eq!(spec.ja3(), wanted);
    assert_eq!(spec.ja3().hash(), CHROME_JA3_HASH);
    assert_eq!(spec.extensions, wanted.extensions);
    assert!(spec.alps.is_none(), "the JA3 has no ALPS extension");
    assert_eq!(
        spec.signature_algorithms, base.signature_algorithms,
        "content comes from the base"
    );
    assert!(spec.grease && !spec.permute_extensions);
    assert_eq!(spec.check_reproducible(), Ok(()));
    assert_eq!(spec.with_ja3(&spec.ja3()), Ok(spec.clone()));
    let json = serde_json::to_string(&spec).expect("serializes");
    assert_eq!(
        serde_json::from_str::<TlsSpec>(&json).expect("deserializes"),
        spec
    );
}

#[test]
fn a_custom_ja3_turns_extension_permutation_off() {
    let mut base = chrome();
    base.permute_extensions = true;
    assert_eq!(
        base.with_ja3(&base.ja3())
            .map(|spec| spec.permute_extensions),
        Ok(false)
    );
}

#[test]
fn a_custom_ja3_prunes_key_shares_to_the_groups_it_offers() {
    let spec = chrome()
        .with_ja3(&ja3("0-10-11-13-16-43-51", "23-24", "0"))
        .expect("reproducible");
    assert_eq!(spec.key_share_groups, [0x0017]);
    assert_eq!(spec.supported_groups, [0x0017, 0x0018]);
}

#[test]
fn rejects_custom_ja3_strings_boringssl_cannot_send() {
    let exts = "0-10-11-13-16-43-51";
    let cases: Vec<(&str, Ja3, SpecError)> = vec![
        (
            "version",
            Ja3::parse("770,4865,0,29,0").expect("parses"),
            SpecError::LegacyVersion(770),
        ),
        (
            "reference example",
            Ja3::parse("769,47-53-5-10-49161,0-10-11,23-24-25,0").expect("parses"),
            SpecError::LegacyVersion(769),
        ),
        (
            "DHE cipher",
            Ja3::parse(&format!("771,{CIPHERS}-51,{exts},29,0")).expect("parses"),
            SpecError::UnsupportedCipher(51),
        ),
        (
            "TLS 1.3 subset",
            Ja3::parse(&format!("771,4865-47,{exts},29,0")).expect("parses"),
            SpecError::Tls13Suites,
        ),
        (
            "pre_shared_key",
            ja3("0-10-11-13-16-41-43-51", "29", "0"),
            refused(41),
        ),
        (
            "repeated extension",
            ja3("0-0-10-11-13-16-43-51", "29", "0"),
            SpecError::DuplicateExtension(0),
        ),
        (
            "x448",
            ja3(exts, "29-30", "0"),
            SpecError::UnsupportedGroup(30),
        ),
        (
            "compressed points",
            ja3(exts, "29", "1"),
            SpecError::PointFormats(vec![1]),
        ),
        (
            "points without the extension",
            ja3("0-10-13-16-43-51", "29", "0"),
            SpecError::PointFormats(vec![0]),
        ),
        (
            "groups missing",
            ja3(exts, "", "0"),
            SpecError::ExtensionContent(0x000A),
        ),
    ];
    for (label, wanted, want) in cases {
        assert_eq!(chrome().with_ja3(&wanted), Err(want), "{label}");
    }
}

#[test]
fn a_custom_ja3_cannot_invent_content_the_base_lacks() {
    let mut base = chrome();
    base.alpn.clear();
    base.extensions.retain(|&id| id != ext_id::ALPN);
    let wanted = ja3("0-10-11-13-16-43-51", "29", "0");
    assert_eq!(
        base.with_ja3(&wanted),
        Err(SpecError::ExtensionContent(ext_id::ALPN))
    );
}

#[test]
fn custom_signature_algorithms_replace_the_base_list() {
    let base = chrome();
    let spec = base
        .with_signature_algorithms(&[0x0804, 0x0403])
        .expect("reproducible");
    assert_eq!(spec.signature_algorithms, [0x0804, 0x0403]);
    assert_eq!(spec.check_reproducible(), Ok(()));
    let cases: Vec<(&[u16], SpecError)> = vec![
        (&[0x0403, 0x0904], sigalg(0x0904, "ml_dsa_44")),
        (&[0x0403, 0x0403], duplicate("signature_algorithms", 0x0403)),
        (&[0x0808], sigalg(0x0808, "ed448")),
        (
            &[],
            SpecError::ExtensionContent(ext_id::SIGNATURE_ALGORITHMS),
        ),
    ];
    for (ids, want) in cases {
        assert_eq!(base.with_signature_algorithms(ids), Err(want), "{ids:?}");
    }
    let mut without = base;
    without
        .extensions
        .retain(|&id| id != ext_id::SIGNATURE_ALGORITHMS);
    without.signature_algorithms.clear();
    assert_eq!(
        without.with_signature_algorithms(&[0x0403]),
        Err(SpecError::ExtensionContent(ext_id::SIGNATURE_ALGORITHMS))
    );
}

/// A copy of the base without one cipher and the session ticket extension.
fn trimmed() -> TlsSpec {
    let mut spec = chrome();
    spec.cipher_suites.retain(|&id| id != 0x0035);
    spec.extensions.retain(|&id| id != ext_id::SESSION_TICKET);
    spec
}

#[test]
fn a_custom_ja4_raw_overlays_the_base_in_both_orders() {
    for order in [Ja4Order::Sorted, Ja4Order::Original] {
        let base = chrome();
        assert_eq!(
            base.with_ja4_raw(&base.ja4_raw(order), order),
            Ok(base.clone()),
            "{order:?}"
        );
        let wanted = trimmed();
        let spec = base
            .with_ja4_raw(&wanted.ja4_raw(order), order)
            .expect("reproducible");
        assert_eq!(spec.ja4(), wanted.ja4(), "{order:?}");
        assert!(!spec.cipher_suites.contains(&0x0035) && !spec.session_ticket());
        assert_eq!(spec.check_reproducible(), Ok(()));
    }
}

#[test]
fn the_original_order_ja4_pins_the_order() {
    let mut reversed = chrome();
    reversed.cipher_suites.reverse();
    reversed.extensions.reverse();
    let mut base = chrome();
    base.permute_extensions = true;
    let order = Ja4Order::Original;
    let spec = base
        .with_ja4_raw(&reversed.ja4_raw(order), order)
        .expect("reproducible");
    assert_eq!(spec.cipher_suites, reversed.cipher_suites);
    assert_eq!(spec.extensions, reversed.extensions);
    assert!(!spec.permute_extensions);
    let sorted = base
        .with_ja4_raw(&reversed.ja4_raw(Ja4Order::Sorted), Ja4Order::Sorted)
        .expect("reproducible");
    assert_eq!(
        sorted.cipher_suites, base.cipher_suites,
        "the sorted form keeps the base order"
    );
    assert!(sorted.permute_extensions);
}

#[test]
fn rejects_custom_ja4_strings_that_cannot_be_reproduced() {
    let base = chrome();
    let sorted = base.ja4_raw(Ja4Order::Sorted);
    let with = |edit: fn(&mut crate::wire::ja4::Ja4Raw)| {
        let mut raw = sorted.clone();
        edit(&mut raw);
        base.with_ja4_raw(&raw, Ja4Order::Sorted)
    };
    assert_eq!(
        with(|r| r.ciphers.push(0x0033)),
        Err(SpecError::UnsupportedCipher(0x0033))
    );
    assert_eq!(
        with(|r| r.ciphers.push(0x1301)),
        Err(duplicate("cipher_suites", 0x1301))
    );
    assert_eq!(
        with(|r| r.a = "t12d1516h2".to_owned()),
        Err(SpecError::Tls13Suites),
        "a 1.2 client cannot send the TLS 1.3 suites"
    );
    assert_eq!(
        with(|r| r.a = "t13d".to_owned()),
        Err(SpecError::Ja4Mismatch(Ja4Section::A))
    );
    assert_eq!(
        with(|r| r.signature_algorithms.push(0x0904)),
        Err(sigalg(0x0904, "ml_dsa_44"))
    );
    assert_eq!(
        with(|r| r.a = "t99d1516h2".to_owned()),
        Err(SpecError::Ja4Mismatch(Ja4Section::A))
    );
    assert_eq!(
        with(|r| r.a = "t13d1516zz".to_owned()),
        Err(SpecError::Ja4Alpn("zz".to_owned()))
    );
    assert_eq!(
        with(|r| r.a = "t13d2016h2".to_owned()),
        Err(SpecError::Ja4Mismatch(Ja4Section::A))
    );
    assert_eq!(
        with(|r| r.extensions.insert(0, 0x0000)),
        Err(SpecError::Ja4Mismatch(Ja4Section::Extensions))
    );
    assert_eq!(
        with(|r| r.extensions.retain(|&id| id != 0x000D)),
        Err(SpecError::ExtensionContent(ext_id::SIGNATURE_ALGORITHMS))
    );
    let original = base.ja4_raw(Ja4Order::Original);
    let mut no_sni = original;
    no_sni.extensions.retain(|&id| id != ext_id::SERVER_NAME);
    assert_eq!(
        base.with_ja4_raw(&no_sni, Ja4Order::Original),
        Err(SpecError::Ja4Mismatch(Ja4Section::A))
    );
}

#[test]
fn the_ja4_alpn_code_selects_the_name_that_goes_first() {
    let base = chrome();
    let mut raw = base.ja4_raw(Ja4Order::Sorted);
    for (code, want) in [
        ("h1", vec!["http/1.1", "h2"]),
        ("h0", vec!["http/1.0", "h2", "http/1.1"]),
        ("h2", vec!["h2", "http/1.1"]),
    ] {
        raw.a = format!("t13d1516{code}");
        let spec = base.with_ja4_raw(&raw, Ja4Order::Sorted).expect(code);
        assert_eq!(spec.alpn, want, "{code}");
        assert_eq!(spec.ja4_raw(Ja4Order::Sorted), raw, "{code}");
    }
}

#[test]
fn the_highest_offered_version_follows_the_ja4_version_code() {
    assert_eq!(versions_with_max(&[0x0303], 0x0304), [0x0304, 0x0303]);
    assert_eq!(versions_with_max(&[], 0x0304), [0x0304]);
    assert_eq!(versions_with_max(&[0x0304, 0x0303], 0x0303), [0x0303]);
    assert_eq!(
        versions_with_max(&[0x0301, 0x0304], 0x0302),
        [0x0302, 0x0301]
    );
}

#[test]
fn a_custom_peetprint_overlays_the_base_and_reproduces_exactly() {
    let base = chrome();
    assert_eq!(base.with_peetprint(&base.peetprint()), Ok(base.clone()));
    let mut wanted = base.peetprint();
    wanted.ciphers.retain(|slot| *slot != Slot::Id(53));
    wanted.groups = vec![Slot::Grease, Slot::Id(29), Slot::Id(23)];
    let spec = base.with_peetprint(&wanted).expect("reproducible");
    assert!(!spec.cipher_suites.contains(&0x0035));
    assert_eq!(spec.supported_groups, [29, 23]);
    assert_eq!(spec.key_share_groups, [29]);
    assert_eq!(spec.peetprint().normalized(), wanted.clone().normalized());
    assert_eq!(spec.check_reproducible(), Ok(()));
}

#[test]
fn a_peetprint_without_grease_switches_the_flag_off() {
    let base = chrome();
    let plain = TlsSpec::from_client_hello(&hello(&HelloBuilder::chrome_like()));
    let spec = base
        .with_peetprint(&plain.peetprint())
        .expect("reproducible");
    assert!(!spec.grease);
    assert_eq!(spec.peetprint(), plain.peetprint());
}

#[test]
fn rejects_custom_peetprints_that_cannot_be_reproduced() {
    let base = chrome();
    let peet = base.peetprint();
    let with = |edit: fn(&mut Peetprint)| {
        let mut wanted = peet.clone();
        edit(&mut wanted);
        base.with_peetprint(&wanted)
    };
    assert_eq!(
        with(|p| p.ciphers.insert(0, Slot::Grease)),
        Err(SpecError::PeetprintMismatch(PeetField::Ciphers))
    );
    assert_eq!(
        with(|p| p.groups.push(Slot::Id(30))),
        Err(SpecError::UnsupportedGroup(30))
    );
    assert_eq!(
        with(|p| p.extensions.push(Slot::Id(0))),
        Err(SpecError::DuplicateExtension(0))
    );
    assert_eq!(with(|p| p.extensions.push(Slot::Id(41))), Err(refused(41)));
    assert_eq!(
        with(|p| p.psk_modes.clear()),
        Err(SpecError::ExtensionContent(ext_id::PSK_KEY_EXCHANGE_MODES))
    );
    assert_eq!(
        with(|p| p
            .ciphers
            .retain(|s| !matches!(s, Slot::Id(0x1300..=0x13FF)))),
        Err(SpecError::Tls13Suites)
    );
    assert_eq!(
        with(|p| p.ciphers.push(Slot::Id(0x0033))),
        Err(SpecError::UnsupportedCipher(0x0033))
    );
}

#[test]
fn known_gaps_are_removed_on_request_and_only_then() {
    let mut measured = chrome();
    measured.extensions.insert(0, ext_id::TRUST_ANCHORS);
    measured
        .signature_algorithms
        .extend([0x0904, 0x0905, 0x0906]);
    let both = [
        KnownGap::TrustAnchorsExtension,
        KnownGap::MlDsaSignatureAlgorithms,
    ];
    assert_eq!(measured.check_reproducible(), Err(refused(0xCA34)));
    assert_eq!(
        measured.without_known_gaps(&both).check_reproducible(),
        Ok(())
    );
    assert_eq!(measured.without_known_gaps(&both), chrome());
    let only_extension = measured.without_known_gaps(&both[..1]);
    assert_eq!(
        only_extension.check_reproducible(),
        Err(sigalg(0x0904, "ml_dsa_44"))
    );
    assert_eq!(measured.without_known_gaps(&[]), measured);
}

#[test]
fn comparing_hellos_detects_extension_permutation() {
    let plain = HelloBuilder::chrome_like();
    let spec = TlsSpec::from_client_hello(&hello(&plain));
    let mut shuffled_ids = JA4_EXAMPLE_EXTENSIONS.to_vec();
    shuffled_ids.rotate_left(5);
    let shuffled = hello(&HelloBuilder::new(&CHROME_CIPHERS, &shuffled_ids));
    assert_eq!(
        spec.with_observed_order(&shuffled)
            .map(|s| s.permute_extensions),
        Ok(true)
    );
    assert_eq!(
        spec.with_observed_order(&hello(&plain))
            .map(|s| s.permute_extensions),
        Ok(false)
    );
    let fewer = hello(&HelloBuilder::new(
        &CHROME_CIPHERS,
        &JA4_EXAMPLE_EXTENSIONS[..10],
    ));
    assert_eq!(
        spec.with_observed_order(&fewer),
        Err(SpecError::HellosDiffer)
    );
    let other_ciphers = hello(&HelloBuilder::new(
        &CHROME_CIPHERS[..10],
        &JA4_EXAMPLE_EXTENSIONS,
    ));
    assert_eq!(
        spec.with_observed_order(&other_ciphers),
        Err(SpecError::HellosDiffer)
    );
}

#[test]
fn error_messages_name_the_offending_value() {
    let text = SpecError::UnsupportedSigalg {
        id: 0x0904,
        name: "ml_dsa_44",
    }
    .to_string();
    assert!(
        text.contains("ml_dsa_44") && text.contains("0904"),
        "{text}"
    );
    let text = SpecError::UnsupportedCipher(0x0033).to_string();
    assert!(text.contains("0033"), "{text}");
}
