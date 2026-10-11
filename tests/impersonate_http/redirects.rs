//! Redirects: cookies per hop, the hop limit, and what must not follow a redirect to another site.

use seleniumbase_rs::impersonate::http_session::{HttpError, Method};

use crate::site::{Answer, Site};

const A: &str = "a.test";
const B: &str = "b.test";

#[tokio::test]
async fn each_hop_stores_its_cookies_and_sends_the_ones_that_apply() {
    let site = Site::start(|seen| match seen.path() {
        "/start" => Answer::redirect(302, "/next").cookie("a=1"),
        "/next" => Answer::redirect(302, "/end").cookie("b=2"),
        _ => Answer::ok("done"),
    })
    .await;
    let session = site.session(&[A]);

    let reply = session.get(&site.url(A, "/start")).await.expect("a reply");

    assert_eq!(reply.url().path(), "/end");
    assert_eq!(reply.text().expect("text"), "done");
    assert_eq!(reply.redirects().len(), 2);
    let seen = site.seen();
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0].cookie(), None);
    assert_eq!(seen[1].cookie(), Some("a=1"));
    assert_eq!(seen[2].cookie(), Some("a=1; b=2"));
}

#[tokio::test]
async fn a_redirect_with_a_large_body_is_followed_without_waiting_for_all_of_it() {
    let site = Site::start(|seen| match seen.path() {
        "/big" => Answer::redirect(302, "/end").body(&vec![b'x'; 200 * 1024]),
        _ => Answer::ok("done"),
    })
    .await;
    let session = site.session(&[A]);

    let reply = session.get(&site.url(A, "/big")).await.expect("a reply");

    assert_eq!(reply.text().expect("text"), "done");
    assert_eq!(site.seen().len(), 2);
}

#[tokio::test]
async fn a_redirect_loop_stops_at_the_hop_limit() {
    let site = Site::start(|_| Answer::redirect(302, "/loop")).await;
    let session = site.session(&[A]);

    let error = session
        .get(&site.url(A, "/loop"))
        .await
        .expect_err("a loop");

    assert!(matches!(error, HttpError::TooManyRedirects(10)), "{error}");
    assert_eq!(site.seen().len(), 11, "the first request and ten redirects");
}

#[tokio::test]
async fn the_hop_limit_is_the_one_in_the_options() {
    let site = Site::start(|_| Answer::redirect(302, "/loop")).await;
    let options = site.options(&[A]).max_redirects(3);
    let session = seleniumbase_rs::impersonate::http_session::HttpSession::from_identity(
        &crate::site::identity(),
        None,
        options,
    )
    .expect("a session");

    let error = session
        .get(&site.url(A, "/loop"))
        .await
        .expect_err("a loop");

    assert!(matches!(error, HttpError::TooManyRedirects(3)), "{error}");
    assert_eq!(site.seen().len(), 4);
}

#[tokio::test]
async fn a_redirect_to_another_site_never_carries_the_first_sites_cookies() {
    let site = Site::start(|seen| match (seen.host.as_str(), seen.path()) {
        (A, "/go") => Answer::redirect(302, &format!("https://{B}:{}/landing", seen.port))
            .cookie("sid=SECRET; Path=/"),
        _ => Answer::ok("landed"),
    })
    .await;
    let session = site.session(&[A, B]);

    let reply = session.get(&site.url(A, "/go")).await.expect("a reply");

    assert_eq!(reply.url().host_str(), Some(B));
    let seen = site.seen();
    assert_eq!(seen[1].host, B);
    assert_eq!(seen[1].cookie(), None, "a.test's cookie stays with a.test");
    session.get(&site.url(B, "/again")).await.expect("a reply");
    assert_eq!(site.seen()[2].cookie(), None);
    session.get(&site.url(A, "/back")).await.expect("a reply");
    assert_eq!(site.seen()[3].cookie(), Some("sid=SECRET"));
    assert_eq!(session.cookie_count(), 1);
}

#[tokio::test]
async fn authorization_and_other_headers_do_not_follow_a_redirect_to_another_origin() {
    let site = Site::start(|seen| match (seen.host.as_str(), seen.path()) {
        (A, "/same") => Answer::redirect(302, "/same-end"),
        (A, "/cross") => Answer::redirect(302, &format!("https://{B}:{}/end", seen.port)),
        _ => Answer::ok(""),
    })
    .await;
    let session = site.session(&[A, B]);
    let send = |path: &'static str| {
        session
            .request(Method::GET, &site.url(A, path))
            .header("authorization", "Bearer s3cret")
            .header("x-api-key", "k")
            .header("accept", "application/json")
            .send()
    };

    send("/same").await.expect("a reply");
    send("/cross").await.expect("a reply");

    let seen = site.seen();
    assert_eq!(
        seen[1].header("authorization"),
        Some("Bearer s3cret"),
        "the same origin keeps it"
    );
    assert_eq!(seen[1].header("x-api-key"), Some("k"));
    assert_eq!(seen[3].host, B);
    assert_eq!(
        seen[3].header("authorization"),
        None,
        "another origin never gets it"
    );
    assert_eq!(seen[3].header("x-api-key"), None);
    assert_eq!(
        seen[3].header("accept"),
        Some("application/json"),
        "accept may travel"
    );
}

