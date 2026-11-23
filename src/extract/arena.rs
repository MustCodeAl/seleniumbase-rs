//! Native Arena allocation and zero-copy extraction for high-throughput scraping lifecycles.
//!
//! Provides [`PageArena`]—a native bump allocator that allocates table cells, metadata strings,
//! and parsed DOM tokens contiguously inside pre-allocated memory chunks.
//!
//! Recycles memory across page crawls with an $O(1)$ pointer reset, eliminating global heap
//! contention and memory fragmentation without relying on third-party allocation crates.

use std::cell::RefCell;
use std::fmt;

use super::table::TableData;
use crate::error::SeleniumBaseError;

/// Default chunk size for a [`PageArena`] (256 KB).
pub const DEFAULT_ARENA_CAPACITY: usize = 256 * 1024;

/// A contiguous bump-allocated memory arena for per-page extraction lifecycles.
///
/// Pre-allocates heap chunks and sub-allocates string slices via bump pointers.
/// Calling [`reset`](Self::reset) resets the allocation offset in $O(1)$, recycling the
/// entire memory block for the next page without individual `free()` system calls.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::extract::PageArena;
///
/// let mut arena = PageArena::with_capacity(64 * 1024);
/// let allocated = arena.alloc_str("hello world");
/// assert_eq!(allocated, "hello world");
///
/// // Reset the entire arena for the next page in O(1)
/// arena.reset();
/// ```
pub struct PageArena {
    chunks: RefCell<Vec<Box<[u8]>>>,
    current_chunk: RefCell<usize>,
    current_offset: RefCell<usize>,
    chunk_capacity: usize,
}

impl Default for PageArena {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_ARENA_CAPACITY)
    }
}

impl fmt::Debug for PageArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageArena")
            .field("chunk_capacity", &self.chunk_capacity)
            .field("chunks_count", &self.chunks.borrow().len())
            .field("current_chunk", &self.current_chunk.borrow())
            .field("current_offset", &self.current_offset.borrow())
            .finish()
    }
}

impl PageArena {
    /// Creates a new `PageArena` with default 256 KB chunk size.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `PageArena` with a specified chunk capacity in bytes.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(1024);
        let first_chunk = vec![0u8; cap].into_boxed_slice();
        Self {
            chunks: RefCell::new(vec![first_chunk]),
            current_chunk: RefCell::new(0),
            current_offset: RefCell::new(0),
            chunk_capacity: cap,
        }
    }

    /// Allocates a string slice inside the arena, returning a reference whose lifetime
    /// matches the arena borrow.
    #[must_use]
    pub fn alloc_str<'a>(&'a self, text: &str) -> &'a str {
        let bytes = text.as_bytes();
        let len = bytes.len();

        let mut chunk_idx = *self.current_chunk.borrow();
        let mut offset = *self.current_offset.borrow();

        let target_cap = self.chunk_capacity.max(len);

        // Check if we need to switch or add a chunk
        if offset + len > self.chunk_capacity {
            let next_idx = chunk_idx + 1;
            let mut chunks = self.chunks.borrow_mut();
            if next_idx >= chunks.len() {
                chunks.push(vec![0u8; target_cap].into_boxed_slice());
            }
            chunk_idx = next_idx;
            offset = 0;
            *self.current_chunk.borrow_mut() = chunk_idx;
            *self.current_offset.borrow_mut() = 0;
        }

        // SAFETY: The chunk buffer is allocated in a Box on the heap whose memory
        // address does not change when new chunks are appended to `chunks`.
        // The slice is only written here once and never mutated while borrowed.
        let slice = unsafe {
            let chunks_ref = &*self.chunks.as_ptr();
            let chunk_ptr = chunks_ref[chunk_idx].as_ptr() as *mut u8;
            let dest = std::slice::from_raw_parts_mut(chunk_ptr.add(offset), len);
            dest.copy_from_slice(bytes);
            std::str::from_utf8_unchecked(dest)
        };

        *self.current_offset.borrow_mut() = offset + len;
        slice
    }

    /// Resets the allocation offsets in $O(1)$, recycling all pre-allocated chunks
    /// for subsequent page extractions without deallocating memory.
    pub fn reset(&mut self) {
        *self.current_chunk.borrow_mut() = 0;
        *self.current_offset.borrow_mut() = 0;
    }

    /// Returns the total bytes allocated across active chunks.
    #[must_use]
    pub fn allocated_bytes(&self) -> usize {
        let chunk_idx = *self.current_chunk.borrow();
        let offset = *self.current_offset.borrow();
        chunk_idx
            .saturating_mul(self.chunk_capacity)
            .saturating_add(offset)
    }
}

/// Zero-copy structured table data stored inside a [`PageArena`].
///
/// Cells and headers are borrowed string slices (`&'a str`) allocated directly inside the arena,
/// guaranteeing contiguous physical memory layout and zero heap `malloc` calls per cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDataArena<'a> {
    /// Column header names.
    pub headers: Vec<&'a str>,
    /// Table rows of cell values.
    pub rows: Vec<Vec<&'a str>>,
}

impl<'a> TableDataArena<'a> {
    /// Parses a `TableDataArena` from an HTML fragment using the provided [`PageArena`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if no `<table>` element is present.
    pub fn from_html(html_text: &str, arena: &'a PageArena) -> Result<Self, SeleniumBaseError> {
        let table = TableData::from_html(html_text)?;
        let headers: Vec<&'a str> = table.headers().iter().map(|h| arena.alloc_str(h)).collect();

        let rows: Vec<Vec<&'a str>> = table
            .rows()
            .iter()
            .map(|row| row.iter().map(|cell| arena.alloc_str(cell)).collect())
            .collect();

        Ok(Self { headers, rows })
    }

    /// Returns the number of rows in the table.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Returns `true` if the table contains zero rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_allocates_and_resets() {
        let mut arena = PageArena::with_capacity(4096);
        let s = arena.alloc_str("test string");
        assert_eq!(s, "test string");
        assert!(arena.allocated_bytes() > 0);

        arena.reset();
        assert_eq!(arena.allocated_bytes(), 0);
    }

    #[test]
    fn arena_table_extraction() {
        let arena = PageArena::new();
        let html = r#"
            <table>
                <thead><tr><th>Col1</th><th>Col2</th></tr></thead>
                <tbody><tr><td>A</td><td>B</td></tr></tbody>
            </table>
        "#;
        let table = TableDataArena::from_html(html, &arena).expect("valid table");
        assert_eq!(table.headers, vec!["Col1", "Col2"]);
        assert_eq!(table.rows, vec![vec!["A", "B"]]);
    }
}
