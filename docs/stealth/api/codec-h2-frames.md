---
title: "codec::h2_frames"
sidebarTitle: "HTTP/2 frames"
description: "HTTP/2 frame reading in seleniumbase-rs-codec: read_preface, PrefaceReader, H2Preface, Frame, read_frame, encode_frame, H2CaptureError, the HPACK decoder and the Huffman decoder."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_codec::h2_frames` reads the HTTP/2 connection preface, SETTINGS,
PRIORITY and the first HEADERS frame from captured bytes. A capture tool listens
for a client, collects the bytes it sends, and asks after each read. When the
first request's header block is complete, the result holds everything an Akamai
fingerprint needs: the SETTINGS in order, the connection `WINDOW_UPDATE`, the
PRIORITY frames, the HEADERS priority fields, and, from a full HPACK decode of the
block, the header order and the pseudo-header order.

The reader works on bytes already in memory and never waits: an incomplete frame
means `Ok(None)` ("read more"). Hostile input is bounded (frame size, frame count,
preface size, header block size), so that no input makes it allocate without
limit or panic. The HPACK decoder is in `h2_frames::hpack` and its Huffman decoder
in `h2_frames::huffman`.

<RequestExample>

```rust Read more until the request is complete
use seleniumbase_rs_codec::h2_frames::{
    encode_frame, frame_type, read_preface, H2CaptureError, CLIENT_PREFACE,
};

fn main() -> Result<(), H2CaptureError> {
    let mut bytes = CLIENT_PREFACE.to_vec();
    bytes.extend(encode_frame(frame_type::SETTINGS, 0, 0, &[0, 4, 0, 0, 0x10, 0])?);
    assert_eq!(read_preface(&bytes)?, None, "no HEADERS yet: read more");
    Ok(())
}
```

```rust A capture loop with PrefaceReader
use seleniumbase_rs_codec::h2_frames::{H2CaptureError, PrefaceReader};

/// Called with the whole buffer after every socket read.
fn on_read(reader: &mut PrefaceReader, buf: &[u8]) -> Result<(), H2CaptureError> {
    if let Some(preface) = reader.read(buf)? {
        println!("{}", preface.akamai()?);
        for field in preface.regular_headers() {
            println!("{}", field.name);
        }
    }
    Ok(())
}
```

```rust Decode a header block
use seleniumbase_rs_codec::h2_frames::hpack::{HpackDecoder, HpackError, DEFAULT_TABLE_SIZE};

fn names(block: &[u8]) -> Result<Vec<String>, HpackError> {
    let mut decoder = HpackDecoder::new(DEFAULT_TABLE_SIZE);
    Ok(decoder.decode(block)?.into_iter().map(|field| field.name).collect())
}
```

</RequestExample>

## `read_preface`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn read_preface(bytes: &[u8]) -> Result<Option<H2Preface>, H2CaptureError>
```

Reads the client preface, its SETTINGS, `WINDOW_UPDATE` and PRIORITY frames, and
the first request's header block. Returns `Ok(None)` when `bytes` ends before the
first request is complete; call it again with the longer buffer. Each call reads
from the start, which the `MAX_PREFACE_BYTES` and `MAX_PREFACE_FRAMES` limits keep
bounded; a loop that reads a socket and calls it after every read should use a
`PrefaceReader`, which parses each frame once.

<ResponseField name="returns" type="Result<Option<H2Preface>, H2CaptureError>">

An `H2CaptureError` when the bytes are not an HTTP/2 client start, a frame is
malformed or larger than `DEFAULT_MAX_FRAME_PAYLOAD`, the request does not arrive
within the limits, or its header block does not decode.

</ResponseField>

## `PrefaceReader`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Default)]
pub struct PrefaceReader { /* private fields */ }

impl PrefaceReader {
    pub fn new() -> Self;
    pub fn frames_read(&self) -> usize;
    pub fn bytes_read(&self) -> usize;
    pub fn read(&mut self, bytes: &[u8]) -> Result<Option<H2Preface>, H2CaptureError>;
}
```

Reads a client's connection start from a buffer that grows between calls, parsing
every frame once. `read_preface` reads from the start on every call, so a peer
that sends one byte at a time makes a loop that calls it after each read do work
quadratic in the bytes received. A `PrefaceReader` remembers the frames it has
parsed and the offset it stopped at, so each call costs the new bytes only. The
buffer of a later call must start with the bytes of the earlier ones; a shorter
buffer only makes the call report that it needs more.

After it returns a request or an error, the reader is spent: a later call returns
`H2CaptureError::ReaderFinished`. `frames_read` counts each frame once, however
many calls it took to arrive; `bytes_read` is the magic and the frames parsed.
`Debug` shows the counters only, because the bytes read may hold cookies.

## `H2Preface`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct H2Preface {
    pub settings: Vec<(u16, u32)>,
    pub window_update: u32,
    pub priority_frames: Vec<PriorityFrame>,
    pub headers_priority: Option<HeadersPriority>,
    pub request_stream_id: u32,
    pub headers: Vec<HeaderField>,
    pub consumed: usize,
}
```

