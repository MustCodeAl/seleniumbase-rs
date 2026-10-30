---
title: "codec::akamai"
sidebarTitle: "Akamai"
description: "Akamai HTTP/2 fingerprint strings in seleniumbase-rs-codec: Akamai, PriorityFrame, Pseudo, AkamaiError and the bounds of the parser."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_stealth::codec::akamai` parses and renders Akamai HTTP/2 fingerprint
strings, in the format of Akamai's public whitepaper on passive HTTP/2
fingerprinting: four sections separated by `|`.

```text
SETTINGS | WINDOW_UPDATE | PRIORITY | pseudo-header order
1:4096;2:0;4:1048576|983041|3:0:0:201,5:1:3:16|m,a,s,p
```

- SETTINGS is `id:value` pairs joined by `;`, in the order the frame carried them.
- `WINDOW_UPDATE` is the connection-level increment, or `0` when no frame was sent.
- PRIORITY is `stream:exclusive:depends_on:weight` entries joined by `,`, or `0`
  for none. The weight is the protocol weight, 1 to 256 (the wire byte plus one).
- The pseudo-header order is a permutation of `m,a,s,p` (`:method`,
  `:authority`, `:scheme`, `:path`).

Parsing is strict about what can exist on the wire: canonical decimal numbers
only, each SETTINGS id once, weights in 1 to 256, stream ids below 2^31. What a
given client stack can emit is a separate question, answered by
[`H2Spec`](/stealth/api/codec-h2-spec).

<RequestExample>

```rust Parse the measured Chrome string
use seleniumbase_rs_stealth::codec::akamai::{Akamai, Pseudo};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The Akamai string measured from Chrome 155 on macOS.
    let akamai: Akamai = "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p".parse()?;
    assert_eq!(akamai.settings, vec![(1, 65536), (2, 0), (4, 6_291_456), (6, 262_144)]);
    assert_eq!(akamai.window_update, 15_663_105);
    assert!(akamai.priority_frames.is_empty());
    assert_eq!(akamai.pseudo_order, Pseudo::ALL);
    println!("{akamai}");
    Ok(())
}
```

```rust A refusal names the entry
use seleniumbase_rs_stealth::codec::akamai::{Akamai, AkamaiError};

let bad = Akamai::parse("1:1;2:0;1:2|0|0|m,a,s,p");
assert_eq!(bad, Err(AkamaiError::DuplicateSetting(1)));
```

</RequestExample>

<ResponseExample>

```text Parse the measured Chrome string
1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p
```

</ResponseExample>

## `Akamai`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Akamai {
    pub settings: Vec<(u16, u32)>,
    pub window_update: u32,
    pub priority_frames: Vec<PriorityFrame>,
    pub pseudo_order: [Pseudo; 4],
}
```

A parsed Akamai HTTP/2 fingerprint. The fields are public so a caller can build
one directly; `Akamai::parse` is the validating constructor, and
`H2Spec::with_akamai` checks again whatever it is given. `Display` writes the
canonical string and `FromStr` parses, so a canonical string round-trips.

<ResponseField name="settings" type="Vec<(u16, u32)>">

SETTINGS entries `(id, value)` in the order the frame carried them.

</ResponseField>

<ResponseField name="window_update" type="u32">

The connection-level `WINDOW_UPDATE` increment; `0` means no frame.

</ResponseField>

<ResponseField name="priority_frames" type="Vec<PriorityFrame>">

PRIORITY frames in the order they were sent.

</ResponseField>

<ResponseField name="pseudo_order" type="[Pseudo; 4]">

The order of the four request pseudo-headers.

</ResponseField>

### `Akamai::parse`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn parse(text: &str) -> Result<Self, AkamaiError>
```

Parses an Akamai fingerprint string. Leading and trailing whitespace is ignored;
nothing inside the string is. Returns an `AkamaiError` naming the section and
entry that is malformed.

## `PriorityFrame`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorityFrame {
    pub stream_id: u32,
    pub exclusive: bool,
    pub depends_on: u32,
    pub weight: u16,
}
```

One PRIORITY frame, in the Akamai notation `stream:exclusive:depends_on:weight`
(its `Display`, with `exclusive` written `0` or `1`).

<ResponseField name="stream_id" type="u32">

The stream the frame prioritises.

</ResponseField>

<ResponseField name="exclusive" type="bool">

Whether the dependency is exclusive.

</ResponseField>

