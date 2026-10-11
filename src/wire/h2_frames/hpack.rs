//! HPACK header block decoding (RFC 7541), enough to recover the header order of a request.
//!
//! The decoder is complete for what a client can send: indexed fields, the three literal
//! forms, dynamic table size updates, the static table, the dynamic table with eviction,
//! and Huffman-coded strings. It bounds what a hostile block can make it build: the size
//! of the dynamic table, the number of fields and the total decoded bytes.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::huffman::{self, HuffmanError};

/// The initial `SETTINGS_HEADER_TABLE_SIZE`, which is also the largest table an encoder may
/// use until the decoder advertises a different value.
pub const DEFAULT_TABLE_SIZE: usize = 4096;
/// The largest dynamic table [`HpackDecoder::new`] allows its peer, sixteen times the 64 KiB that
/// browsers announce. A larger request is lowered to it.
pub const MAX_TABLE_SIZE: usize = 1024 * 1024;
/// The most decoded bytes (names plus values) one header block may produce.
pub const MAX_HEADER_LIST_BYTES: usize = 64 * 1024;
/// The most header fields one header block may produce.
pub const MAX_HEADER_FIELDS: usize = 512;

/// Bytes added to every dynamic table entry on top of its name and value (RFC 7541, 4.1).
const ENTRY_OVERHEAD: usize = 32;
/// The longest shift of an integer continuation byte: five bytes carry 35 bits.
const MAX_INTEGER_SHIFT: u32 = 28;
/// The static table has 61 entries; dynamic entries are numbered from 62.
const STATIC_LEN: usize = 61;

/// The HPACK static table (RFC 7541, appendix A), entry 1 first.
static STATIC_TABLE: [(&str, &str); STATIC_LEN] = [
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", "gzip, deflate"),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

/// One decoded header field, in the order it appeared in the block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderField {
    /// The field name, such as `:method` or `user-agent`.
    pub name: String,
    /// The field value.
    pub value: String,
}

impl HeaderField {
    /// A field from a name and a value.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// The size of the field as the dynamic table counts it.
    fn table_size(&self) -> usize {
        self.name.len() + self.value.len() + ENTRY_OVERHEAD
    }

    /// Whether the field is a pseudo-header such as `:path`.
    #[must_use]
    pub fn is_pseudo(&self) -> bool {
        self.name.starts_with(':')
    }
}

/// Why a header block was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HpackError {
    /// The block ends inside an integer, a string or a field.
    #[error("the header block ends inside a field")]
    Truncated,
    /// An integer is larger than this decoder supports.
    #[error("an HPACK integer is too large")]
    IntegerOverflow,
    /// A field refers to index 0, which is not assigned.
    #[error("index 0 is not a valid header table index")]
    IndexZero,
    /// A field refers to an entry that is not in the table.
    #[error("header table index {0} is outside the static and dynamic tables")]
    IndexOutOfRange(usize),
    /// A Huffman-coded string is malformed.
    #[error("malformed Huffman-coded string: {0}")]
    Huffman(#[from] HuffmanError),
    /// A name or a value is not valid UTF-8.
    #[error("a header name or value is not valid UTF-8")]
    NotUtf8,
    /// A dynamic table size update asks for more than the decoder allows.
    #[error("dynamic table size update to {requested} exceeds the limit of {max}")]
    SizeUpdateTooLarge {
        /// The size the block asked for.
        requested: usize,
        /// The largest size the decoder allows.
        max: usize,
    },
    /// A dynamic table size update follows a header field instead of opening the block.
    #[error("dynamic table size update after a header field")]
    SizeUpdateAfterField,
    /// The block decodes to more fields or bytes than the decoder accepts.
    #[error("the header block decodes to too many fields or bytes")]
    HeaderListTooLarge,
}

/// A read position over one header block.
struct Reader<'a> {
    block: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.block.get(self.pos).copied()
    }

    fn byte(&mut self) -> Result<u8, HpackError> {
        let byte = self.peek().ok_or(HpackError::Truncated)?;
        self.pos += 1;
        Ok(byte)
    }

    /// Reads an integer whose first `prefix_bits` bits share a byte with other data
    /// (RFC 7541, section 5.1).
    fn integer(&mut self, prefix_bits: u32) -> Result<usize, HpackError> {
        let mask = (1_u8 << prefix_bits) - 1;
        let first = self.byte()? & mask;
        let mut value = u64::from(first);
        if first < mask {
            return usize::try_from(value).map_err(|_wide| HpackError::IntegerOverflow);
        }
        let mut shift = 0;
        loop {
            let byte = self.byte()?;
            if shift > MAX_INTEGER_SHIFT {
                return Err(HpackError::IntegerOverflow);
            }
            value += u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return usize::try_from(value).map_err(|_wide| HpackError::IntegerOverflow);
            }
            shift += 7;
        }
    }

    /// Reads a string literal, Huffman-decoding it when its first bit says so
    /// (RFC 7541, section 5.2).
    fn string(&mut self) -> Result<String, HpackError> {
        let huffman_coded = self.peek().ok_or(HpackError::Truncated)? & 0x80 != 0;
        let len = self.integer(7)?;
        let end = self.pos.checked_add(len).ok_or(HpackError::Truncated)?;
        let raw = self.block.get(self.pos..end).ok_or(HpackError::Truncated)?;
        self.pos = end;
        let bytes = if huffman_coded {
            huffman::decode(raw)?
        } else {
            raw.to_vec()
        };
        String::from_utf8(bytes).map_err(|_invalid| HpackError::NotUtf8)
    }
}

