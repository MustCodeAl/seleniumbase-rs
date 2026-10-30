---
title: "codec::h2_spec"
sidebarTitle: "H2Spec"
description: "The HTTP/2 profile in seleniumbase-rs-codec: H2Spec (alias Http2Spec), its constructors from a capture, a preface or an Akamai string, the check against what the wreq HTTP/2 stack can send, setting_id and SpecError."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_stealth::codec::h2_spec` describes the start of an HTTP/2 connection that
a client should reproduce: the SETTINGS entries and their order, the connection
`WINDOW_UPDATE`, the PRIORITY frames, the priority fields of the first HEADERS
frame, and the order of the pseudo-headers. An `H2Spec` is built from bytes
captured from a real client (`from_capture`, `from_preface`), from an Akamai
fingerprint string (`from_akamai`), or by overlaying such a string on a measured
base (`with_akamai`).

Recording a capture and judging it are separate steps. `from_preface` records
whatever the client sent, GREASE settings included. `check_reproducible` then
says whether the wreq HTTP/2 stack can send it, before any client exists. What the
stack cannot emit, as of `wreq-proto` 0.2.6:

- SETTINGS other than ids 1 to 6, 8 and 9 (experimental and GREASE settings);
- a SETTINGS id twice (the stack's order builder drops the second silently);
- leaving out `SETTINGS_INITIAL_WINDOW_SIZE`, which the stack always sends;
- two PRIORITY frames for one stream, or one for stream 0 (both are dropped silently).

<RequestExample>

```rust From an Akamai string
use seleniumbase_rs_stealth::codec::akamai::Akamai;
use seleniumbase_rs_stealth::codec::h2_spec::{H2Spec, SpecError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let akamai: Akamai = "1:4096;2:0;4:1048576|983041|0|m,a,s,p".parse()?;
    let spec = H2Spec::from_akamai(&akamai)?;
    assert_eq!(spec.akamai(), akamai);

    let grease: Akamai = "2570:1;4:1048576|0|0|m,a,s,p".parse()?;
    assert_eq!(H2Spec::from_akamai(&grease), Err(SpecError::UnsupportedSetting(2570)));
    Ok(())
}
```

```rust The measured HTTP/2 start
use seleniumbase_rs_stealth::evasion::fingerprint::OsType;
use seleniumbase_rs_stealth::codec::emulation;
use seleniumbase_rs_stealth::codec::BrowserKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chrome = emulation::get(BrowserKind::Chrome, OsType::Macos)?;
    let http2 = chrome.http2.as_ref().ok_or("no HTTP/2 capture")?;
    println!("{}", http2.akamai());
    println!("{:?}", http2.headers_priority);
    Ok(())
}
```

</RequestExample>

<ResponseExample>

```text The measured HTTP/2 start
1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p
Some(HeadersPriority { depends_on: 0, weight: 256, exclusive: true })
```

</ResponseExample>

## `H2Spec`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct H2Spec {
    pub settings: Vec<(u16, u32)>,
    pub window_update: u32,
    #[serde(default)]
    pub priority_frames: Vec<PriorityFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers_priority: Option<HeadersPriority>,
    pub pseudo_order: [Pseudo; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_stream_id: Option<u32>,
}

pub type Http2Spec = H2Spec;
```

A whole HTTP/2 profile: what a client sends before and in its first request.
`Http2Spec` is the same type under the name the design plan uses; it is the type
of `Emulation::http2`. Re-exported at the crate root as `H2Spec`.

<ResponseField name="settings" type="Vec<(u16, u32)>">

SETTINGS entries `(id, value)` in wire order.

</ResponseField>

<ResponseField name="window_update" type="u32">

The connection-level `WINDOW_UPDATE` increment; `0` means no frame.

</ResponseField>

<ResponseField name="priority_frames" type="Vec<PriorityFrame>" default="[]">

PRIORITY frames sent before the first request, in order.

