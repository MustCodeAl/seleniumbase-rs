---
title: "codec::h3_spec"
sidebarTitle: "H3Spec"
description: "HTTP/3 and QUIC specification parameters: transport parameters, SETTINGS frame entries, and pseudo-header ordering."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_stealth::codec::h3_spec` (implemented in `crates/codec/src/h3_spec.rs`) models the wire-level parameters of modern Chromium HTTP/3 implementations: QUIC transport parameters in wire order, HTTP/3 SETTINGS frame entries, and pseudo-header sequence.

## Specification Model

### `H3Spec`

```rust
use seleniumbase_rs_stealth::codec::H3Spec;
use seleniumbase_rs::evasion::fingerprint::OsType;

// Current stable Chromium HTTP/3 spec
let spec = H3Spec::current_stable();
assert_eq!(spec.pseudo_headers(), &[":method", ":authority", ":scheme", ":path"]);

// Target specific Chromium version and OS
let macos_spec = H3Spec::chrome(157, OsType::Macos);
assert_eq!(macos_spec.major(), 157);
```

### `H3SpecError`

`H3SpecError` is returned when parsing or validating malformed HTTP/3 specifications or unsupported transport parameters.

### Transport Parameters and Constants

`h3_spec::transport_param_id` provides RFC 9000 section 18.2 constants:
- `INITIAL_MAX_DATA` (`0x04`)
- `INITIAL_MAX_STREAM_DATA_BIDI_LOCAL` (`0x05`)
- `INITIAL_MAX_STREAMS_BIDI` (`0x08`)
- `MAX_IDLE_TIMEOUT` (`0x01`)
- `DISABLE_ACTIVE_MIGRATION` (`0x0c`)

`h3_spec::setting_id` provides RFC 9114 section 7.2.4.1 constants:
- `QPACK_MAX_TABLE_CAPACITY` (`0x01`)
- `MAX_FIELD_SECTION_SIZE` (`0x06`)
- `QPACK_BLOCKED_STREAMS` (`0x07`)
- `ENABLE_CONNECT_PROTOCOL` (`0x08`)
- `H3_DATAGRAM` (`0x33`)

### `TransportParam`

Encapsulates individual QUIC transport parameters encoded during the initial handshake:

```rust
use seleniumbase_rs_stealth::codec::h3_spec::TransportParam;

let param = TransportParam::new(0x01, vec![0x80, 0x00, 0x75, 0x30]);
assert_eq!(param.id(), 0x01);
```