/// The fields decoded so far, with the limits applied as they are added.
#[derive(Default)]
struct Decoded {
    fields: Vec<HeaderField>,
    bytes: usize,
}

impl Decoded {
    fn push(&mut self, field: HeaderField) -> Result<(), HpackError> {
        self.bytes += field.name.len() + field.value.len();
        if self.fields.len() >= MAX_HEADER_FIELDS || self.bytes > MAX_HEADER_LIST_BYTES {
            return Err(HpackError::HeaderListTooLarge);
        }
        self.fields.push(field);
        Ok(())
    }
}

/// A stateful HPACK decoder: one per connection direction, fed the header blocks in order.
#[derive(Debug, Clone)]
pub struct HpackDecoder {
    /// Dynamic entries, newest first (index 62 is the front).
    dynamic: VecDeque<HeaderField>,
    dynamic_size: usize,
    /// The size the encoder last signalled.
    current_max: usize,
    /// The largest size the encoder may signal.
    protocol_max: usize,
}

impl Default for HpackDecoder {
    fn default() -> Self {
        Self::new(DEFAULT_TABLE_SIZE)
    }
}

impl HpackDecoder {
    /// A decoder whose peer may use a dynamic table of up to `max_table_size` bytes, lowered to
    /// [`MAX_TABLE_SIZE`] when it is larger, so that no caller can make the table unbounded.
    #[must_use]
    pub fn new(max_table_size: usize) -> Self {
        let max_table_size = max_table_size.min(MAX_TABLE_SIZE);
        Self {
            dynamic: VecDeque::new(),
            dynamic_size: 0,
            current_max: max_table_size,
            protocol_max: max_table_size,
        }
    }

    /// The number of entries in the dynamic table.
    #[must_use]
    pub fn dynamic_len(&self) -> usize {
        self.dynamic.len()
    }

    /// The size of the dynamic table as RFC 7541 section 4.1 counts it.
    #[must_use]
    pub fn dynamic_size(&self) -> usize {
        self.dynamic_size
    }

    /// Decodes one complete header block, updating the dynamic table.
    ///
    /// # Errors
    ///
    /// Returns an [`HpackError`] when the block is truncated or malformed, refers to a
    /// missing table entry, resizes the table illegally, or decodes to more than
    /// [`MAX_HEADER_FIELDS`] fields or [`MAX_HEADER_LIST_BYTES`] bytes.
    pub fn decode(&mut self, block: &[u8]) -> Result<Vec<HeaderField>, HpackError> {
        let mut reader = Reader { block, pos: 0 };
        let mut out = Decoded::default();
        while let Some(first) = reader.peek() {
            self.decode_one(first, &mut reader, &mut out)?;
        }
        Ok(out.fields)
    }

    fn decode_one(
        &mut self,
        first: u8,
        reader: &mut Reader<'_>,
        out: &mut Decoded,
    ) -> Result<(), HpackError> {
        if first & 0x80 != 0 {
            let index = reader.integer(7)?;
            out.push(self.field_at(index)?)
        } else if first & 0x40 != 0 {
            self.literal(reader, 6, true, out)
        } else if first & 0x20 != 0 {
            if !out.fields.is_empty() {
                return Err(HpackError::SizeUpdateAfterField);
            }
            let requested = reader.integer(5)?;
            self.resize(requested)
        } else {
            self.literal(reader, 4, false, out)
        }
    }

    /// A literal field: the name is an index into the tables or a string, then a value string.
    fn literal(
        &mut self,
        reader: &mut Reader<'_>,
        prefix_bits: u32,
        index_it: bool,
        out: &mut Decoded,
    ) -> Result<(), HpackError> {
        let index = reader.integer(prefix_bits)?;
        let name = if index == 0 {
            reader.string()?
        } else {
            self.field_at(index)?.name
        };
        let field = HeaderField {
            name,
            value: reader.string()?,
        };
        if index_it {
            self.insert(field.clone());
        }
        out.push(field)
    }

    /// The field at `index` in the combined static and dynamic table (1-based).
    fn field_at(&self, index: usize) -> Result<HeaderField, HpackError> {
        if index == 0 {
            return Err(HpackError::IndexZero);
        }
        if let Some((name, value)) = STATIC_TABLE.get(index - 1) {
            return Ok(HeaderField::new(*name, *value));
        }
        self.dynamic
            .get(index - STATIC_LEN - 1)
            .cloned()
            .ok_or(HpackError::IndexOutOfRange(index))
    }

    fn insert(&mut self, field: HeaderField) {
        let size = field.table_size();
        if size > self.current_max {
            self.dynamic.clear();
            self.dynamic_size = 0;
            return;
        }
        self.evict_to(self.current_max - size);
        self.dynamic_size += size;
        self.dynamic.push_front(field);
    }

    fn resize(&mut self, requested: usize) -> Result<(), HpackError> {
        if requested > self.protocol_max {
            return Err(HpackError::SizeUpdateTooLarge {
                requested,
                max: self.protocol_max,
            });
        }
        self.current_max = requested;
        self.evict_to(requested);
        Ok(())
    }

    fn evict_to(&mut self, limit: usize) {
        while self.dynamic_size > limit {
            let Some(oldest) = self.dynamic.pop_back() else {
                self.dynamic_size = 0;
                break;
            };
            self.dynamic_size -= oldest.table_size();
        }
    }
}

#[cfg(test)]
mod tests;
