//! Leak-by-design arenas for scenarios where lifetime management
//! is more important than memory cleanup.
//!
//! These arenas are explicitly designed to leak all allocated memory
//! when they go out of scope. This eliminates borrow checker issues
//! in complex scenarios at the cost of not reclaiming memory.
//!
//! Use these for:
//! - Compiler implementations where all data lives for compilation duration
//! - Short-lived programs where cleanup isn't critical
//! - Prototyping where simplicity > memory efficiency

use alloc::alloc::{Allocator, Layout, Global, AllocError};
use core::{
    cell::UnsafeCell,
    ptr::{self, NonNull},
    mem::{self, MaybeUninit},
};

/// A leak-by-design arena that never cleans up any memory.
/// This arena has no Drop implementation, which eliminates
/// borrow checker issues with complex lifetime scenarios.
///
/// All allocated memory is leaked when the arena goes out of scope.
/// This is intentional and trades memory cleanup for lifetime simplicity.
#[derive(Debug)]
pub struct LeakArena<A: Allocator = Global> {
    inner: UnsafeCell<LeakArenaInner<A>>,
}

#[derive(Debug)]
struct LeakArenaInner<A: Allocator> {
    allocator: A,
    current_chunk: Option<NonNull<u8>>,
    chunk_size: usize,
    chunk_pos: usize,
    chunk_remaining: usize,
}

impl<A: Allocator> LeakArena<A> {
    /// Create a new leak arena with the given allocator and chunk size
    pub fn new_in(allocator: A, chunk_size: usize) -> Self {
        Self {
            inner: UnsafeCell::new(LeakArenaInner {
                allocator,
                current_chunk: None,
                chunk_size,
                chunk_pos: 0,
                chunk_remaining: 0,
            }),
        }
    }

    unsafe fn inner_mut(&self) -> &mut LeakArenaInner<A> {
        unsafe { &mut *self.inner.get() }
    }

    fn alloc_new_chunk(&self, inner: &mut LeakArenaInner<A>) -> Result<(), AllocError> {
        let layout = Layout::array::<u8>(inner.chunk_size).map_err(|_| AllocError)?;
        let chunk = inner.allocator.allocate(layout)?;
        inner.current_chunk = Some(chunk.cast::<u8>());
        inner.chunk_pos = 0;
        inner.chunk_remaining = inner.chunk_size;
        Ok(())
    }

    fn alloc_bytes(&self, size: usize, align: usize) -> Result<*mut u8, AllocError> {
        let inner = unsafe { self.inner_mut() };

        // Ensure we have a chunk
        if inner.current_chunk.is_none() {
            self.alloc_new_chunk(inner)?;
        }

        let chunk = inner.current_chunk.unwrap();
        let chunk_start = chunk.as_ptr();
        let current_pos = unsafe { chunk_start.add(inner.chunk_pos) };
        
        // Align the current position
        let aligned_pos = current_pos as usize;
        let aligned_pos = (aligned_pos + align - 1) & !(align - 1);
        let aligned_ptr = aligned_pos as *mut u8;
        let offset_from_current = aligned_pos - (current_pos as usize);
        
        let total_needed = offset_from_current + size;
        
        if total_needed > inner.chunk_remaining {
            // Need a new chunk
            self.alloc_new_chunk(inner)?;
            return self.alloc_bytes(size, align); // Retry with new chunk
        }
        
        // Update positions
        inner.chunk_pos += total_needed;
        inner.chunk_remaining -= total_needed;
        
        Ok(aligned_ptr)
    }

    /// Allocate a single value of type T.
    /// The value will be leaked when the arena goes out of scope.
    pub fn alloc<T>(&self, value: T) -> &mut T {
        let ptr = self.alloc_bytes(mem::size_of::<T>(), mem::align_of::<T>())
            .expect("allocation failed") as *mut T;
        unsafe {
            ptr.write(value);
            &mut *ptr
        }
    }