#[tokio::test]
async fn a_303_turns_a_post_into_a_get_and_a_307_keeps_the_body() {
    let site = Site::start(|seen| match seen.path() {
        "/see-other" => Answer::redirect(303, "/get"),
        "/temporary" => Answer::redirect(307, "/post"),
        _ => Answer::ok(""),
    })
    .await;
    let session = site.session(&[A]);

    for path in ["/see-other", "/temporary"] {
        session
            .request(Method::POST, &site.url(A, path))
            .json(&serde_json::json!({ "k": 1 }))
            .send()
            .await
            .expect("a reply");
    }

    let seen = site.seen();
    assert_eq!((seen[1].method.as_str(), seen[1].body.len()), ("GET", 0));
    assert_eq!(seen[1].header("content-type"), None);
    assert_eq!(seen[3].method, "POST");
    assert_eq!(seen[3].body, br#"{"k":1}"#);
    assert_eq!(seen[3].header("content-type"), Some("application/json"));
}

#[tokio::test]
async fn a_redirect_to_a_file_or_with_credentials_is_refused() {
    let site = Site::start(|seen| match seen.path() {
        "/file" => Answer::redirect(302, "file:///etc/passwd"),
        "/creds" => Answer::redirect(302, "https://alice:s3cret@b.test/"),
        _ => Answer::ok(""),
    })
    .await;
    let session = site.session(&[A]);

    for path in ["/file", "/creds"] {
        let error = session.get(&site.url(A, path)).await.expect_err(path);
        assert!(
            matches!(error, HttpError::RedirectRefused { .. }),
            "{path}: {error}"
        );
        let shown = format!("{error} {error:?}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
    }
    assert_eq!(site.seen().len(), 2, "no request followed");
}

#[tokio::test]
async fn with_redirects_off_the_3xx_is_the_reply_and_its_cookies_are_kept() {
    let site = Site::start(|_| Answer::redirect(302, "/elsewhere").cookie("kept=1")).await;
    let options = site.options(&[A]).follow_redirects(false);
    let session = seleniumbase_rs::impersonate::http_session::HttpSession::from_identity(
        &crate::site::identity(),
        None,
        options,
    )
    .expect("a session");

    let reply = session.get(&site.url(A, "/")).await.expect("a reply");

    assert_eq!(reply.status(), 302);
    assert_eq!(reply.headers().get("location"), Some("/elsewhere"));
    assert_eq!(site.seen().len(), 1);
    assert_eq!(session.cookie_count(), 1);
}

#[tokio::test]
async fn a_3xx_without_a_location_is_the_reply() {
    let site = Site::start(|_| Answer::status(302)).await;
    let session = site.session(&[A]);

    let reply = session.get(&site.url(A, "/")).await.expect("a reply");

    assert_eq!(reply.status(), 302);
    assert_eq!(site.seen().len(), 1);
}

#[tokio::test]
async fn a_strict_cookie_stays_home_when_the_chain_crossed_sites() {
    let site = Site::start(|seen| match (seen.host.as_str(), seen.path()) {
        (B, "/set") => Answer::ok("")
            .cookie("strict=1; SameSite=Strict; Secure")
            .cookie("lax=1; SameSite=Lax; Secure"),
        (A, "/go") => Answer::redirect(302, &format!("https://{B}:{}/check", seen.port)),
        _ => Answer::ok(""),
    })
    .await;
    let session = site.session(&[A, B]);

    session.get(&site.url(B, "/set")).await.expect("a reply");
    session.get(&site.url(A, "/go")).await.expect("a reply");
    session.get(&site.url(B, "/check")).await.expect("a reply");

    let seen = site.seen();
    assert_eq!(seen[2].path(), "/check");
    assert_eq!(
        seen[2].cookie(),
        Some("lax=1"),
        "a cross-site chain sends the lax cookie only"
    );
    assert_eq!(
        seen[3].cookie(),
        Some("strict=1; lax=1"),
        "a direct request sends both"
    );
}
