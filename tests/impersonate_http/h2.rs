//! An origin that speaks HTTP/2: the headers of that protocol, and the reply saying so.

use std::sync::Arc;

use seleniumbase_rs::impersonate::client::WireProtocol;
use seleniumbase_rs::impersonate::http_session::{HttpSession, ReplyProtocol};
use seleniumbase_rs::scrape::ledger::events::CollectingEventSink;
use seleniumbase_rs::wire::header_spec::RequestShape;
use url::Url;

use crate::site::{identity, now, H2Site};

const HOST: &str = "h2.test";
const BODY: &str = "over http2";

#[tokio::test]
async fn an_http2_origin_gets_the_http2_headers_and_the_reply_says_so() {
    let site = H2Site::start(BODY).await;
    let mut base = identity();
    let url = Url::parse(&site.url(HOST, "/")).expect("a url");
    base.cookies.store(&url, "carried=1; Max-Age=3600", now());
    let sink = Arc::new(CollectingEventSink::new());
    let options = site.options(&[HOST]).event_sink(sink);
    let session = HttpSession::from_identity(&base, None, options).expect("a session");

    let reply = session
        .get(&site.url(HOST, "/page"))
        .await
        .expect("a reply");

    assert_eq!(reply.protocol(), ReplyProtocol::Http2);
    assert_eq!(reply.text().expect("text"), BODY);
    let prefaces = site.prefaces();
    assert_eq!(prefaces.len(), 1);
    let sent = |name: &str| {
        prefaces[0]
            .headers
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.value.as_str())
    };
    let profile = session.profile();
    assert_eq!(sent("user-agent"), Some(profile.user_agent()));
    assert_eq!(sent("cookie"), Some("carried=1"));
    assert_eq!(sent(":path"), Some("/page"));
    let expected = profile
        .headers_for(
            RequestShape::Document,
            Some("carried=1"),
            WireProtocol::Http2,
        )
        .expect("the headers");
    let (_, priority) = expected
        .iter()
        .find(|(name, _)| name == "priority")
        .expect("HTTP/2 carries the priority header of the table");
    assert_eq!(sent("priority"), Some(priority.as_str()));
    assert_eq!(sent("host"), None, "HTTP/2 names the host as :authority");
    assert!(sent(":authority").is_some_and(|authority| authority.starts_with(HOST)));
}