    /// Allocate a slice by cloning from the given slice.
    /// The slice will be leaked when the arena goes out of scope.
    pub fn alloc_slice<T: Clone>(&self, values: &[T]) -> &mut [T] {
        if values.is_empty() {
            return &mut [];
        }
        
        let size = mem::size_of::<T>() * values.len();
        let align = mem::align_of::<T>();
        let ptr = self.alloc_bytes(size, align).expect("allocation failed") as *mut T;
        
        unsafe {
            for (i, value) in values.iter().enumerate() {
                ptr.add(i).write(value.clone());
            }
            core::slice::from_raw_parts_mut(ptr, values.len())
        }
    }

    /// Allocate a slice by copying from the given slice (for Copy types).
    /// The slice will be leaked when the arena goes out of scope.
    pub fn alloc_slice_copy<T: Copy>(&self, values: &[T]) -> &mut [T] {
        if values.is_empty() {
            return &mut [];
        }
        
        let size = mem::size_of::<T>() * values.len();
        let align = mem::align_of::<T>();
        let ptr = self.alloc_bytes(size, align).expect("allocation failed") as *mut T;
        
        unsafe {
            ptr::copy_nonoverlapping(values.as_ptr(), ptr, values.len());
            core::slice::from_raw_parts_mut(ptr, values.len())
        }
    }

    /// Allocate a copy of a string slice.
    /// The string will be leaked when the arena goes out of scope.
    pub fn alloc_str(&self, s: &str) -> &mut str {
        let bytes = self.alloc_slice_copy(s.as_bytes());
        unsafe { core::str::from_utf8_unchecked_mut(bytes) }
    }

    /// Allocate uninitialized space for a slice of T.
    /// The caller is responsible for initializing the elements.
    /// The slice will be leaked when the arena goes out of scope.
    pub fn alloc_slice_uninit<T>(&self, len: usize) -> &mut [MaybeUninit<T>] {
        if len == 0 {
            return &mut [];
        }
        
        let size = mem::size_of::<T>() * len;
        let align = mem::align_of::<T>();
        let ptr = self.alloc_bytes(size, align).expect("allocation failed") as *mut MaybeUninit<T>;
        
        unsafe { core::slice::from_raw_parts_mut(ptr, len) }
    }
}

impl LeakArena<Global> {
    /// Create a new leak arena with the global allocator and given chunk size
    pub fn new(chunk_size: usize) -> Self {
        Self::new_in(Global, chunk_size)
    }
}

// Explicitly NO Drop implementation - this is the key!
// The arena will leak all memory when it goes out of scope.
// This is intentional and eliminates borrow checker issues.

// Convenience type aliases
pub type LeakDroplessArena = LeakArena<Global>;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec::Vec, string::String};

    #[test]
    fn test_leak_arena_basic() {
        let arena = LeakArena::new(1024);
        
        let val = arena.alloc(42);
        assert_eq!(*val, 42);
        
        let s = arena.alloc_str("hello");
        assert_eq!(s, "hello");
        
        let slice = arena.alloc_slice(&[1, 2, 3]);
        assert_eq!(slice, [1, 2, 3]);
    }

    #[test]
    fn test_leak_arena_complex_types() {
        let arena = LeakArena::new(1024);
        
        // These would normally require Drop, but LeakArena doesn't call destructors
        let _vec = arena.alloc(Vec::from([1, 2, 3]));
        let _string = arena.alloc(String::from("test"));
        
        // Multiple allocations
        for i in 0..100 {
            let _val = arena.alloc(i);
        }
        
        // When this test ends, the arena will be dropped but nothing will be cleaned up
        // This is expected behavior for LeakArena
    }

    #[test]
    fn test_alignment() {
        let arena = LeakArena::new(1024);
        
        // Test that different alignments work correctly
        let _byte = arena.alloc(1u8);
        let _word = arena.alloc(1u32);
        let _double = arena.alloc(1u64);
        
        // Test that they're properly aligned
        let byte_ptr = arena.alloc(1u8) as *const u8;
        let word_ptr = arena.alloc(1u32) as *const u32;
        let double_ptr = arena.alloc(1u64) as *const u64;
        
        assert_eq!(byte_ptr as usize % mem::align_of::<u8>(), 0);
        assert_eq!(word_ptr as usize % mem::align_of::<u32>(), 0);
        assert_eq!(double_ptr as usize % mem::align_of::<u64>(), 0);
    }
}