</ResponseField>

<ResponseField name="headers_priority" type="Option<HeadersPriority>">

The priority fields of the first HEADERS frame, when the client sends them.

</ResponseField>

<ResponseField name="pseudo_order" type="[Pseudo; 4]">

The order of the request pseudo-headers.

</ResponseField>

<ResponseField name="initial_stream_id" type="Option<u32>">

The stream id of the first request, when it is not the usual `1`.

</ResponseField>

### Constructors

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn from_preface(preface: &H2Preface) -> Result<Self, SpecError>
pub fn from_capture(bytes: &[u8]) -> Result<Self, SpecError>
pub fn from_akamai(akamai: &Akamai) -> Result<Self, SpecError>
pub fn with_akamai(&self, akamai: &Akamai) -> Result<Self, SpecError>
```

<ResponseField name="from_preface" type="method">

Records a captured connection start; nothing is judged. `PseudoOrder` when the
request does not carry the four pseudo-headers exactly once and before its other
headers.

</ResponseField>

<ResponseField name="from_capture" type="method">

Reads a captured connection start from raw client bytes. `Incomplete` when the
bytes end before the first request is complete, `Capture` when they are not a
valid client start.

</ResponseField>

<ResponseField name="from_akamai" type="method">

A spec from an Akamai fingerprint alone, checked to be sendable. The string says
nothing about the priority fields of HEADERS, so none are set. The first request's
stream id is placed after the streams the PRIORITY frames name.

</ResponseField>

<ResponseField name="with_akamai" type="method">

Overlays an Akamai fingerprint on this spec. The string replaces the SETTINGS,
the connection window, the PRIORITY frames and the pseudo-header order, and the
first request's stream id is recomputed to come after the streams the PRIORITY
frames name. What the string does not carry, the HEADERS priority fields, is kept
from this spec. The result is checked.

</ResponseField>

### Other methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn check_reproducible(&self) -> Result<(), SpecError>
pub fn akamai(&self) -> Akamai
pub fn setting(&self, id: u16) -> Option<u32>
pub fn without_unsupported_settings(&self) -> (Self, Vec<(u16, u32)>)
```

- `check_reproducible`: checks that the wreq HTTP/2 stack can send exactly this
  spec, and returns the first problem found, naming the setting, frame or field.
  SETTINGS values are held to the ranges of RFC 9113 section 6.5.2, RFC 8441 and
  RFC 9218; a PRIORITY frame must be on an odd client stream below 2^31, not
  depend on itself, and have a weight of 1 to 256; the initial stream id must be
  odd and below 2^31.
- `akamai`: the Akamai fingerprint of this spec.
- `setting`: the value of the SETTINGS entry with this id, when there is one.
- `without_unsupported_settings`: this spec without the SETTINGS the stack cannot
  send, and the entries removed. For measured specs that include a GREASE
  setting, the dropped entries are the known gap between the capture and what a
  client can reproduce.

