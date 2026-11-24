//! Native Arena allocation and zero-copy extraction for high-throughput scraping lifecycles.
//!
//! Provides [`PageArena`]—a native bump allocator that allocates table cells, metadata strings,
//! and parsed DOM tokens contiguously inside pre-allocated memory chunks.
//!
//! Recycles memory across page crawls with an $O(1)$ pointer reset, eliminating global heap
//! contention and memory fragmentation without relying on third-party allocation crates.

use std::alloc::Layout;
use std::cell::RefCell;
use std::fmt;
use std::ptr::NonNull;

use allocator_api2::alloc::{AllocError, Allocator};

pub use allocator_api2::boxed::Box as ArenaBox;
pub use allocator_api2::vec::Vec as ArenaVec;

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

    /// Allocates a value of type `T` on the arena, returning a mutable reference.
    ///
    /// # Panics
    ///
    /// Panics if the arena allocation fails.
    #[allow(
        clippy::mut_from_ref,
        reason = "Arena bump allocator provides interior mutability"
    )]
    pub fn alloc<T>(&self, val: T) -> &mut T {
        let layout = Layout::new::<T>();
        let non_null = self
            .allocate(layout)
            .expect("arena allocation must succeed");
        // SAFETY: The allocated memory is uniquely borrowed, aligned, and properly sized for `T`.
        unsafe {
            let ptr = non_null.as_ptr() as *mut T;
            std::ptr::write(ptr, val);
            &mut *ptr
        }
    }

    /// Allocates and copies a slice of elements into the arena.
    ///
    /// # Panics
    ///
    /// Panics if the arena allocation fails.
    #[allow(
        clippy::mut_from_ref,
        reason = "Arena bump allocator provides interior mutability"
    )]
    pub fn alloc_slice<T: Clone>(&self, slice: &[T]) -> &mut [T] {
        let layout = Layout::array::<T>(slice.len()).expect("valid slice layout");
        let non_null = self
            .allocate(layout)
            .expect("arena allocation must succeed");
        // SAFETY: The allocated pointer is aligned and sized for `[T]`.
        unsafe {
            let ptr = non_null.as_ptr() as *mut T;
            for (i, item) in slice.iter().enumerate() {
                std::ptr::write(ptr.add(i), item.clone());
            }
            std::slice::from_raw_parts_mut(ptr, slice.len())
        }
    }

    /// Creates a new [`ArenaVec`] using this arena as its custom allocator.
    #[must_use]
    pub fn new_vec_in<T>(&self) -> ArenaVec<T, &Self> {
        ArenaVec::new_in(self)
    }

    /// Creates a new [`ArenaBox`] containing `val` using this arena as its allocator.
    pub fn new_box_in<T>(&self, val: T) -> ArenaBox<T, &Self> {
        ArenaBox::new_in(val, self)
    }

    /// Allocates a string slice inside the arena, returning a reference whose lifetime
    /// matches the arena borrow.
    #[must_use]
    pub fn alloc_str<'a>(&'a self, text: &str) -> &'a str {
        let bytes = text.as_bytes();
        let layout = Layout::for_value(bytes);
        let non_null = self
            .allocate(layout)
            .expect("arena allocation must succeed");
        // SAFETY: The memory is freshly allocated, non-overlapping, and copied from valid UTF-8.
        unsafe {
            let dest = std::slice::from_raw_parts_mut(non_null.as_ptr() as *mut u8, bytes.len());
            dest.copy_from_slice(bytes);
            std::str::from_utf8_unchecked(dest)
        }
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

// SAFETY: PageArena satisfies the Allocator contract by providing contiguous, properly aligned
// memory buffers. Deallocations are no-ops because PageArena uses bump-allocation with O(1) bulk reset.
unsafe impl Allocator for PageArena {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let align = layout.align();
        let size = layout.size();
        if size == 0 {
            let dangling = NonNull::new(align as *mut u8).ok_or(AllocError)?;
            return Ok(NonNull::slice_from_raw_parts(dangling, 0));
        }

        let mut chunk_idx = *self.current_chunk.borrow();
        let mut offset = *self.current_offset.borrow();

        let aligned_offset =
            (offset.saturating_add(align).saturating_sub(1)) & !(align.saturating_sub(1));
        let needed_chunk_capacity = self.chunk_capacity.max(size.saturating_add(align));

        if aligned_offset.saturating_add(size) > self.chunk_capacity {
            let next_idx = chunk_idx + 1;
            let mut chunks = self.chunks.borrow_mut();
            if next_idx >= chunks.len() {
                chunks.push(vec![0u8; needed_chunk_capacity].into_boxed_slice());
            }
            chunk_idx = next_idx;
            offset = 0;
            *self.current_chunk.borrow_mut() = chunk_idx;
            *self.current_offset.borrow_mut() = 0;
        }

        let start_offset =
            (offset.saturating_add(align).saturating_sub(1)) & !(align.saturating_sub(1));
        let end_offset = start_offset.saturating_add(size);
        *self.current_offset.borrow_mut() = end_offset;

        // SAFETY: The chunk buffer is a Box heap allocation that remains valid and pinned.
        let ptr = unsafe {
            let chunks_ref = &*self.chunks.as_ptr();
            let chunk_ptr = chunks_ref[chunk_idx].as_ptr() as *mut u8;
            chunk_ptr.add(start_offset)
        };

        NonNull::new(ptr)
            .map(|nn| NonNull::slice_from_raw_parts(nn, size))
            .ok_or(AllocError)
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // Bump arena: individual deallocations are no-ops; reclaimed on reset()
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
    fn arena_allocator_api_vec_and_box() {
        let arena = PageArena::with_capacity(4096);
        let mut vec = arena.new_vec_in::<u32>();
        vec.push(10);
        vec.push(20);
        vec.push(30);
        assert_eq!(vec.as_slice(), &[10, 20, 30]);

        let b = arena.new_box_in("allocated in arena");
        assert_eq!(*b, "allocated in arena");

        let val_ref = arena.alloc(42usize);
        assert_eq!(*val_ref, 42);

        let slice_ref = arena.alloc_slice(&[1u8, 2, 3, 4, 5]);
        assert_eq!(slice_ref, &[1, 2, 3, 4, 5]);
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
