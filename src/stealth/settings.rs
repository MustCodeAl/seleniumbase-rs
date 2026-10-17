//! The values the settings of a [`Fingerprint`](super::fingerprint::Fingerprint)
//! can hold.
//!
//! Most settings are a plain `String`, `u32` or `f64`. A few belong together
//! (a screen is a width and a height, a position is a latitude and a longitude)
//! and are small types of their own. Every one of them converts from the
//! obvious tuple, so a builder call reads like the thing it sets:
//!
//! ```
//! use seleniumbase_rs::stealth::fingerprint::{Fingerprint, Spoof};
//!
//! let fp = Fingerprint::builder()
//!     .screen(1920, 1080)
//!     .geolocation((52.52, 13.405))
//!     .webgl(("Google Inc. (NVIDIA)", "ANGLE (NVIDIA, GeForce RTX 4070)"))
//!     .webrtc(Spoof::Off)
//!     .build()?;
//! # Ok::<(), seleniumbase_rs::stealth::fingerprint::FingerprintError>(())
//! ```

use serde::{Deserialize, Serialize};

use super::fingerprint::MediaDeviceSpec;
use super::spoof::Spoof;

/// A screen size in CSS pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenSize {
    /// The width, in CSS pixels.
    pub width: u32,
    /// The height, in CSS pixels.
    pub height: u32,
}

impl ScreenSize {
    /// A screen of `width` by `height` CSS pixels.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl From<(u32, u32)> for ScreenSize {
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width, height)
    }
}

impl From<(u32, u32)> for Spoof<ScreenSize> {
    fn from(size: (u32, u32)) -> Self {
        Self::Value(size.into())
    }
}

/// The position `navigator.geolocation` reports.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Geolocation {
    /// Degrees north.
    pub latitude: f64,
    /// Degrees east.
    pub longitude: f64,
    /// Metres above sea level; `0` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
    /// How far off the position may be, in metres; `100` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
}

impl Geolocation {
    /// A position at `latitude`, `longitude`.
    #[must_use]
    pub const fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
            altitude: None,
            accuracy: None,
        }
    }

    /// The same position with an altitude, in metres.
    #[must_use]
    pub const fn altitude(mut self, metres: f64) -> Self {
        self.altitude = Some(metres);
        self
    }

    /// The same position with an accuracy, in metres.
    #[must_use]
    pub const fn accuracy(mut self, metres: f64) -> Self {
        self.accuracy = Some(metres);
        self
    }
}

impl From<(f64, f64)> for Geolocation {
    fn from((latitude, longitude): (f64, f64)) -> Self {
        Self::new(latitude, longitude)
    }
}

impl From<(f64, f64)> for Spoof<Geolocation> {
    fn from(position: (f64, f64)) -> Self {
        Self::Value(position.into())
    }
}

/// The WebGL strings `getParameter` reports for the unmasked vendor and
/// renderer.
///
/// A half that is `None` is the browser's own.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WebGl {
    /// `UNMASKED_VENDOR_WEBGL`, such as `Google Inc. (NVIDIA)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// `UNMASKED_RENDERER_WEBGL`, such as `ANGLE (NVIDIA, ...)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer: Option<String>,
    /// The PCI vendor id of the GPU (`0x10de`). It is kept for tools that read
    /// it and never shown to a page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<String>,
    /// The PCI device id of the GPU (`0x1f91`). Kept, never shown to a page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer_id: Option<String>,
}

impl WebGl {
    /// A vendor and a renderer.
    #[must_use]
    pub fn new(vendor: impl Into<String>, renderer: impl Into<String>) -> Self {
        Self {
            vendor: Some(vendor.into()),
            renderer: Some(renderer.into()),
            ..Self::default()
        }
    }

    /// A renderer alone: the vendor stays the browser's own.
    #[must_use]
    pub fn renderer_only(renderer: impl Into<String>) -> Self {
        Self {
            renderer: Some(renderer.into()),
            ..Self::default()
        }
    }

    /// A vendor alone: the renderer stays the browser's own.
    #[must_use]
    pub fn vendor_only(vendor: impl Into<String>) -> Self {
        Self {
            vendor: Some(vendor.into()),
            ..Self::default()
        }
    }

    /// The same strings with the PCI ids of the GPU.
    #[must_use]
    pub fn ids(mut self, vendor_id: impl Into<String>, renderer_id: impl Into<String>) -> Self {
        self.vendor_id = Some(vendor_id.into());
        self.renderer_id = Some(renderer_id.into());
        self
    }

    /// Whether neither string is given, so the setting would change nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.vendor.is_none() && self.renderer.is_none()
    }
}

impl<V: Into<String>, R: Into<String>> From<(V, R)> for WebGl {
    fn from((vendor, renderer): (V, R)) -> Self {
        Self::new(vendor, renderer)
    }
}

impl<V: Into<String>, R: Into<String>> From<(V, R)> for Spoof<WebGl> {
    fn from(strings: (V, R)) -> Self {
        Self::Value(strings.into())
    }
}

/// The addresses WebRTC reports, and with them the request that its STUN and
/// TURN servers are filtered.
///
/// A fingerprint whose WebRTC is a `Value` filters the servers a page names, so
/// no peer connection can learn the real address. The two addresses are
/// optional on top of that: one that is given replaces the address the
/// browser would write into the SDP.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WebRtcAddresses {
    /// The public address of the candidates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_ip: Option<String>,
    /// The local address of the connection line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_ip: Option<String>,
}

impl WebRtcAddresses {
    /// A public and a local address.
    #[must_use]
    pub fn new(public_ip: impl Into<String>, local_ip: impl Into<String>) -> Self {
        Self {
            public_ip: Some(public_ip.into()),
            local_ip: Some(local_ip.into()),
        }
    }
}