The decoded start of a client connection, up to and including its first request.

<ResponseField name="settings" type="Vec<(u16, u32)>">

The entries of the first SETTINGS frame, in wire order.

</ResponseField>

<ResponseField name="window_update" type="u32">

The first connection-level `WINDOW_UPDATE` increment, or `0` when there was none.

</ResponseField>

<ResponseField name="priority_frames" type="Vec<PriorityFrame>">

The PRIORITY frames before the request, in order.

</ResponseField>

<ResponseField name="headers_priority" type="Option<HeadersPriority>">

The priority fields of the request's HEADERS frame, when it had them.

</ResponseField>

<ResponseField name="request_stream_id" type="u32">

The stream id of the first request.

</ResponseField>

<ResponseField name="headers" type="Vec<HeaderField>">

The request's headers in wire order, pseudo-headers included.

</ResponseField>

<ResponseField name="consumed" type="usize">

How many bytes of the input the preface used, the magic included.

</ResponseField>

### `H2Preface` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn pseudo_order(&self) -> Result<[Pseudo; 4], H2CaptureError>
pub fn akamai(&self) -> Result<Akamai, H2CaptureError>
pub fn regular_headers(&self) -> impl Iterator<Item = &HeaderField>
pub fn header(&self, name: &str) -> Option<&str>
```

- `pseudo_order`: the order of the four pseudo-headers. `PseudoOrder` unless the
  request has each of `:method`, `:authority`, `:scheme` and `:path` exactly
  once, all before the other headers.
- `akamai`: the [Akamai fingerprint](/stealth/api/codec-akamai) of this
  connection start.
- `regular_headers`: the request's headers without the pseudo-headers, in wire
  order.
- `header`: the value of the first header named `name`, compared without regard
  to ASCII case.

## `HeadersPriority`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadersPriority {
    pub depends_on: u32,
    pub weight: u16,
    pub exclusive: bool,
}
```

The priority fields of a HEADERS frame, as Akamai notation reads them: the stream
the new stream depends on, the protocol weight (1 to 256, the wire byte plus one),
and whether the dependency is exclusive.

## `Frame`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Frame {
    Settings { ack: bool, settings: Vec<(u16, u32)> },
    WindowUpdate { stream_id: u32, increment: u32 },
    Priority(PriorityFrame),
    Headers {
        stream_id: u32,
        end_stream: bool,
        end_headers: bool,
        priority: Option<HeadersPriority>,
        fragment: Vec<u8>,
    },
    Continuation { stream_id: u32, end_headers: bool, fragment: Vec<u8> },
    Other { frame_type: u8, flags: u8, stream_id: u32, len: usize },
}

impl Frame {
    pub fn frame_type(&self) -> u8;
}
```

One frame, reduced to what a fingerprint needs. `frame_type()` is its type code.

<ResponseField name="Frame" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Settings { ack, settings }">

SETTINGS, with the entries in wire order (an acknowledgement has none).

</ResponseField>

<ResponseField name="WindowUpdate { stream_id, increment }">

`WINDOW_UPDATE`; `stream_id` is `0` for the connection, and the increment is at least 1.

</ResponseField>

<ResponseField name="Priority(PriorityFrame)">

PRIORITY.

</ResponseField>

<ResponseField name="Headers { stream_id, end_stream, end_headers, priority, fragment }">

HEADERS, with the padding and priority fields removed; `fragment` is the HPACK fragment.

</ResponseField>

<ResponseField name="Continuation { stream_id, end_headers, fragment }">

CONTINUATION, a further fragment of a header block.

</ResponseField>

<ResponseField name="Other { frame_type, flags, stream_id, len }">

Any other frame type: only its framing is read.

</ResponseField>

</Expandable>

</ResponseField>

## `read_frame`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn read_frame(
    bytes: &[u8],
    max_payload: usize,
) -> Result<Option<(Frame, usize)>, H2CaptureError>
```

