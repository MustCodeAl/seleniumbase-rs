//! The identity headers of a profile: the user agent, the client hints and `Accept-Language`.

use crate::stealth::fingerprint::{BrandVersion, ClientHints};
use crate::wire::header_spec::HeaderIdentity;

/// The locale whose `Accept-Language` a measured profile starts with.
pub(super) const DEFAULT_LOCALE_TAG: &str = "en-US";
/// The token that carries the Chrome major version in a user agent.
const CHROME_TOKEN: &str = "Chrome/";
/// `Sec-CH-UA-Mobile` for a desktop browser.
const NOT_MOBILE: &str = "?0";
/// `Sec-CH-UA-Mobile` for a mobile browser.
const MOBILE: &str = "?1";

/// A string as a structured header field writes it: in quotes, with `\` and `"` escaped.
pub(super) fn quote(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        if character == '\\' || character == '"' {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted
}

/// The `Sec-CH-UA` value of a brand list: `"Brand";v="1", "Other";v="2"`.
pub(super) fn sec_ch_ua(brands: &[BrandVersion]) -> String {
    brands
        .iter()
        .map(|entry| format!("{};v={}", quote(&entry.brand), quote(&entry.version)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The placeholder values for a user agent, an `Accept-Language` and a set of client hints.
pub(super) fn identity_for(
    user_agent: &str,
    accept_language: Option<&str>,
    hints: Option<&ClientHints>,
) -> HeaderIdentity {
    let mut identity = HeaderIdentity::new(user_agent);
    if let Some(language) = accept_language {
        identity = identity.with_accept_language(language);
    }
    if let Some(hints) = hints {
        identity = with_hints(identity, hints);
    }
    identity
}

/// Sets the three client-hint headers a browser sends on every request.
pub(super) fn with_hints(identity: HeaderIdentity, hints: &ClientHints) -> HeaderIdentity {
    identity.with_client_hints(
        sec_ch_ua(&hints.brands),
        if hints.mobile { MOBILE } else { NOT_MOBILE },
        quote(&hints.platform),
    )
}

/// The Chrome major version a user agent claims.
pub(super) fn user_agent_major(user_agent: &str) -> Option<u32> {
    let (_, rest) = user_agent.split_once(CHROME_TOKEN)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}
