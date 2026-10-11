//! Tests of the events and the two sinks that live with them.

use std::collections::BTreeSet;

use super::fixtures::{every_event, SECRETS};
use super::*;
use crate::scrape::passport::identity::fixtures::egress;
use crate::scrape::passport::policy::DemoteReason;
use crate::stealth::humanize::Rng;

fn json(event: &ScraperEvent) -> serde_json::Value {
    serde_json::to_value(event).expect("an event serializes")
}

#[test]
fn every_event_is_tagged_with_its_snake_case_name() {
    let events = every_event();
    let mut names = BTreeSet::new();
    for event in &events {
        let tag = json(event)["event"].as_str().expect("a tag").to_owned();
        assert_eq!(tag, event.name());
        assert!(
            tag.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "{tag}"
        );
        names.insert(tag);
    }
    assert_eq!(names.len(), events.len(), "each variant has its own name");
}

#[test]
fn urls_reach_an_event_only_redacted() {
    for event in every_event() {
        let text = json(&event).to_string();
        let debug = format!("{event:?}");
        for secret in SECRETS {
            assert!(!text.contains(secret), "{secret} leaked into {text}");
            assert!(!debug.contains(secret), "{secret} leaked into {debug}");
        }
    }
    let request = json(&every_event()[0]);
    let shown = request["url"].as_str().expect("a url");
    assert!(
        shown.contains("example.com/path") && shown.contains("page=2"),
        "{shown}"
    );
    assert!(shown.contains("token=***"), "{shown}");
}

#[test]
fn no_random_secret_in_a_url_survives_into_an_event() {
    let mut rng = Rng::new(0x000E_C0DE);
    for _ in 0..64 {
        let password = format!("pw{:016x}", rng.next_u64());
        let token = format!("tk{:016x}", rng.next_u64());
        let raw = format!("https://bob:{password}@host.test/a?api_key={token}&q=1");
        let event = ScraperEvent::Request {
            url: RedactedUrl::new(&raw),
            status: 204,
            mode: Mode::Browser,
            elapsed_ms: 1,
        };
        for text in [json(&event).to_string(), format!("{event:?}")] {
            assert!(
                !text.contains(&password) && !text.contains(&token),
                "{text}"
            );
        }
    }
}

#[test]
fn a_proxy_enters_an_event_as_an_egress_key_only() {
    let event = ScraperEvent::EgressRotated {
        from: Some(egress("p1.example")),
        to: Some(egress("p2.example")),
    };
    let value = json(&event);
    let keys: BTreeSet<&str> = value["from"]
        .as_object()
        .expect("a key")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, BTreeSet::from(["scheme", "host", "port", "country"]));
    assert_eq!(value["to"]["host"], "p2.example");
    assert_eq!(value["to"]["port"], 8080);
}

#[test]
fn the_direct_connection_is_null() {
    let event = ScraperEvent::EgressRotated {
        from: None,
        to: Some(egress("p1.example")),
    };
    assert!(json(&event)["from"].is_null());
}

#[test]
fn the_fields_of_an_event_are_plain_values() {
    let events = every_event();
    let request = json(&events[0]);
    assert_eq!(request["status"], 200);
    assert_eq!(request["mode"], "Http");
    assert_eq!(request["elapsed_ms"], 31);
    let detected = json(&events[1]);
    assert_eq!(detected["vendor"], "Cloudflare");
    assert_eq!(detected["kind"], "JsChallenge");
    let changed = json(&events[4]);
    assert_eq!(changed["from"], "Http");
    assert_eq!(changed["to"], "Browser");
    assert_eq!(
        changed["reason"],
        serde_json::json!({"escalate": {"challenge": "JsChallenge"}})
    );
    assert_eq!(
        json(&events[9]),
        serde_json::json!({"event": "identity_exported", "cookies": 3})
    );
}

#[test]
fn a_mode_change_can_carry_a_demotion() {
    let event = ScraperEvent::ModeChanged {
        from: Mode::Browser,
        to: Mode::Http,
        reason: Transition::Demote(DemoteReason::ChallengeSolved),
    };
    assert_eq!(
        json(&event)["reason"],
        serde_json::json!({"demote": "challenge_solved"})
    );
}

#[test]
fn the_noop_sink_takes_everything_and_keeps_nothing() {
    let sink = NoopEventSink;
    for event in every_event() {
        sink.emit(&event);
    }
}

#[test]
fn the_collecting_sink_keeps_events_in_order() {
    let sink = CollectingEventSink::new();
    assert!(sink.is_empty());
    let events = every_event();
    for event in &events {
        sink.emit(event);
    }
    assert_eq!(sink.len(), events.len());
    assert_eq!(sink.events(), events);
    assert_eq!(sink.len(), events.len(), "reading does not drain");
}

#[test]
fn take_drains_the_sink() {
    let sink = CollectingEventSink::new();
    sink.emit(&ScraperEvent::IdentityExported { cookies: 1 });
    sink.emit(&ScraperEvent::IdentityRestored { cookies: 2 });
    assert_eq!(sink.take().len(), 2);
    assert!(sink.is_empty());
    assert_eq!(sink.take(), Vec::new());
}

#[test]
fn clones_of_a_collecting_sink_share_their_events() {
    let sink = CollectingEventSink::new();
    let twin = sink.clone();
    twin.emit(&ScraperEvent::IdentityExported { cookies: 5 });
    assert_eq!(
        sink.events(),
        vec![ScraperEvent::IdentityExported { cookies: 5 }]
    );
}

#[test]
fn the_collecting_sink_keeps_only_the_newest_events() {
    let sink = CollectingEventSink::new();
    let extra = 5;
    for cookies in 0..MAX_COLLECTED_EVENTS + extra {
        sink.emit(&ScraperEvent::IdentityExported { cookies });
    }
    assert_eq!(sink.len(), MAX_COLLECTED_EVENTS);
    let kept = sink.events();
    assert_eq!(
        kept.first(),
        Some(&ScraperEvent::IdentityExported { cookies: extra })
    );
    assert_eq!(
        kept.last(),
        Some(&ScraperEvent::IdentityExported {
            cookies: MAX_COLLECTED_EVENTS + extra - 1
        })
    );
}

#[test]
fn a_collecting_sink_prints_a_count_not_its_events() {
    let sink = CollectingEventSink::new();
    sink.emit(&ScraperEvent::ActionChosen {
        site: "private-site.example".to_owned(),
        action: Action::Proceed,
    });
    let text = format!("{sink:?}");
    assert!(text.contains("events: 1"), "{text}");
    assert!(!text.contains("private-site"), "{text}");
}

#[test]
fn sinks_are_shareable_trait_objects() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<NoopEventSink>();
    assert_send_sync::<CollectingEventSink>();
    assert_send_sync::<ScraperEvent>();
    let collecting = CollectingEventSink::new();
    let sinks: Vec<Arc<dyn EventSink>> =
        vec![Arc::new(NoopEventSink), Arc::new(collecting.clone())];
    for sink in &sinks {
        sink.emit(&ScraperEvent::IdentityRestored { cookies: 0 });
    }
    assert_eq!(collecting.len(), 1);
}