<ResponseField name="depends_on" type="u32">

The stream this one depends on.

</ResponseField>

<ResponseField name="weight" type="u16">

The protocol weight, 1 to 256 (the wire byte plus one).

</ResponseField>

## `Pseudo`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pseudo {
    Method,
    Authority,
    Scheme,
    Path,
}

impl Pseudo {
    pub const ALL: [Pseudo; 4] = [Pseudo::Method, Pseudo::Authority, Pseudo::Scheme, Pseudo::Path];
    pub const fn letter(self) -> char;
    pub const fn header_name(self) -> &'static str;
    pub fn from_letter(letter: char) -> Option<Self>;
    pub fn from_header_name(name: &str) -> Option<Self>;
}
```

A request pseudo-header, as named in the pseudo-header order section. This is the
complete set of pseudo-headers of a plain request, so the enum is not
`non_exhaustive`: a pseudo-header order is a permutation of exactly these four.
In JSON it is written in lower case (`"method"`, `"authority"`, ...).

<ResponseField name="Pseudo" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Method">

`:method`, written `m`.

</ResponseField>

<ResponseField name="Authority">

`:authority`, written `a`.

</ResponseField>

<ResponseField name="Scheme">

`:scheme`, written `s`.

</ResponseField>

<ResponseField name="Path">

`:path`, written `p`.

</ResponseField>

</Expandable>

</ResponseField>

`ALL` is the order `m,a,s,p`. `letter` and `header_name` give the letter and the
header name (`:method`); `from_letter` accepts lowercase letters only, and
`from_header_name` the exact name.

## `AkamaiError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AkamaiError {
    TooLong,
    Sections(usize),
    Setting { index: usize, token: String },
    DuplicateSetting(u16),
    TooManySettings,
    WindowUpdate(String),
    PriorityFrame { index: usize, token: String },
    TooManyPriorityFrames,
    PseudoOrder(String),
}
```

Why an Akamai string was refused. `index` is the zero-based position of the
entry and `token` the offending text.

<ResponseField name="AkamaiError" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="TooLong">

The string is longer than `MAX_AKAMAI_LEN` bytes (after trimming).

</ResponseField>

<ResponseField name="Sections(usize)">

The string does not have exactly four `|`-separated sections.

</ResponseField>

<ResponseField name="Setting { index, token }">

A SETTINGS entry is not a canonical `id:value` pair with a 16-bit id and a 32-bit value.

</ResponseField>

<ResponseField name="DuplicateSetting(u16)">

A SETTINGS id appears more than once; it is refused, not dropped.

</ResponseField>

<ResponseField name="TooManySettings">

The SETTINGS section has more than `MAX_SETTINGS` entries.

</ResponseField>

<ResponseField name="WindowUpdate(String)">

The `WINDOW_UPDATE` section is not a number from 0 to 2147483647.

</ResponseField>

<ResponseField name="PriorityFrame { index, token }">

A PRIORITY entry is not `stream:exclusive:depends_on:weight` with a valid range.
An empty PRIORITY section is malformed; write `0` for none.

</ResponseField>

<ResponseField name="TooManyPriorityFrames">

The PRIORITY section has more than `MAX_PRIORITY_FRAMES` entries.

</ResponseField>

<ResponseField name="PseudoOrder(String)">

The pseudo-header order is not a permutation of `m,a,s,p`.

</ResponseField>

</Expandable>

</ResponseField>

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const MAX_STREAM_ID: u32 = 0x7fff_ffff;
pub const MIN_WEIGHT: u16 = 1;
pub const MAX_WEIGHT: u16 = 256;
pub const MAX_AKAMAI_LEN: usize = 4096;
pub const MAX_SETTINGS: usize = 32;
pub const MAX_PRIORITY_FRAMES: usize = 64;
```

The largest stream id, dependency id or window increment (2^31 - 1, the top bit
is reserved); the smallest and largest protocol weight (the wire byte is one
less); the longest string accepted, in bytes; and the most SETTINGS and PRIORITY
entries in one string.

## Related

<Columns cols={2}>

<Card color="#d5a910" title="H2Spec" icon="server" href="/stealth/api/codec-h2-spec">

`H2Spec::from_akamai` and what the wreq stack can send.

</Card>

<Card color="#d32a61" title="HTTP/2 frames" icon="layer-group" href="/stealth/api/codec-h2-frames">

`H2Preface::akamai` reads the string off captured bytes.

</Card>

</Columns>
