//! The requests the browser is made to send, one connection each.
//!
//! An HTTP/2 connection carries many requests, and the header block of the second one is
//! compressed against the first. To read every request from a clean connection, each probe goes to
//! its own host name (`<probe>.capture.test`): a browser opens one connection per host, and the
//! certificate the capture site issues names only that host, so a connection is never shared.

use crate::scrape::runtime::capture::chrome::CAPTURE_DOMAIN;

/// What a probe asks the browser for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    /// The first label of the probe's host name, and the key of its evidence.
    pub name: &'static str,
    /// The `Sec-Fetch-Dest` value the browser is expected to send; checked against what arrived.
    pub dest: &'static str,
}

/// The navigation: the browser is pointed at the document probe's address.
pub const DOCUMENT: Probe = Probe {
    name: "document",
    dest: "document",
};
/// A script-issued request, made with `fetch` from the document page.
pub const FETCH: Probe = Probe {
    name: "fetch",
    dest: "empty",
};
/// An image the page loads.
pub const IMAGE: Probe = Probe {
    name: "image",
    dest: "image",
};
/// A script the page loads.
pub const SCRIPT: Probe = Probe {
    name: "script",
    dest: "script",
};
/// A stylesheet the page loads.
pub const STYLE: Probe = Probe {
    name: "style",
    dest: "style",
};

/// Every probe, the navigation first.
pub const PROBES: [Probe; 5] = [DOCUMENT, FETCH, IMAGE, SCRIPT, STYLE];

/// The HTML the navigation is answered with, so the page has a document to run the probes in.
pub const DOCUMENT_BODY: &[u8] = b"<!doctype html><meta charset=\"utf-8\"><title>capture</title>";

impl Probe {
    /// The probe's host name, such as `fetch.capture.test`.
    #[must_use]
    pub fn host(&self) -> String {
        format!("{}.{CAPTURE_DOMAIN}", self.name)
    }

    /// The probe whose host name is `host`, compared without regard to ASCII case.
    #[must_use]
    pub fn for_host(host: &str) -> Option<Probe> {
        PROBES
            .into_iter()
            .find(|probe| probe.host().eq_ignore_ascii_case(host))
    }

    /// The address the probe is reached at on `port`.
    #[must_use]
    pub fn url(&self, port: u16) -> String {
        format!("https://{}:{port}/", self.host())
    }
}

/// The script, run in the document page, that makes the browser send the other probes.
///
/// The requests are plain: no cache options and no custom headers, so the headers the browser
/// sends are the ones it sends by default. The fetch is cross-origin, which is allowed to fail
/// once its response has no CORS headers; the request has already been sent by then.
#[must_use]
pub fn page_script(port: u16) -> String {
    let fetch = FETCH.url(port);
    let image = format!("{}probe.png", IMAGE.url(port));
    let script = format!("{}probe.js", SCRIPT.url(port));
    let style = format!("{}probe.css", STYLE.url(port));
    format!(
        "(() => {{\n\
         \x20 fetch(\"{fetch}\").catch(() => {{}});\n\
         \x20 const image = new Image();\n\
         \x20 image.src = \"{image}\";\n\
         \x20 const script = document.createElement(\"script\");\n\
         \x20 script.src = \"{script}\";\n\
         \x20 document.head.appendChild(script);\n\
         \x20 const style = document.createElement(\"link\");\n\
         \x20 style.rel = \"stylesheet\";\n\
         \x20 style.href = \"{style}\";\n\
         \x20 document.head.appendChild(style);\n\
         }})();"
    )
}