Reads one frame from the front of `bytes`. Returns the frame and the number of
bytes it used, or `Ok(None)` when `bytes` ends before the frame does. A payload
length above `max_payload` is refused as soon as the nine header bytes are in,
without waiting for the payload. An error also comes back when the payload is
impossible for the frame's type.

## `encode_frame`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn encode_frame(
    frame_type: u8,
    flags: u8,
    stream_id: u32,
    payload: &[u8],
) -> Result<Vec<u8>, H2CaptureError>
```

Builds the bytes of one frame. `FrameTooLarge` when the payload does not fit the
24-bit length, `StreamIdRange` when the stream id does not fit 31 bits.

## `H2CaptureError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum H2CaptureError {
    BadPreface,
    FrameTooLarge { len: usize, max: usize },
    FrameSize { frame_type: u8, len: usize },
    StreamId { frame_type: u8, stream_id: u32 },
    BadPadding,
    ZeroWindowIncrement,
    UnexpectedFrame { frame_type: u8 },
    ExpectedContinuation { frame_type: u8 },
    HeaderBlockTooLarge,
    PrefaceTooLarge,
    TooManyFrames { max: usize },
    ReaderFinished,
    MissingSettings,
    PseudoOrder,
    StreamIdRange(u32),
    Hpack(HpackError),
}
```

Why captured HTTP/2 bytes were refused.

<ResponseField name="H2CaptureError" type="enum">

<Expandable title="variants">

<ResponseField name="BadPreface">

The bytes do not start with the HTTP/2 client connection preface.

</ResponseField>

<ResponseField name="FrameTooLarge { len, max }">

A frame declares a payload larger than the limit in force.

</ResponseField>

<ResponseField name="FrameSize { frame_type, len }">

A frame's payload length is impossible for its type.

</ResponseField>

<ResponseField name="StreamId { frame_type, stream_id }">

A frame is on a stream its type forbids, or off the stream it requires.

</ResponseField>

<ResponseField name="BadPadding">

The pad length of a padded frame is larger than the frame's data.

</ResponseField>

<ResponseField name="ZeroWindowIncrement">

A `WINDOW_UPDATE` carries an increment of zero.

</ResponseField>

<ResponseField name="UnexpectedFrame { frame_type }">

A frame that cannot come before the first request arrived.

</ResponseField>

<ResponseField name="ExpectedContinuation { frame_type }">

A header block was interrupted by something other than its CONTINUATION frames.

</ResponseField>

<ResponseField name="HeaderBlockTooLarge">

A header block spans more than `MAX_HEADER_BLOCK_BYTES`.

</ResponseField>

<ResponseField name="PrefaceTooLarge">

The first request did not complete within `MAX_PREFACE_BYTES`.

</ResponseField>

<ResponseField name="TooManyFrames { max }">

The first request did not complete within `MAX_PREFACE_FRAMES` frames.

</ResponseField>

<ResponseField name="ReaderFinished">

A `PrefaceReader` was asked to read again after it returned a result or an error.

</ResponseField>

<ResponseField name="MissingSettings">

The first request arrived before any SETTINGS frame.

</ResponseField>

<ResponseField name="PseudoOrder">

The pseudo-headers are not each of `:method`, `:authority`, `:scheme` and
`:path` once, first.

</ResponseField>

<ResponseField name="StreamIdRange(u32)">

A frame cannot be built: the stream id is above 2^31 - 1.

</ResponseField>

<ResponseField name="Hpack(HpackError)">

The header block does not decode.

</ResponseField>

</Expandable>

</ResponseField>

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const CLIENT_PREFACE: &[u8; 24] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
pub const FRAME_HEADER_LEN: usize = 9;
pub const DEFAULT_MAX_FRAME_PAYLOAD: usize = 16_384;
pub const MAX_FRAME_PAYLOAD: usize = 0x00ff_ffff;
pub const MAX_PREFACE_BYTES: usize = 256 * 1024;
pub const MAX_HEADER_BLOCK_BYTES: usize = 64 * 1024;
pub const MAX_PREFACE_FRAMES: usize = 64;

pub mod frame_type {
    pub const DATA: u8 = 0x0;
    pub const HEADERS: u8 = 0x1;
    pub const PRIORITY: u8 = 0x2;
    pub const RST_STREAM: u8 = 0x3;
    pub const SETTINGS: u8 = 0x4;
    pub const PUSH_PROMISE: u8 = 0x5;
    pub const PING: u8 = 0x6;
    pub const GOAWAY: u8 = 0x7;
    pub const WINDOW_UPDATE: u8 = 0x8;
    pub const CONTINUATION: u8 = 0x9;
}
```

