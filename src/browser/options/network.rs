//! How the browser's network behaves: cut off, slowed down, or as it is.

use std::time::Duration;

use serde_json::{json, Value};

/// How the browser's network behaves: cut off, slowed down, or as it is.
///
/// Both engines take the same value:
/// [`BaseCase::set_network_throttle`](crate::BaseCase::set_network_throttle) and
/// [`BrowserSession::set_network_throttle`](crate::BrowserSession::set_network_throttle) on the
/// WebDriver engine, [`Emulation::network_throttle`](crate::sb_cdp::Emulation::network_throttle)
/// on the pure CDP engine. The default changes nothing, so it is also the value that puts the
/// connection back.
///
/// A throttle belongs to the tab it was set on, as the rest of the tab's emulation does: a
/// window the session switches to afterwards is not throttled until it is told so.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use seleniumbase_rs::NetworkThrottle;
///
/// // A slow connection: each response starts 400 ms late, and the page may receive
/// // 200 000 bytes a second and send 94 000.
/// let slow = NetworkThrottle {
///     latency: Duration::from_millis(400),
///     download: Some(200_000),
///     upload: Some(94_000),
///     ..NetworkThrottle::default()
/// };
/// assert!(!slow.offline);
///
/// // No network at all.
/// let cut_off = NetworkThrottle { offline: true, ..NetworkThrottle::default() };
/// assert!(cut_off.offline);
///
/// // The connection as it is: the default throttles nothing.
/// assert_eq!(NetworkThrottle::default().download, None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NetworkThrottle {
    /// No network at all: every request fails at once.
    pub offline: bool,
    /// Added before each response starts to arrive. The browser takes whole milliseconds, so
    /// a finer part is dropped.
    pub latency: Duration,
    /// The most bytes per second the page may receive; `None` for no limit.
    pub download: Option<u32>,
    /// The most bytes per second the page may send; `None` for no limit.
    pub upload: Option<u32>,
}

impl NetworkThrottle {
    /// The parameters of `Network.emulateNetworkConditions` for this throttle: the latency in
    /// milliseconds, the limits in bytes per second, and `-1` (the protocol's "no limit") for
    /// a limit that is `None`.
    pub(crate) fn protocol_params(&self) -> Value {
        json!({
            "offline": self.offline,
            "latency": u64::try_from(self.latency.as_millis()).unwrap_or(u64::MAX),
            "downloadThroughput": limit(self.download),
            "uploadThroughput": limit(self.upload),
        })
    }

    /// The DevTools commands that apply this throttle to one tab, in the order they are sent:
    /// `Network.enable`, then `Network.emulateNetworkConditions`.
    pub(crate) fn commands(&self) -> [(&'static str, Value); 2] {
        [
            ("Network.enable", json!({})),
            ("Network.emulateNetworkConditions", self.protocol_params()),
        ]
    }
}

/// What the protocol takes for a limit in bytes per second: the limit, or `-1` for none.
fn limit(bytes_per_second: Option<u32>) -> i64 {
    bytes_per_second.map_or(-1, i64::from)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::NetworkThrottle;

    #[test]
    fn the_default_sends_what_going_back_online_has_always_sent() {
        // `Emulation::offline(false)` sent exactly this before the type existed.
        assert_eq!(
            NetworkThrottle::default().protocol_params(),
            json!({
                "offline": false,
                "latency": 0,
                "downloadThroughput": -1,
                "uploadThroughput": -1,
            })
        );
    }

    #[test]
    fn being_offline_alone_limits_nothing_else() {
        let throttle = NetworkThrottle {
            offline: true,
            ..NetworkThrottle::default()
        };
        assert_eq!(
            throttle.protocol_params(),
            json!({
                "offline": true,
                "latency": 0,
                "downloadThroughput": -1,
                "uploadThroughput": -1,
            })
        );
    }

    #[test]
    fn a_throttle_is_sent_as_milliseconds_and_bytes_per_second() {
        let throttle = NetworkThrottle {
            offline: false,
            latency: Duration::from_millis(400),
            download: Some(200_000),
            upload: Some(94_000),
        };
        assert_eq!(
            throttle.protocol_params(),
            json!({
                "offline": false,
                "latency": 400,
                "downloadThroughput": 200_000,
                "uploadThroughput": 94_000,
            })
        );
    }

    #[test]
    fn a_limit_that_is_none_is_minus_one_and_the_other_stays() {
        let download_only = NetworkThrottle {
            download: Some(1_000),
            ..NetworkThrottle::default()
        };
        let params = download_only.protocol_params();
        assert_eq!(params["downloadThroughput"], json!(1_000));
        assert_eq!(params["uploadThroughput"], json!(-1));

        let upload_only = NetworkThrottle {
            upload: Some(2_000),
            ..NetworkThrottle::default()
        };
        let params = upload_only.protocol_params();
        assert_eq!(params["downloadThroughput"], json!(-1));
        assert_eq!(params["uploadThroughput"], json!(2_000));
    }

    #[test]
    fn the_largest_limit_stays_positive() {
        let throttle = NetworkThrottle {
            download: Some(u32::MAX),
            upload: Some(u32::MAX),
            ..NetworkThrottle::default()
        };
        let params = throttle.protocol_params();
        assert_eq!(params["downloadThroughput"], json!(u32::MAX));
        assert_eq!(params["uploadThroughput"], json!(u32::MAX));
    }

    #[test]
    fn a_latency_is_whole_milliseconds_and_never_overflows() {
        let finer = NetworkThrottle {
            latency: Duration::from_micros(1_999),
            ..NetworkThrottle::default()
        };
        assert_eq!(finer.protocol_params()["latency"], json!(1));

        let endless = NetworkThrottle {
            latency: Duration::MAX,
            ..NetworkThrottle::default()
        };
        assert_eq!(endless.protocol_params()["latency"], json!(u64::MAX));
    }

    #[test]
    fn the_commands_enable_the_domain_and_then_emulate() {
        let throttle = NetworkThrottle {
            latency: Duration::from_millis(50),
            ..NetworkThrottle::default()
        };
        assert_eq!(
            throttle.commands(),
            [
                ("Network.enable", json!({})),
                (
                    "Network.emulateNetworkConditions",
                    throttle.protocol_params()
                ),
            ]
        );
    }
}