impl<P: Into<String>, L: Into<String>> From<(P, L)> for WebRtcAddresses {
    fn from((public_ip, local_ip): (P, L)) -> Self {
        Self::new(public_ip, local_ip)
    }
}

impl<P: Into<String>, L: Into<String>> From<(P, L)> for Spoof<WebRtcAddresses> {
    fn from(addresses: (P, L)) -> Self {
        Self::Value(addresses.into())
    }
}

/// What `navigator.mediaDevices.enumerateDevices()` lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaDevices {
    /// How many microphones are listed.
    #[serde(default)]
    pub audio_inputs: u32,
    /// How many speakers are listed.
    #[serde(default)]
    pub audio_outputs: u32,
    /// How many cameras are listed.
    #[serde(default)]
    pub video_inputs: u32,
    /// Devices with their own ids and labels. When this is not empty, it is the
    /// whole list and the counts are not used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labelled: Vec<MediaDeviceSpec>,
}

impl MediaDevices {
    /// This many microphones, speakers and cameras, with generated labels.
    #[must_use]
    pub fn counts(audio_inputs: u32, audio_outputs: u32, video_inputs: u32) -> Self {
        Self {
            audio_inputs,
            audio_outputs,
            video_inputs,
            labelled: Vec::new(),
        }
    }

    /// Exactly these devices.
    #[must_use]
    pub fn labelled(devices: Vec<MediaDeviceSpec>) -> Self {
        Self {
            labelled: devices,
            ..Self::default()
        }
    }
}

impl From<(u32, u32, u32)> for MediaDevices {
    fn from((audio_inputs, audio_outputs, video_inputs): (u32, u32, u32)) -> Self {
        Self::counts(audio_inputs, audio_outputs, video_inputs)
    }
}

impl From<(u32, u32, u32)> for Spoof<MediaDevices> {
    fn from(counts: (u32, u32, u32)) -> Self {
        Self::Value(counts.into())
    }
}

impl From<Vec<MediaDeviceSpec>> for Spoof<MediaDevices> {
    fn from(devices: Vec<MediaDeviceSpec>) -> Self {
        Self::Value(MediaDevices::labelled(devices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tuple_is_the_value_of_the_setting_it_names() {
        let screen: Spoof<ScreenSize> = (1920, 1080).into();
        let place: Spoof<Geolocation> = (52.52, 13.405).into();
        let gl: Spoof<WebGl> = ("Vendor", "Renderer").into();
        let rtc: Spoof<WebRtcAddresses> = ("203.0.113.9", "10.0.0.2").into();
        let devices: Spoof<MediaDevices> = (1, 2, 3).into();

        assert_eq!(screen, Spoof::Value(ScreenSize::new(1920, 1080)));
        assert_eq!(place.value().map(|p| p.latitude), Some(52.52));
        assert_eq!(
            gl.value().and_then(|g| g.renderer.as_deref()),
            Some("Renderer")
        );
        assert_eq!(
            rtc.value().and_then(|a| a.public_ip.as_deref()),
            Some("203.0.113.9")
        );
        assert_eq!(devices.value().map(|d| d.video_inputs), Some(3));
    }

    #[test]
    fn a_position_carries_its_optional_altitude_and_accuracy() {
        let place = Geolocation::new(1.0, 2.0).altitude(30.0).accuracy(15.0);

        assert_eq!((place.altitude, place.accuracy), (Some(30.0), Some(15.0)));
        assert_eq!(Geolocation::new(1.0, 2.0).accuracy, None);
    }

    #[test]
    fn a_webgl_half_is_the_browsers_own_until_it_is_given() {
        assert!(WebGl::default().is_empty());
        assert_eq!(WebGl::renderer_only("R").vendor, None);
        assert_eq!(WebGl::vendor_only("V").renderer, None);
        assert!(!WebGl::new("V", "R").ids("0x10de", "0x1f91").is_empty());
    }

    #[test]
    fn json_leaves_out_the_parts_that_are_absent() {
        let text = serde_json::to_string(&Spoof::Value(Geolocation::new(1.0, 2.0))).unwrap();
        let gl = serde_json::to_string(&Spoof::Value(WebGl::renderer_only("R"))).unwrap();

        assert_eq!(text, r#"{"latitude":1.0,"longitude":2.0}"#);
        assert_eq!(gl, r#"{"renderer":"R"}"#);
    }

    #[test]
    fn json_reads_a_keyword_or_the_value_for_a_compound_setting() {
        let off: Spoof<Geolocation> = serde_json::from_str(r#""off""#).unwrap();
        let place: Spoof<Geolocation> =
            serde_json::from_str(r#"{"latitude":1.0,"longitude":2.0}"#).unwrap();
        let size: Spoof<ScreenSize> =
            serde_json::from_str(r#"{"width":800,"height":600}"#).unwrap();

        assert_eq!(off, Spoof::Off);
        assert_eq!(place, Spoof::Value(Geolocation::new(1.0, 2.0)));
        assert_eq!(size, Spoof::Value(ScreenSize::new(800, 600)));
    }

    #[test]
    fn labelled_devices_replace_the_counts() {
        let spec = MediaDeviceSpec {
            kind: "audioinput".to_owned(),
            device_id: "a".to_owned(),
            group_id: "g".to_owned(),
            label: "Mic".to_owned(),
        };
        let devices = MediaDevices::labelled(vec![spec]);

        assert_eq!(devices.labelled.len(), 1);
        assert_eq!(devices.audio_inputs, 0);
    }
}