The 24 bytes every HTTP/2 client connection starts with (RFC 9113, section 3.4);
the frame header length; the largest payload a peer may send before it learns a
larger `MAX_FRAME_SIZE`; the largest payload any frame can carry (24 bits); the
most bytes `read_preface` reads before it gives up; the most bytes of HPACK data
one header block may span; and the most frames of any type before the end of the
first request's header block (browsers send three to eight). `frame_type` holds
the frame type codes of RFC 9113, section 6.

## `hpack::HpackDecoder`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone)]
pub struct HpackDecoder { /* private fields */ }

impl Default for HpackDecoder { /* HpackDecoder::new(DEFAULT_TABLE_SIZE) */ }

impl HpackDecoder {
    pub fn new(max_table_size: usize) -> Self;
    pub fn dynamic_len(&self) -> usize;
    pub fn dynamic_size(&self) -> usize;
    pub fn decode(&mut self, block: &[u8]) -> Result<Vec<HeaderField>, HpackError>;
}
```

A stateful HPACK decoder (RFC 7541): one per connection direction, fed the header
blocks in order. It is complete for what a client can send: indexed fields, the
three literal forms, dynamic table size updates, the static table, the dynamic
table with eviction, and Huffman-coded strings. `new` lets the peer use a dynamic
table of up to `max_table_size` bytes, lowered to `MAX_TABLE_SIZE` when it is
larger. `dynamic_len` and `dynamic_size` are the entry count and the size as RFC
7541 section 4.1 counts it. `decode` decodes one complete block, updating the
dynamic table, and fails when the block is truncated or malformed, refers to a
missing entry, resizes the table illegally, or decodes to more than
`MAX_HEADER_FIELDS` fields or `MAX_HEADER_LIST_BYTES` bytes.

## `hpack::HeaderField`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderField {
    pub name: String,
    pub value: String,
}

impl HeaderField {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self;
    pub fn is_pseudo(&self) -> bool;
}
```

One decoded header field, in the order it appeared in the block, such as
`:method` or `user-agent`. `is_pseudo` is true for a name that starts with `:`.

## `hpack::HpackError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HpackError {
    Truncated,
    IntegerOverflow,
    IndexZero,
    IndexOutOfRange(usize),
    Huffman(HuffmanError),
    NotUtf8,
    SizeUpdateTooLarge { requested: usize, max: usize },
    SizeUpdateAfterField,
    HeaderListTooLarge,
}
```

Why a header block was refused: it ends inside a field; an integer is larger than
the decoder supports; a field refers to index 0 or to an entry outside the static
and dynamic tables; a Huffman string is malformed; a name or value is not UTF-8; a
dynamic table size update asks for more than allowed, or follows a header field
instead of opening the block; or the block decodes to too many fields or bytes.

## `hpack` constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const DEFAULT_TABLE_SIZE: usize = 4096;
pub const MAX_TABLE_SIZE: usize = 1024 * 1024;
pub const MAX_HEADER_LIST_BYTES: usize = 64 * 1024;
pub const MAX_HEADER_FIELDS: usize = 512;
```

The initial `SETTINGS_HEADER_TABLE_SIZE`; the largest dynamic table the decoder
allows its peer (sixteen times the 64 KiB browsers announce); and the most decoded
bytes (names plus values) and fields one header block may produce.

## `huffman::decode`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn decode(input: &[u8]) -> Result<Vec<u8>, HuffmanError>

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HuffmanError {
    EndOfString,
    PaddingTooLong,
    PaddingNotOnes,
    InvalidCode,
}
```

Decodes a Huffman-coded HPACK string literal (RFC 7541, section 5.2 and appendix
B). The decoding tables are derived at compile time from the code lengths and
checked against the examples of RFC 7541 appendix C. It fails when the
end-of-string symbol appears inside the data (`EndOfString`), when the padding is
longer than seven bits (`PaddingTooLong`) or is not all ones (`PaddingNotOnes`),
or when the bits match no code (`InvalidCode`, which well-formed tables never
produce).

## Related

<Columns cols={2}>

<Card color="#d5a910" title="H2Spec" icon="server" href="/stealth/api/codec-h2-spec">

`H2Spec::from_capture` and `from_preface` record a captured start.

</Card>

<Card color="#1ca1c7" title="Akamai" icon="code" href="/stealth/api/codec-akamai">

The fingerprint string a preface produces.

</Card>

</Columns>