## `SpecError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
// Re-exported at the crate root as H2SpecError.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SpecError {
    UnsupportedSetting(u16),
    DuplicateSetting(u16),
    SettingValue { id: u16, value: u32 },
    SettingAlwaysSent(u16),
    WindowUpdate(u32),
    Priority { stream: u32, reason: &'static str },
    HeadersPriority { reason: &'static str },
    PseudoOrder,
    InitialStreamId(u32),
    Incomplete,
    Capture(H2CaptureError),
}
```

Why an HTTP/2 spec cannot be built or cannot be sent.

<ResponseField name="SpecError" type="enum">

<Expandable title="variants">

<ResponseField name="UnsupportedSetting(u16)">

A SETTINGS id the stack cannot send, such as a GREASE or experimental id.

</ResponseField>

<ResponseField name="DuplicateSetting(u16)">

A SETTINGS id appears twice.

</ResponseField>

<ResponseField name="SettingValue { id, value }">

A SETTINGS value outside the range RFC 9113 allows for its id.

</ResponseField>

<ResponseField name="SettingAlwaysSent(u16)">

A SETTINGS id the stack always sends is missing, so the spec could not be matched.

</ResponseField>

<ResponseField name="WindowUpdate(u32)">

A `WINDOW_UPDATE` increment that would push the window past 2^31 - 1.

</ResponseField>

<ResponseField name="Priority { stream, reason }">

A PRIORITY frame that is invalid or that the stack would drop, such as
`more than one PRIORITY frame for the stream`.

</ResponseField>

<ResponseField name="HeadersPriority { reason }">

Priority fields on HEADERS that are out of range.

</ResponseField>

<ResponseField name="PseudoOrder">

The pseudo-header order is not a permutation of the four request pseudo-headers.

</ResponseField>

<ResponseField name="InitialStreamId(u32)">

An initial stream id that is not an odd id below 2147483648.

</ResponseField>

<ResponseField name="Incomplete">

The captured bytes stop before the first request is complete.

</ResponseField>

<ResponseField name="Capture(H2CaptureError)">

The captured bytes are not a valid HTTP/2 client start.

</ResponseField>

</Expandable>

</ResponseField>

## `setting_id`

<Badge color="gray" shape="pill">mod</Badge>

```rust
pub mod setting_id {
    pub const HEADER_TABLE_SIZE: u16 = 1;
    pub const ENABLE_PUSH: u16 = 2;
    pub const MAX_CONCURRENT_STREAMS: u16 = 3;
    pub const INITIAL_WINDOW_SIZE: u16 = 4;
    pub const MAX_FRAME_SIZE: u16 = 5;
    pub const MAX_HEADER_LIST_SIZE: u16 = 6;
    pub const ENABLE_CONNECT_PROTOCOL: u16 = 8;
    pub const NO_RFC7540_PRIORITIES: u16 = 9;
}
```

The SETTINGS identifiers of RFC 9113 section 6.5.2, RFC 8441 and RFC 9218.

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const EMITTABLE_SETTINGS: [u16; 8] = [
    setting_id::HEADER_TABLE_SIZE,
    setting_id::ENABLE_PUSH,
    setting_id::MAX_CONCURRENT_STREAMS,
    setting_id::INITIAL_WINDOW_SIZE,
    setting_id::MAX_FRAME_SIZE,
    setting_id::MAX_HEADER_LIST_SIZE,
    setting_id::ENABLE_CONNECT_PROTOCOL,
    setting_id::NO_RFC7540_PRIORITIES,
];
pub const ALWAYS_SENT_SETTINGS: [u16; 1] = [setting_id::INITIAL_WINDOW_SIZE];
pub const INITIAL_CONNECTION_WINDOW: u32 = 65_535;
pub const MAX_WINDOW_UPDATE: u32 = MAX_STREAM_ID - INITIAL_CONNECTION_WINDOW;
pub const MIN_MAX_FRAME_SIZE: u32 = 16_384;
pub const MAX_MAX_FRAME_SIZE: u32 = 16_777_215;
pub const MAX_SPEC_PRIORITY_FRAMES: usize = MAX_PRIORITY_FRAMES;
```

The SETTINGS ids the wreq HTTP/2 stack can send; the ids it sends whatever it is
configured with (read from `wreq-proto` 0.2.6: `Http2Options::initial_window_size`
is always passed on, and the capability probe of the client package is the final
authority); the connection flow-control window before any `WINDOW_UPDATE`; the
largest connection increment (the window may not pass 2^31 - 1); the legal range
of `SETTINGS_MAX_FRAME_SIZE`; and the most PRIORITY frames a spec may carry (64).

## Related

<Columns cols={2}>

<Card color="#1ca1c7" title="Akamai" icon="code" href="/stealth/api/codec-akamai">

The fingerprint string the spec reads and writes.

</Card>

</Columns>
